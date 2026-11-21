mod normalize;

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::future::BoxFuture;
use futures::stream::{BoxStream, FuturesUnordered};
use futures::{FutureExt as _, StreamExt as _};
use i18n::t;
use serde_json::{Value, json};
use tokio::io::AsyncBufReadExt;
use tokio::sync::mpsc;

use crate::jsonrpc::{Incoming, RpcClient};
use crate::process::{Child, Stdio};
use crate::{
    AgentEvent, CancellationToken, DoneStatus, ExternalSession, Harness, HarnessError, HarnessId,
    Model, PermissionMode, ReasoningLevel, RunControls, RunRequest, SlashCommand, Steer,
    SteeringMode, TurnSettings, UserInputAnswer, UserInputQuestion, shutdown_child,
};
use normalize::Feed;

const MAX_LISTED_SESSIONS: usize = 500;
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);
const COMMANDS_WAIT: Duration = Duration::from_secs(8);
/// ACP updates name no turn, so the next prompt waits until a stopped turn has gone quiet.
const STOPPED_TURN_SETTLE: Duration = Duration::from_millis(250);

pub struct AcpAgent {
    pub id: HarnessId,
    pub name: &'static str,
    pub program: &'static str,
    pub args: &'static [&'static str],
    pub executable_env: &'static str,
    pub install_paths: fn() -> Vec<PathBuf>,
    pub searched: &'static str,
    pub update_args: &'static [&'static str],
    pub modes: &'static [(PermissionMode, &'static str)],
    /// When false, messages sent during a turn wait for it to end.
    pub steers_mid_turn: bool,
    /// An `n!` key, shown when the agent says the user must sign in.
    pub sign_in_hint: &'static str,
    pub command_output: fn(&str) -> &str,
    pub reasoning_levels: &'static [ReasoningLevel],
}

impl AcpAgent {
    fn mode(&self, permission: PermissionMode) -> Option<&'static str> {
        self.modes
            .iter()
            .find(|(mode, _)| *mode == permission)
            .map(|(_, id)| *id)
    }
}

pub struct AcpHarness {
    agent: &'static AcpAgent,
    models_cache: crate::catalog::Catalog,
    executable: Option<PathBuf>,
    interrupt_grace: Duration,
    kill_grace: Duration,
}

struct Connection {
    child: Child,
    client: RpcClient,
    incoming: mpsc::Receiver<Incoming>,
    stderr_tail: crate::StderrTail,
    stderr_read: Option<tokio::task::JoinHandle<()>>,
}

impl AcpHarness {
    pub fn new(agent: &'static AcpAgent) -> Self {
        Self {
            agent,
            models_cache: crate::catalog::Catalog::default(),
            executable: None,
            interrupt_grace: Duration::from_secs(2),
            kill_grace: Duration::from_secs(3),
        }
    }

    pub fn with_executable(mut self, path: impl Into<PathBuf>) -> Self {
        self.executable = Some(path.into());
        self
    }

    /// `interrupt_grace` runs from `session/cancel` to SIGTERM, `kill_grace` from SIGTERM to SIGKILL.
    pub fn with_graces(mut self, interrupt_grace: Duration, kill_grace: Duration) -> Self {
        self.interrupt_grace = interrupt_grace;
        self.kill_grace = kill_grace;
        self
    }

    fn resolve_executable(&self) -> Result<PathBuf, HarnessError> {
        if let Some(path) = &self.executable {
            return crate::executable::validate_native_override(path);
        }
        if let Some(path) =
            std::env::var_os(self.agent.executable_env).filter(|path| !path.is_empty())
        {
            return crate::executable::validate_native_override(&PathBuf::from(path));
        }
        crate::executable::find_on_paths(self.agent.program, (self.agent.install_paths)())
            .ok_or_else(|| HarnessError::agent_missing(self.agent.id, self.agent.searched))
    }

    fn current_model_context(&self) -> Result<crate::ModelContext, HarnessError> {
        crate::model_context::context(self.id(), &self.resolve_executable()?, &[])
    }

    async fn connect(&self, cwd: &Path) -> Result<Connection, HarnessError> {
        let exe = self.resolve_executable()?;
        let mut cmd = crate::process::command(&exe);
        cmd.args(self.agent.args);
        cmd.current_dir(cwd);
        crate::compose_child_path(&mut cmd, &exe);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                HarnessError::NotInstalled(crate::executable::binary_hint(&exe))
            } else {
                HarnessError::Io(error)
            }
        })?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            shutdown_child(&mut child, self.kill_grace).await;
            return Err(HarnessError::Protocol(format!(
                "{} has no stdio",
                self.agent.program
            )));
        };
        let stderr_tail = crate::StderrTail::default();
        let stderr_read = child.stderr.take().map(|stderr| {
            let tail = stderr_tail.clone();
            let agent = self.agent.name;
            tokio::spawn(async move {
                let mut lines = tokio::io::BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::debug!(target: "agent_harness::acp", agent, "stderr: {line}");
                    tail.push(&line);
                }
            })
        });
        let (client, incoming) = RpcClient::new(stdin, stdout);
        Ok(Connection {
            child,
            client,
            incoming,
            stderr_tail,
            stderr_read,
        })
    }

    async fn probe<T, F, Fut>(
        &self,
        cwd: Option<&Path>,
        timeout: Duration,
        timeout_message: &str,
        body: F,
    ) -> Result<T, HarnessError>
    where
        F: FnOnce(RpcClient, mpsc::Receiver<Incoming>) -> Fut,
        Fut: std::future::Future<Output = Result<T, HarnessError>>,
    {
        let cwd = cwd.map_or_else(crate::executable::scratch_dir, Path::to_path_buf);
        let Connection {
            mut child,
            client,
            mut incoming,
            ..
        } = self.connect(&cwd).await?;
        let probe = async {
            request_collecting(&client, &mut incoming, "initialize", initialize_params()).await?;
            body(client.clone(), incoming).await
        };
        let result = tokio::time::timeout(timeout, probe).await;
        let kill_grace = self.kill_grace;
        tokio::spawn(async move { shutdown_child(&mut child, kill_grace).await });
        match result {
            Ok(inner) => inner.map_err(|error| signed_out(self.agent, error)),
            Err(_) => Err(HarnessError::Protocol(timeout_message.into())),
        }
    }

    async fn discover_models(&self) -> Result<Vec<Model>, HarnessError> {
        let cwd = crate::executable::scratch_dir();
        let agent = self.agent;
        self.probe(
            None,
            PROBE_TIMEOUT,
            "model discovery timed out",
            |client, mut incoming| async move {
                let (session, _) = request_collecting(
                    &client,
                    &mut incoming,
                    "session/new",
                    json!({ "cwd": cwd, "mcpServers": [] }),
                )
                .await?;
                let session_id = session_id(agent, &session)?;
                let mut config = Config::from_result(&session);
                let current = config.current(MODEL).map(str::to_owned);
                let mut models = Vec::new();
                for (value, name) in config.choices(MODEL) {
                    if current.as_deref() != Some(value.as_str()) {
                        config
                            .apply(&client, &mut incoming, &session_id, MODEL, &value)
                            .await?;
                    }
                    let model = Model {
                        id: value.clone(),
                        label: name,
                        description: None,
                        reasoning_levels: config
                            .choices(THOUGHT_LEVEL)
                            .iter()
                            .filter_map(|(level, _)| normalize::reasoning_level(level))
                            .collect(),
                        options: Vec::new(),
                    };
                    if current.as_deref() == Some(value.as_str()) {
                        models.insert(0, model);
                    } else {
                        models.push(model);
                    }
                }
                if models.is_empty() {
                    return Err(crate::CatalogFailure {
                        code: crate::CatalogFailureCode::Failed,
                        message: format!("{} returned no models", agent.name),
                    }
                    .into());
                }
                Ok(models)
            },
        )
        .await
    }
}

fn initialize_params() -> Value {
    json!({
        "protocolVersion": 1,
        "clientCapabilities": {
            "fs": { "readTextFile": false, "writeTextFile": false },
            "terminal": false,
        },
        "clientInfo": {
            "name": "wu",
            "title": "Wu",
            "version": env!("CARGO_PKG_VERSION"),
        },
    })
}

/// ACP's `auth_required` error, which agents send for every session request until the user signs in.
fn needs_sign_in(error: &HarnessError) -> bool {
    matches!(error, HarnessError::Protocol(message)
        if message.contains("(code -32000)") || message.contains("Authentication required"))
}

fn sign_in_message(agent: &AcpAgent, error: &HarnessError) -> String {
    if needs_sign_in(error) {
        i18n::translate(agent.sign_in_hint).to_string()
    } else {
        error.to_string()
    }
}

fn signed_out(agent: &AcpAgent, error: HarnessError) -> HarnessError {
    if needs_sign_in(&error) {
        HarnessError::Protocol(sign_in_message(agent, &error))
    } else {
        error
    }
}

fn session_id(agent: &AcpAgent, result: &Value) -> Result<String, HarnessError> {
    result
        .get("sessionId")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| HarnessError::Protocol(format!("{} returned no session id", agent.program)))
}

async fn request_collecting(
    client: &RpcClient,
    incoming: &mut mpsc::Receiver<Incoming>,
    method: &str,
    params: Value,
) -> Result<(Value, Vec<Value>), HarnessError> {
    let mut response = client.request_now(method, params);
    let mut updates = Vec::new();
    loop {
        tokio::select! {
            biased;
            message = incoming.recv() => match message {
                Some(Incoming::Notification { method, params }) if method == "session/update" => {
                    updates.push(params);
                }
                Some(Incoming::Notification { .. }) => {}
                Some(Incoming::Request { id, method, .. }) => {
                    client.respond_error(&id, -32601, &format!("unsupported method: {method}"));
                }
                Some(Incoming::Eof) | None => {
                    return response.await.map(|result| (result, updates));
                }
            },
            result = &mut response => return result.map(|result| (result, updates)),
        }
    }
}

fn updates_for<'a>(updates: &'a [Value], session_id: &'a str) -> impl Iterator<Item = &'a Value> {
    updates.iter().filter_map(move |params| {
        (params.get("sessionId").and_then(Value::as_str) == Some(session_id))
            .then(|| params.get("update"))
            .flatten()
    })
}

const MODEL: &str = "model";
const MODE: &str = "mode";
const THOUGHT_LEVEL: &str = "thought_level";

/// Newer agents describe their settings as `configOptions`; older ones send `models` and `modes`.
#[derive(Default)]
struct Config {
    options: Vec<Value>,
    models: Option<Value>,
    modes: Option<Value>,
}

impl Config {
    fn from_result(result: &Value) -> Self {
        let mut config = Self::default();
        config.replace(result);
        config
    }

    fn replace(&mut self, result: &Value) {
        if let Some(options) = result.get("configOptions").and_then(Value::as_array) {
            self.options = options.clone();
        }
        if let Some(models) = result.get("models").filter(|models| models.is_object()) {
            self.models = Some(models.clone());
        }
        if let Some(modes) = result.get("modes").filter(|modes| modes.is_object()) {
            self.modes = Some(modes.clone());
        }
    }

    fn option(&self, category: &str) -> Option<&Value> {
        self.options.iter().find(|option| {
            option.get("category").and_then(Value::as_str) == Some(category)
                || option.get("id").and_then(Value::as_str) == Some(category)
        })
    }

    fn legacy(&self, category: &str) -> Option<(&Value, &'static str, &'static str, &'static str)> {
        match category {
            MODEL => Some((
                self.models.as_ref()?,
                "availableModels",
                "modelId",
                "currentModelId",
            )),
            MODE => Some((
                self.modes.as_ref()?,
                "availableModes",
                "id",
                "currentModeId",
            )),
            _ => None,
        }
    }

    fn current(&self, category: &str) -> Option<&str> {
        match self.option(category) {
            Some(option) => option.get("currentValue")?.as_str(),
            None => {
                let (legacy, _, _, current) = self.legacy(category)?;
                legacy.get(current)?.as_str()
            }
        }
    }

    fn choices(&self, category: &str) -> Vec<(String, String)> {
        let (entries, id_key) = match self.option(category) {
            Some(option) => (option.get("options"), "value"),
            None => match self.legacy(category) {
                Some((legacy, list, id, _)) => (legacy.get(list), id),
                None => return Vec::new(),
            },
        };
        entries
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|choice| {
                let value = choice.get(id_key)?.as_str()?.trim();
                if value.is_empty() {
                    return None;
                }
                let name = choice
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .unwrap_or(value);
                Some((value.to_owned(), name.to_owned()))
            })
            .collect()
    }

    fn offers(&self, category: &str, value: &str) -> bool {
        self.choices(category)
            .iter()
            .any(|(choice, _)| choice == value)
    }

    fn set_request(
        &self,
        session_id: &str,
        category: &str,
        value: &str,
    ) -> Option<(&'static str, Value)> {
        if let Some(id) = self.option(category).and_then(|option| option.get("id")) {
            return Some((
                "session/set_config_option",
                json!({ "sessionId": session_id, "configId": id, "value": value }),
            ));
        }
        self.legacy(category)?;
        Some(match category {
            MODEL => (
                "session/set_model",
                json!({ "sessionId": session_id, "modelId": value }),
            ),
            _ => (
                "session/set_mode",
                json!({ "sessionId": session_id, "modeId": value }),
            ),
        })
    }

    fn assume(&mut self, category: &str, value: &str) {
        if let Some(option) = self.options.iter_mut().find(|option| {
            option.get("category").and_then(Value::as_str) == Some(category)
                || option.get("id").and_then(Value::as_str) == Some(category)
        }) {
            option["currentValue"] = Value::String(value.to_owned());
            return;
        }
        let (legacy, key) = match category {
            MODEL => (&mut self.models, "currentModelId"),
            MODE => (&mut self.modes, "currentModeId"),
            _ => return,
        };
        if let Some(legacy) = legacy {
            legacy[key] = Value::String(value.to_owned());
        }
    }

    fn note_update(&mut self, update: &Value) {
        match update.get("sessionUpdate").and_then(Value::as_str) {
            Some("config_option_update") => self.replace(update),
            Some("current_mode_update") => {
                if let Some(mode) = update.get("currentModeId").and_then(Value::as_str) {
                    self.assume(MODE, mode);
                }
            }
            _ => {}
        }
    }

    async fn apply(
        &mut self,
        client: &RpcClient,
        incoming: &mut mpsc::Receiver<Incoming>,
        session_id: &str,
        category: &str,
        value: &str,
    ) -> Result<Vec<Value>, HarnessError> {
        if self.current(category) == Some(value) || !self.offers(category, value) {
            return Ok(Vec::new());
        }
        let Some((method, params)) = self.set_request(session_id, category, value) else {
            return Ok(Vec::new());
        };
        let (result, updates) = request_collecting(client, incoming, method, params).await?;
        if result.get("configOptions").is_some() {
            self.replace(&result);
        } else {
            self.assume(category, value);
        }
        Ok(updates)
    }

    async fn apply_all(
        &mut self,
        client: &RpcClient,
        incoming: &mut mpsc::Receiver<Incoming>,
        session_id: &str,
        wanted: &[(&str, &str)],
    ) -> Applied {
        let mut applied = Applied::default();
        for (category, value) in wanted {
            match self
                .apply(client, incoming, session_id, category, value)
                .await
            {
                Ok(updates) => applied.updates_meanwhile.extend(updates),
                Err(error) => applied.problems.push(error.to_string()),
            }
        }
        applied
    }
}

#[derive(Default)]
struct Applied {
    updates_meanwhile: Vec<Value>,
    problems: Vec<String>,
}

fn wanted_settings<'a>(
    model: Option<&'a str>,
    mode: Option<&'a str>,
    reasoning: Option<ReasoningLevel>,
) -> Vec<(&'static str, &'a str)> {
    let mut wanted = Vec::new();
    if let Some(model) = model.filter(|model| !model.is_empty()) {
        wanted.push((MODEL, model));
    }
    if let Some(mode) = mode {
        wanted.push((MODE, mode));
    }
    // The thinking levels on offer depend on the model, so this goes after the model switch.
    if let Some(level) = reasoning.map(thought_level) {
        wanted.push((THOUGHT_LEVEL, level));
    }
    wanted
}

fn thought_level(level: ReasoningLevel) -> &'static str {
    match level {
        ReasoningLevel::Minimal => "minimal",
        ReasoningLevel::Low => "low",
        ReasoningLevel::Medium => "medium",
        ReasoningLevel::High => "high",
        ReasoningLevel::XHigh => "xhigh",
        ReasoningLevel::Max
        | ReasoningLevel::Ultra
        | ReasoningLevel::Ultracode
        | ReasoningLevel::Ultrathink => "max",
    }
}

#[async_trait]
impl Harness for AcpHarness {
    fn id(&self) -> HarnessId {
        self.agent.id
    }
    fn display_name(&self) -> &str {
        self.agent.name
    }
    fn supports_steering(&self) -> bool {
        true
    }
    fn steering_mode(&self) -> SteeringMode {
        if self.agent.steers_mid_turn {
            SteeringMode::StepBoundary
        } else {
            SteeringMode::TurnBoundary
        }
    }
    fn reasoning_levels(&self) -> &[ReasoningLevel] {
        self.agent.reasoning_levels
    }
    fn stops_turn_in_place(&self) -> bool {
        true
    }
    fn applies_settings_live(&self, before: &TurnSettings, after: &TurnSettings) -> bool {
        let model_known = after.model == before.model
            || after
                .model
                .as_deref()
                .is_some_and(|model| !model.is_empty());
        let level_known = after.reasoning == before.reasoning || after.reasoning.is_some();
        model_known && level_known && after.model_options == before.model_options
    }
    fn installed(&self) -> bool {
        self.resolve_executable().is_ok()
    }
    fn executable_path(&self) -> Option<PathBuf> {
        self.resolve_executable().ok()
    }
    fn update_args(&self) -> &'static [&'static str] {
        self.agent.update_args
    }
    fn model_context(&self) -> Result<Option<crate::ModelContext>, HarnessError> {
        self.current_model_context().map(Some)
    }
    async fn model_catalog(&self, force: bool) -> Result<crate::ModelCatalog, HarnessError> {
        self.current_model_context()?.log();
        self.models_cache
            .get_with(
                force,
                || self.current_model_context().map(|context| context.key()),
                || self.discover_models(),
            )
            .await
    }

    async fn models(&self) -> Result<Vec<Model>, HarnessError> {
        self.resolve_executable()?;
        self.model_catalog(false)
            .await
            .map(|catalog| catalog.models)
    }

    /// Agents may list their commands a moment after the session opens.
    async fn commands(&self, cwd: &Path) -> Result<Vec<SlashCommand>, HarnessError> {
        let agent = self.agent;
        self.probe(
            Some(cwd),
            PROBE_TIMEOUT,
            "command discovery timed out",
            |client, mut incoming| async move {
                let (session, updates) = request_collecting(
                    &client,
                    &mut incoming,
                    "session/new",
                    json!({ "cwd": cwd, "mcpServers": [] }),
                )
                .await?;
                let session_id = session_id(agent, &session)?;
                let listed = |updates: &[Value]| {
                    updates_for(updates, &session_id)
                        .filter(|update| {
                            update.get("sessionUpdate").and_then(Value::as_str)
                                == Some("available_commands_update")
                        })
                        .last()
                        .map(normalize::commands)
                };
                if let Some(commands) = listed(&updates) {
                    return Ok(commands);
                }
                let deadline = tokio::time::sleep(COMMANDS_WAIT);
                tokio::pin!(deadline);
                let mut commands = Vec::new();
                loop {
                    tokio::select! {
                        message = incoming.recv() => match message {
                            Some(Incoming::Notification { method, params }) if method == "session/update" => {
                                if let Some(listed) = listed(std::slice::from_ref(&params)) {
                                    commands = listed;
                                    break;
                                }
                            }
                            Some(Incoming::Request { id, method, .. }) => {
                                client.respond_error(&id, -32601, &format!("unsupported method: {method}"));
                            }
                            Some(Incoming::Notification { .. }) => {}
                            Some(Incoming::Eof) | None => break,
                        },
                        _ = &mut deadline => break,
                    }
                }
                Ok(commands)
            },
        )
        .await
    }

    async fn external_sessions(&self, cwd: &Path) -> Result<Vec<ExternalSession>, HarnessError> {
        self.probe(
            Some(cwd),
            PROBE_TIMEOUT,
            "chat listing timed out",
            |client, mut incoming| async move {
                let mut sessions = Vec::new();
                let mut seen_cursors = HashSet::new();
                let mut cursor: Option<String> = None;
                loop {
                    let mut params = json!({ "cwd": cwd });
                    if let Some(cursor) = cursor.as_deref() {
                        params["cursor"] = Value::String(cursor.to_owned());
                    }
                    let (page, _) =
                        request_collecting(&client, &mut incoming, "session/list", params).await?;
                    sessions.extend(normalize::external_sessions(&page));
                    match page.get("nextCursor").and_then(Value::as_str) {
                        Some(next)
                            if sessions.len() < MAX_LISTED_SESSIONS
                                && seen_cursors.insert(next.to_owned()) =>
                        {
                            cursor = Some(next.to_owned());
                        }
                        _ => return Ok(sessions),
                    }
                }
            },
        )
        .await
    }

    async fn external_history(
        &self,
        cwd: &Path,
        session_id: &str,
    ) -> Result<Vec<AgentEvent>, HarnessError> {
        let command_output = self.agent.command_output;
        self.probe(
            Some(cwd),
            Duration::from_secs(60),
            "reading the chat timed out",
            |client, mut incoming| async move {
                let (_, updates) = request_collecting(
                    &client,
                    &mut incoming,
                    "session/load",
                    json!({ "sessionId": session_id, "cwd": cwd, "mcpServers": [] }),
                )
                .await?;
                let mut feed = Feed::history(command_output);
                let mut events: Vec<AgentEvent> = updates_for(&updates, session_id)
                    .flat_map(|update| feed.update(update))
                    .collect();
                events.extend(feed.finish());
                Ok(events)
            },
        )
        .await
    }

    async fn run(
        &self,
        request: RunRequest,
        controls: RunControls,
    ) -> Result<BoxStream<'static, Result<AgentEvent, HarnessError>>, HarnessError> {
        if request.fork.is_some() && request.resume.is_none() {
            return Err(HarnessError::Protocol(
                t!("{agent} can't copy a chat.", agent = self.agent.name).to_string(),
            ));
        }
        // Agents save every session in its folder, and one-off runs mustn't show up as saved chats.
        let cwd = if request.cwd.is_empty() || request.ephemeral {
            crate::executable::scratch_dir()
        } else {
            PathBuf::from(&request.cwd)
        };
        let connection = self.connect(&cwd).await?;
        let (event_tx, event_rx) = mpsc::channel::<Result<AgentEvent, HarnessError>>(256);
        tokio::spawn(run_session(Session {
            agent: self.agent,
            cwd,
            connection,
            event_tx,
            controls,
            request,
            interrupt_grace: self.interrupt_grace,
            kill_grace: self.kill_grace,
        }));
        Ok(futures::stream::unfold(event_rx, |mut rx| async move {
            rx.recv().await.map(|event| (event, rx))
        })
        .boxed())
    }
}

struct Session {
    agent: &'static AcpAgent,
    cwd: PathBuf,
    connection: Connection,
    event_tx: mpsc::Sender<Result<AgentEvent, HarnessError>>,
    controls: RunControls,
    request: RunRequest,
    interrupt_grace: Duration,
    kill_grace: Duration,
}

async fn send(tx: &mpsc::Sender<Result<AgentEvent, HarnessError>>, event: AgentEvent) -> bool {
    tx.send(Ok(event)).await.is_ok()
}

async fn send_final(tx: &mpsc::Sender<Result<AgentEvent, HarnessError>>, event: AgentEvent) {
    if !send(tx, event).await {
        tracing::debug!(target: "agent_harness::acp", "event receiver gone, dropping final event");
    }
}

struct Opened {
    session_id: String,
    config: Config,
    problems: Vec<String>,
}

async fn open_session(
    agent: &AcpAgent,
    client: &RpcClient,
    incoming: &mut mpsc::Receiver<Incoming>,
    request: &RunRequest,
    cwd: &Path,
) -> Result<Opened, HarnessError> {
    let (initialized, _) =
        request_collecting(client, incoming, "initialize", initialize_params()).await?;
    let capabilities = initialized.get("agentCapabilities");
    let can_resume = capabilities
        .and_then(|capabilities| capabilities.pointer("/sessionCapabilities/resume"))
        .is_some_and(|resume| !resume.is_null());
    let can_load = capabilities
        .and_then(|capabilities| capabilities.get("loadSession"))
        .and_then(Value::as_bool)
        == Some(true);
    let mut problems = Vec::new();
    let mut opened = None;
    if let Some(resume) = &request.resume {
        let params = json!({ "sessionId": resume, "cwd": cwd, "mcpServers": [] });
        // Loading replays the whole chat, which the chat already shows, so resuming is preferred.
        let method = if can_resume || !can_load {
            "session/resume"
        } else {
            "session/load"
        };
        match request_collecting(client, incoming, method, params).await {
            Ok((result, _)) => opened = Some((resume.clone(), Config::from_result(&result))),
            Err(error) if needs_sign_in(&error) => return Err(error),
            Err(error) => {
                tracing::debug!(target: "agent_harness::acp", agent = agent.name, "{method} failed (starting fresh): {error}");
                problems.push(
                    t!(
                        "{agent} couldn't reopen this chat, so it started a new one.",
                        agent = agent.name
                    )
                    .to_string(),
                );
            }
        }
    }
    let (session_id, mut config) = match opened {
        Some(opened) => opened,
        None => {
            let params = json!({ "cwd": cwd, "mcpServers": [] });
            let (result, _) = request_collecting(client, incoming, "session/new", params).await?;
            (session_id(agent, &result)?, Config::from_result(&result))
        }
    };

    let wanted = wanted_settings(
        request.model.as_deref(),
        agent.mode(request.permission.for_harness(agent.id)),
        request.reasoning,
    );
    let applied = config
        .apply_all(client, incoming, &session_id, &wanted)
        .await;
    problems.extend(applied.problems);
    Ok(Opened {
        session_id,
        config,
        problems,
    })
}

async fn prompt_blocks(text: &str, attachments: &[String]) -> (Value, Vec<String>) {
    use base64::Engine as _;
    let mut blocks = Vec::new();
    if !text.is_empty() {
        blocks.push(json!({ "type": "text", "text": text }));
    }
    let mut problems = Vec::new();
    for path in attachments {
        let name = Path::new(path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone());
        let bytes = match tokio::fs::read(path).await {
            Ok(bytes) => bytes,
            Err(error) => {
                problems.push(
                    t!("{name} wasn't sent: {error}", name = name, error = error).to_string(),
                );
                continue;
            }
        };
        let Some(mime_type) = crate::claude::image_media_type(Path::new(path), &bytes) else {
            problems.push(
                t!(
                    "{name} wasn't sent: only PNG, JPEG, GIF and WebP images are supported.",
                    name = name
                )
                .to_string(),
            );
            continue;
        };
        blocks.push(json!({
            "type": "image",
            "mimeType": mime_type,
            "data": base64::engine::general_purpose::STANDARD.encode(&bytes),
        }));
    }
    (Value::Array(blocks), problems)
}

type PromptFuture = BoxFuture<'static, (u64, Result<Value, HarnessError>)>;

fn send_prompt(
    client: &RpcClient,
    session_id: &str,
    generation: u64,
    blocks: Value,
) -> PromptFuture {
    client
        .request_now(
            "session/prompt",
            json!({ "sessionId": session_id, "prompt": blocks }),
        )
        .map(move |result| (generation, result))
        .boxed()
}

fn stop_status(
    agent: &AcpAgent,
    result: &Value,
    interrupted: bool,
) -> (DoneStatus, Option<String>) {
    let agent = agent.name;
    match result.get("stopReason").and_then(Value::as_str) {
        _ if interrupted => (DoneStatus::Interrupted, None),
        Some("cancelled") => (DoneStatus::Interrupted, None),
        Some("max_tokens") => (
            DoneStatus::Errored,
            Some(
                t!(
                    "{agent} stopped because the reply got too long.",
                    agent = agent
                )
                .to_string(),
            ),
        ),
        Some("max_turn_requests") => (
            DoneStatus::Errored,
            Some(
                t!(
                    "{agent} stopped because it reached its step limit.",
                    agent = agent
                )
                .to_string(),
            ),
        ),
        Some("refusal") => (
            DoneStatus::Errored,
            Some(t!("{agent} declined to continue.", agent = agent).to_string()),
        ),
        _ => (DoneStatus::Completed, None),
    }
}

fn usage_event(result: &Value) -> Option<AgentEvent> {
    let usage = result.get("usage")?;
    Some(AgentEvent::Usage {
        input_tokens: usage.get("inputTokens")?.as_u64()?,
        output_tokens: usage.get("outputTokens")?.as_u64()?,
    })
}

async fn run_session(session: Session) {
    let Session {
        agent,
        cwd,
        connection,
        event_tx,
        controls,
        request,
        interrupt_grace,
        kill_grace,
    } = session;
    let Connection {
        mut child,
        client,
        mut incoming,
        stderr_tail,
        stderr_read,
    } = connection;
    let RunControls {
        request_input,
        mut steering,
        interrupt,
        permission: mut permission_changes,
    } = controls;
    let request_input = Arc::new(request_input);
    let mut permission = request.permission.for_harness(agent.id);
    let mut permission_open = true;

    let opened = tokio::select! {
        opened = open_session(agent, &client, &mut incoming, &request, &cwd) => {
            opened.map_err(|error| signed_out(agent, error))
        }
        _ = interrupt.cancelled() => {
            send_final(&event_tx, AgentEvent::Done {
                status: DoneStatus::Interrupted,
                result: None,
                error: None,
                session_id: None,
            })
            .await;
            shutdown_child(&mut child, kill_grace).await;
            return;
        }
    };
    let Opened {
        session_id,
        mut config,
        problems,
    } = match opened {
        Ok(opened) => opened,
        Err(error) => {
            send_final(
                &event_tx,
                AgentEvent::Done {
                    status: DoneStatus::Errored,
                    result: None,
                    error: Some(error.to_string()),
                    session_id: request.resume.clone(),
                },
            )
            .await;
            shutdown_child(&mut child, kill_grace).await;
            return;
        }
    };

    let mut feed = Feed::live(uuid::Uuid::new_v4().to_string(), agent.command_output);
    let started = AgentEvent::SessionStarted {
        harness: agent.id,
        model: config
            .current(MODEL)
            .map(str::to_owned)
            .or_else(|| request.model.clone())
            .unwrap_or_default(),
        tools: Vec::new(),
        cwd: request.cwd.clone(),
        session_id: session_id.clone(),
        assistant_message_id: feed.message_id().to_owned(),
    };
    if !send(&event_tx, started).await {
        shutdown_child(&mut child, kill_grace).await;
        return;
    }
    let (blocks, attachment_problems) = prompt_blocks(&request.prompt, &request.attachments).await;
    for message in problems.into_iter().chain(attachment_problems) {
        if !send(&event_tx, AgentEvent::Error { message }).await {
            shutdown_child(&mut child, kill_grace).await;
            return;
        }
    }

    let mut generation = 0;
    let mut prompts: FuturesUnordered<PromptFuture> = FuturesUnordered::new();
    prompts.push(send_prompt(&client, &session_id, generation, blocks));
    let mut queued: VecDeque<Value> = VecDeque::new();
    let mut running = true;
    let mut steering_open = true;
    let mut interrupted = false;
    let mut interrupt_sent = false;
    let mut stopping = false;
    let mut stop_ack = None;
    let mut stop_kill_starts_at = None;
    let mut stopped_turn_may_trail = false;
    let mut stopped_turn_settles_at: Option<tokio::time::Instant> = None;
    let mut pending_settings: Option<TurnSettings> = None;
    let mut unanswered = CancellationToken::new();
    let mut done_sent = false;
    let mut escalation: Option<tokio::task::JoinHandle<()>> = None;

    macro_rules! forward_update {
        ($update:expr, $label:lifetime) => {{
            let update: &Value = $update;
            config.note_update(update);
            if stopped_turn_may_trail && normalize::is_turn_content(update) {
                if stopped_turn_settles_at.is_some() {
                    stopped_turn_settles_at =
                        Some(tokio::time::Instant::now() + STOPPED_TURN_SETTLE);
                }
            } else {
                for event in feed.update(update) {
                    if !send(&event_tx, event).await {
                        break $label;
                    }
                }
            }
        }};
    }

    macro_rules! start_prompt {
        ($blocks:expr, $label:lifetime) => {{
            if !running && let Some(settings) = pending_settings.take() {
                let wanted = wanted_settings(settings.model.as_deref(), None, settings.reasoning);
                let applied = tokio::select! {
                    applied = config.apply_all(&client, &mut incoming, &session_id, &wanted) => applied,
                    _ = interrupt.cancelled() => Applied::default(),
                };
                for update in updates_for(&applied.updates_meanwhile, &session_id) {
                    forward_update!(update, $label);
                }
                for message in applied.problems {
                    if !send(&event_tx, AgentEvent::Error { message }).await {
                        break $label;
                    }
                }
            }
            generation += 1;
            prompts.push(send_prompt(&client, &session_id, generation, $blocks));
            running = true;
            done_sent = false;
            stopped_turn_may_trail = false;
            stopped_turn_settles_at = None;
            let (previous, next) = feed.rotate();
            if !send(
                &event_tx,
                AgentEvent::Steered {
                    assistant_message_id: Some(previous),
                    next_assistant_message_id: Some(next),
                },
            )
            .await
            {
                break $label;
            }
        }};
    }

    'main: loop {
        tokio::select! {
            // The prompt's reply skips the queue, so updates sent before it must be read first.
            biased;
            message = incoming.recv() => match message {
                Some(Incoming::Notification { method, params }) => {
                    if method != "session/update"
                        || params.get("sessionId").and_then(Value::as_str) != Some(session_id.as_str())
                    {
                        continue;
                    }
                    let Some(update) = params.get("update") else { continue };
                    forward_update!(update, 'main);
                }
                Some(Incoming::Request { id, method, params }) => {
                    if method == "session/request_permission" {
                        answer_permission(
                            agent,
                            &client,
                            id,
                            &params,
                            permission.skips_prompts(),
                            &request_input,
                            &unanswered,
                        );
                    } else {
                        tracing::debug!(target: "agent_harness::acp", agent = agent.name, "unhandled server request: {method}");
                        client.respond_error(&id, -32601, &format!("unsupported method: {method}"));
                    }
                }
                Some(Incoming::Eof) | None => break 'main,
            },

            Some((finished, result)) = prompts.next(), if !prompts.is_empty() => {
                // A steer starts a new prompt, which ends the one it interrupted.
                if finished != generation {
                    continue;
                }
                running = false;
                let cancelled = interrupted || stopping;
                let (status, error) = match &result {
                    Ok(result) => stop_status(agent, result, cancelled),
                    Err(_) if cancelled => (DoneStatus::Interrupted, None),
                    Err(error) => (DoneStatus::Errored, Some(sign_in_message(agent, error))),
                };
                if let Some(usage) = result.as_ref().ok().and_then(usage_event)
                    && !send(&event_tx, usage).await
                {
                    break 'main;
                }
                if !send(&event_tx, feed.finish_message()).await {
                    break 'main;
                }
                done_sent = true;
                if stopping
                    && !interrupted
                    && !crate::confirm_stop(&mut stop_ack, stop_kill_starts_at, &mut escalation)
                {
                    interrupted = true;
                }
                if !send(&event_tx, AgentEvent::Done {
                    status,
                    result: None,
                    error,
                    session_id: Some(session_id.clone()),
                })
                .await
                    || interrupted
                {
                    break 'main;
                }
                if stopping {
                    stopping = false;
                    stopped_turn_may_trail = true;
                    stopped_turn_settles_at =
                        Some(tokio::time::Instant::now() + STOPPED_TURN_SETTLE);
                    feed.close_tools();
                    if let Some(handle) = escalation.take() {
                        handle.abort();
                    }
                }
                if let Some(blocks) = queued.pop_front() {
                    start_prompt!(blocks, 'main);
                } else if !steering_open {
                    break 'main;
                }
            },

            // A message sent after a stop waits here until the stopped turn has ended.
            steer = steering.recv(), if steering_open && !interrupted && !stopping => match steer {
                Some(Steer::StopTurn(ack)) => {
                    if running {
                        stopping = true;
                        stop_ack = Some(ack);
                        stop_kill_starts_at = Some(tokio::time::Instant::now() + interrupt_grace);
                        queued.clear();
                        unanswered.cancel();
                        unanswered = CancellationToken::new();
                        client.notify("session/cancel", Some(json!({ "sessionId": session_id })));
                        escalation = crate::process::escalate_interrupt(&child, interrupt_grace, kill_grace);
                    }
                }
                Some(Steer::Message(message)) => {
                    let (blocks, problems) = prompt_blocks(&message.prompt, &message.attachments).await;
                    for message in problems {
                        if !send(&event_tx, AgentEvent::Error { message }).await {
                            break 'main;
                        }
                    }
                    if message.settings.is_some() {
                        pending_settings = message.settings;
                    }
                    if (running && !agent.steers_mid_turn) || stopped_turn_settles_at.is_some() {
                        queued.push_back(blocks);
                        continue;
                    }
                    start_prompt!(blocks, 'main);
                }
                None => {
                    steering_open = false;
                    if !running {
                        break 'main;
                    }
                }
            },

            _ = tokio::time::sleep_until(
                stopped_turn_settles_at.unwrap_or_else(tokio::time::Instant::now)
            ), if stopped_turn_settles_at.is_some() => {
                stopped_turn_settles_at = None;
                if let Some(blocks) = queued.pop_front() {
                    start_prompt!(blocks, 'main);
                }
            },

            changed = permission_changes.changed(), if permission_open => match changed {
                Ok(()) => {
                    permission = permission_changes.borrow_and_update().for_harness(agent.id);
                    if let Some(mode) = agent.mode(permission)
                        && config.current(MODE) != Some(mode)
                        && config.offers(MODE, mode)
                        && let Some((method, params)) = config.set_request(&session_id, MODE, mode)
                    {
                        config.assume(MODE, mode);
                        let change = client.request_now(method, params);
                        let agent = agent.name;
                        tokio::spawn(async move {
                            if let Err(error) = change.await {
                                tracing::warn!(target: "agent_harness::acp", agent, "changing the mode failed: {error}");
                            }
                        });
                    }
                }
                Err(_) => permission_open = false,
            },

            _ = interrupt.cancelled(), if !interrupt_sent => {
                interrupt_sent = true;
                interrupted = true;
                if running {
                    if !stopping {
                        client.notify("session/cancel", Some(json!({ "sessionId": session_id })));
                        escalation = crate::process::escalate_interrupt(&child, interrupt_grace, kill_grace);
                    }
                } else {
                    break 'main;
                }
            },

            _ = event_tx.closed() => break 'main,
        }
    }

    let cancelled = interrupted || stopping;
    if !event_tx.is_closed() && !done_sent {
        let mut error = None;
        if !cancelled {
            let exit = tokio::time::timeout(Duration::from_secs(1), child.wait())
                .await
                .ok()
                .and_then(Result::ok);
            if let Some(stderr_read) = stderr_read
                && tokio::time::timeout(Duration::from_secs(1), stderr_read)
                    .await
                    .is_err()
            {
                tracing::debug!(target: "agent_harness::acp", agent = agent.name, "stderr still open after exit");
            }
            error = Some(crate::crash_message(agent.name, exit, &stderr_tail));
        }
        send_final(
            &event_tx,
            AgentEvent::Done {
                status: if cancelled {
                    DoneStatus::Interrupted
                } else {
                    DoneStatus::Errored
                },
                result: None,
                error,
                session_id: Some(session_id.clone()),
            },
        )
        .await;
    }
    shutdown_child(&mut child, kill_grace).await;
    if let Some(handle) = escalation {
        handle.abort();
    }
}

type RequestInputFn = Box<
    dyn Fn(Vec<UserInputQuestion>) -> tokio::sync::oneshot::Receiver<Vec<UserInputAnswer>>
        + Send
        + Sync,
>;

struct PermissionChoices {
    allow: Option<String>,
    deny: Option<String>,
}

/// Wu answers permissions with exactly Allow or Deny, so remembered choices map onto those two.
fn permission_choices(params: &Value) -> PermissionChoices {
    let options: Vec<(&str, &str)> = params
        .get("options")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|option| {
            Some((
                option.get("optionId")?.as_str()?,
                option.get("kind").and_then(Value::as_str).unwrap_or(""),
            ))
        })
        .collect();
    let pick = |once: &str, prefix: &str| {
        options
            .iter()
            .find(|(_, kind)| *kind == once)
            .or_else(|| options.iter().find(|(_, kind)| kind.starts_with(prefix)))
            .map(|(id, _)| (*id).to_owned())
    };
    PermissionChoices {
        allow: pick("allow_once", "allow"),
        deny: pick("reject_once", "reject"),
    }
}

fn permission_question(agent: &AcpAgent, params: &Value) -> UserInputQuestion {
    let tool = params.get("toolCall").unwrap_or(&Value::Null);
    let agent = agent.name;
    let question = match normalize::tool_call(tool) {
        crate::ToolCall::Exec { command } if !command.is_empty() => t!(
            "{agent} wants to run `{command}`. Allow it?",
            agent = agent,
            command = command
        ),
        crate::ToolCall::WriteFile { path, .. } | crate::ToolCall::EditFile { path, .. } => t!(
            "{agent} wants to change {path}. Allow it?",
            agent = agent,
            path = path
        ),
        _ => match tool
            .get("title")
            .and_then(Value::as_str)
            .filter(|title| !title.trim().is_empty())
        {
            Some(title) => t!(
                "{agent} wants to use {tool}. Allow it?",
                agent = agent,
                tool = title
            ),
            None => t!("{agent} wants to use a tool. Allow it?", agent = agent),
        },
    };
    UserInputQuestion {
        id: uuid::Uuid::new_v4().to_string(),
        header: crate::claude::PERMISSION_HEADER.to_owned(),
        question: question.to_string(),
        options: vec![
            crate::claude::PERMISSION_ALLOW.to_owned(),
            crate::claude::PERMISSION_DENY.to_owned(),
        ],
        prefill: None,
        multiline: false,
        accepts_typed_answer: false,
        multi_select: false,
    }
}

fn selected(option_id: &str) -> Value {
    json!({ "outcome": { "outcome": "selected", "optionId": option_id } })
}

/// Every request needs a reply, or the turn waits on it forever.
fn answer_permission(
    agent: &AcpAgent,
    client: &RpcClient,
    id: Value,
    params: &Value,
    auto_approve: bool,
    request_input: &Arc<RequestInputFn>,
    unanswered: &CancellationToken,
) {
    let choices = permission_choices(params);
    let withdrawn = || json!({ "outcome": { "outcome": "cancelled" } });
    let refusal = choices.deny.as_deref().map_or_else(withdrawn, selected);
    let Some(allow) = choices.allow else {
        client.respond(&id, refusal);
        return;
    };
    if auto_approve {
        client.respond(&id, selected(&allow));
        return;
    }
    let question = permission_question(agent, params);
    let client = client.clone();
    let request_input = Arc::clone(request_input);
    let unanswered = unanswered.clone();
    tokio::spawn(async move {
        let answers = tokio::select! {
            answers = (request_input)(vec![question.clone()]) => answers.unwrap_or_default(),
            _ = unanswered.cancelled() => {
                client.respond(&id, withdrawn());
                return;
            }
        };
        let allowed = answers
            .iter()
            .filter(|answer| answer.question_id == question.id)
            .flat_map(|answer| &answer.labels)
            .any(|label| label.eq_ignore_ascii_case(crate::claude::PERMISSION_ALLOW));
        client.respond(&id, if allowed { selected(&allow) } else { refusal });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const AGENT: &AcpAgent = &crate::antigravity::ANTIGRAVITY;

    #[test]
    fn permission_questions_are_allow_or_deny_mapped_to_the_agents_options() {
        let params = json!({
            "toolCall": {
                "toolCallId": "c", "name": "run_command", "kind": "execute",
                "title": "ls -1", "rawInput": { "CommandLine": "ls -1" },
            },
            "options": [
                { "optionId": "always", "name": "Always allow", "kind": "allow_always" },
                { "optionId": "allow", "name": "Allow", "kind": "allow_once" },
                { "optionId": "deny", "name": "Deny", "kind": "reject_once" },
            ],
        });
        let question = permission_question(AGENT, &params);
        assert_eq!(question.options, ["Allow", "Deny"]);
        assert_eq!(question.header, crate::claude::PERMISSION_HEADER);
        assert!(question.question.starts_with("Antigravity "));
        assert!(question.question.contains("`ls -1`"));
        let choices = permission_choices(&params);
        assert_eq!(choices.allow.as_deref(), Some("allow"));
        assert_eq!(choices.deny.as_deref(), Some("deny"));

        let remembered_only = permission_choices(&json!({
            "options": [
                { "optionId": "yes", "kind": "allow_always" },
                { "optionId": "no", "kind": "reject_always" },
            ],
        }));
        assert_eq!(remembered_only.allow.as_deref(), Some("yes"));
        assert_eq!(remembered_only.deny.as_deref(), Some("no"));
    }

    #[test]
    fn stop_reasons_map_to_done_statuses() {
        let status = |reason: &str, interrupted| {
            stop_status(AGENT, &json!({ "stopReason": reason }), interrupted).0
        };
        assert_eq!(status("end_turn", false), DoneStatus::Completed);
        assert_eq!(status("cancelled", false), DoneStatus::Interrupted);
        assert_eq!(status("end_turn", true), DoneStatus::Interrupted);
        assert_eq!(status("max_tokens", false), DoneStatus::Errored);
        assert_eq!(status("refusal", false), DoneStatus::Errored);
    }

    #[test]
    fn config_options_are_found_by_category_and_choices_skip_blanks() {
        let config = Config::from_result(&json!({
            "configOptions": [
                {
                    "id": "model", "category": "model", "currentValue": "gemini-3.8-flash",
                    "options": [
                        { "value": "gemini-3.8-flash", "name": "Gemini 3.8 Flash" },
                        { "value": "claude-opus-4-6-thinking", "name": "Claude Opus 4.6 (Thinking)" },
                        { "value": " ", "name": "Blank" },
                    ],
                },
                {
                    "id": "level", "category": "thought_level", "currentValue": "high",
                    "options": [{ "value": "low" }, { "value": "high", "name": "High" }],
                },
            ],
            "models": {
                "availableModels": [{ "modelId": "gemini-3.8-flash-high", "name": "Variant" }],
                "currentModelId": "gemini-3.8-flash-high",
            },
        }));
        assert_eq!(config.current(MODEL), Some("gemini-3.8-flash"));
        assert_eq!(config.choices(MODEL).len(), 2);
        assert!(config.offers(THOUGHT_LEVEL, "low"));
        assert!(!config.offers(MODE, "auto"));
        assert_eq!(
            config.choices(THOUGHT_LEVEL),
            [
                ("low".to_owned(), "low".to_owned()),
                ("high".to_owned(), "High".to_owned())
            ]
        );
        assert_eq!(
            config.set_request("s", THOUGHT_LEVEL, "low"),
            Some((
                "session/set_config_option",
                json!({ "sessionId": "s", "configId": "level", "value": "low" })
            ))
        );
    }

    #[test]
    fn older_agents_set_models_and_modes_with_their_own_requests() {
        let mut config = Config::from_result(&json!({
            "models": {
                "availableModels": [{ "modelId": "m1", "name": "One" }, { "modelId": "m2" }],
                "currentModelId": "m1",
            },
            "modes": {
                "availableModes": [{ "id": "ask", "name": "Ask" }, { "id": "yolo" }],
                "currentModeId": "ask",
            },
        }));
        assert_eq!(
            config.choices(MODEL),
            [
                ("m1".to_owned(), "One".to_owned()),
                ("m2".to_owned(), "m2".to_owned())
            ]
        );
        assert_eq!(
            config.set_request("s", MODEL, "m2"),
            Some((
                "session/set_model",
                json!({ "sessionId": "s", "modelId": "m2" })
            ))
        );
        assert_eq!(
            config.set_request("s", MODE, "yolo"),
            Some((
                "session/set_mode",
                json!({ "sessionId": "s", "modeId": "yolo" })
            ))
        );
        assert_eq!(config.set_request("s", THOUGHT_LEVEL, "low"), None);
        config.note_update(
            &json!({ "sessionUpdate": "current_mode_update", "currentModeId": "yolo" }),
        );
        assert_eq!(config.current(MODE), Some("yolo"));
    }

    #[test]
    fn sign_in_errors_use_the_agents_hint() {
        let error = signed_out(
            AGENT,
            HarnessError::Protocol("session/new: Authentication required (code -32000)".into()),
        );
        assert!(error.to_string().contains("agy"));
        let other = signed_out(AGENT, HarnessError::Protocol("session/new: busy".into()));
        assert_eq!(
            other.to_string(),
            HarnessError::Protocol("session/new: busy".into()).to_string()
        );
    }
}
