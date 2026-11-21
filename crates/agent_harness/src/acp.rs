//! Drives ACP-speaking coding agents (omp, pi) over the Agent Client Protocol: JSON-RPC over stdio.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use futures::StreamExt;
use futures::stream::BoxStream;
use i18n::t;
use serde_json::{Value, json};
use tokio::io::AsyncBufReadExt;
use tokio::sync::mpsc;

use crate::{
    AgentEvent, DoneStatus, HarnessId, Model, ModelCatalog, ModelContext, PermissionMode,
    ReasoningLevel, RunRequest, SteeringMode, TodoItem, TodoStatus, ToolCall, ToolDiff,
    UserInputAnswer, UserInputQuestion,
};

use crate::jsonrpc::{Incoming, RpcClient};
use crate::process::{Child, Stdio};
use crate::{Harness, HarnessError, RunControls, SteerMessage, StderrTail, shutdown_child};

const PROTOCOL_VERSION: i64 = 1;
const MODEL_CONFIG_CATEGORY: &str = "model";
const THINKING_CONFIG_CATEGORY: &str = "thought_level";
const STEERING_METHOD: &str = "_session/steering";

const INTERRUPT_GRACE: Duration = Duration::from_secs(2);
const KILL_GRACE: Duration = Duration::from_secs(3);

/// The ACP-speaking agents Wu supports. They share one wire protocol and one
/// harness; the agent only decides the entry point and a few capability quirks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpAgent {
    /// oh-my-pi, whose native `omp acp` mode speaks ACP.
    Omp,
    /// pi, driven through the third-party `pi-acp` adapter.
    Pi,
}

impl AcpAgent {
    pub fn harness_id(self) -> HarnessId {
        match self {
            Self::Omp => HarnessId::Omp,
            Self::Pi => HarnessId::Pi,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Omp => "omp",
            Self::Pi => "pi",
        }
    }

    /// Extra CLI args after the executable that enter its ACP mode.
    pub fn spawn_args(self) -> &'static [&'static str] {
        match self {
            Self::Omp => &["acp"],
            Self::Pi => &[],
        }
    }

    fn executable_name(self) -> &'static str {
        match self {
            Self::Omp => "omp",
            Self::Pi => "pi-acp",
        }
    }

    fn env_override(self) -> &'static str {
        match self {
            Self::Omp => "OMP_EXECUTABLE",
            Self::Pi => "PI_EXECUTABLE",
        }
    }

    fn searched_paths_hint(self) -> &'static str {
        match self {
            Self::Omp => {
                "searched PATH, ~/.local/bin, ~/.omp/bin, ~/.npm-global/bin, \
                 /opt/homebrew/bin and /usr/local/bin for `omp`"
            }
            Self::Pi => {
                "searched PATH, ~/.local/bin and /opt/homebrew/bin for `pi-acp` \
                 (npm install -g pi-acp)"
            }
        }
    }

    pub fn resolve_executable() -> Option<PathBuf> {
        Self::Omp.resolve_executable_for()
    }

    fn resolve_executable_for(self) -> Option<PathBuf> {
        if let Some(p) = std::env::var_os(self.env_override()).filter(|p| !p.is_empty()) {
            return crate::executable::validate_native_override(&PathBuf::from(p)).ok();
        }
        let name = self.executable_name();
        let mut extra = Vec::new();
        if let Some(home) = crate::executable::home_dir() {
            extra.push(home.join(".local").join("bin").join(name));
            if self == Self::Omp {
                extra.push(home.join(".omp").join("bin").join(name));
                extra.push(home.join(".npm-global").join("bin").join(name));
            }
        }
        extra.push(PathBuf::from("/opt/homebrew/bin").join(name));
        extra.push(PathBuf::from("/usr/local/bin").join(name));
        crate::executable::find_on_paths(name, extra)
    }

    /// omp injects mid-turn messages through its `_session/steering` extension;
    /// the pi adapter has no such method, so steering degrades to the next turn.
    pub fn steering_method(self) -> Option<&'static str> {
        match self {
            Self::Omp => Some(STEERING_METHOD),
            Self::Pi => None,
        }
    }

    pub fn thinking_config_id(self) -> &'static str {
        match self {
            Self::Omp => "thinking",
            Self::Pi => "thought_level",
        }
    }

    fn crash_name(self) -> &'static str {
        match self {
            Self::Omp => "omp acp",
            Self::Pi => "pi-acp",
        }
    }
}

pub struct AcpHarness {
    agent: AcpAgent,
    models_cache: crate::catalog::Catalog,
    executable: Option<PathBuf>,
    interrupt_grace: Duration,
    kill_grace: Duration,
}

impl AcpHarness {
    pub fn omp() -> Self {
        Self::with_agent(AcpAgent::Omp)
    }

    pub fn pi() -> Self {
        Self::with_agent(AcpAgent::Pi)
    }

    fn with_agent(agent: AcpAgent) -> Self {
        Self {
            agent,
            models_cache: crate::catalog::Catalog::default(),
            executable: None,
            interrupt_grace: INTERRUPT_GRACE,
            kill_grace: KILL_GRACE,
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
        if let Some(p) = &self.executable {
            return crate::executable::validate_native_override(p);
        }
        if let Some(p) = std::env::var_os(self.agent.env_override())
            && !p.is_empty()
        {
            return crate::executable::validate_native_override(&PathBuf::from(p));
        }
        self.agent
            .resolve_executable_for()
            .ok_or_else(|| {
                HarnessError::agent_missing(self.agent.harness_id(), self.agent.searched_paths_hint())
            })
    }

    /// One-shot catalog discovery: a throwaway `omp acp` session reports the
    /// model and thinking options its local configuration provides.
    async fn discover_models(&self) -> Result<Vec<Model>, HarnessError> {
        let exe = self.resolve_executable()?;
        let mut cmd = crate::process::command(&exe);
        for arg in self.agent.spawn_args() {
            cmd.arg(arg);
        }
        cmd.current_dir(crate::executable::scratch_dir());
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                HarnessError::NotInstalled(crate::executable::binary_hint(&exe))
            } else {
                HarnessError::Io(e)
            }
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| HarnessError::Protocol("acp child has no stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| HarnessError::Protocol("acp child has no stdout".into()))?;
        let (client, mut incoming) = RpcClient::new(stdin, stdout);

        let discovery = async {
            initialize(&client).await?;
            let response = client
                .request(
                    "session/new",
                    json!({ "cwd": crate::executable::scratch_dir(), "mcpServers": [] }),
                )
                .await?;
            let models = catalog_models(response.get("configOptions").unwrap_or(&Value::Null));
            // Bootstrap updates may still be streaming; they carry no model data we need.
            drain_notifications(&mut incoming).await;
            Ok(models)
        };
        let result = discovery.await;
        if let Err(error) = child.kill().await {
            tracing::debug!(target: "agent_harness::acp", "killing discovery child failed: {error}");
        }
        result
    }
}

/// Reads notifications until the child goes quiet, so bootstrap traffic doesn't linger.
async fn drain_notifications(incoming: &mut mpsc::Receiver<Incoming>) {
    while let Ok(Some(Incoming::Notification { .. })) =
        tokio::time::timeout(Duration::from_millis(250), incoming.recv()).await
    {}
}

fn catalog_models(config_options: &Value) -> Vec<Model> {
    let mut models = Vec::new();
    let mut thinking_levels = Vec::new();
    for option in config_options.as_array().into_iter().flatten() {
        let category = option.get("category").and_then(Value::as_str);
        let choices = option.get("options").and_then(Value::as_array);
        match category {
            Some("model") => {
                for choice in choices.into_iter().flatten() {
                    let Some(id) = choice.get("value").and_then(Value::as_str) else {
                        continue;
                    };
                    let label = choice
                        .get("name")
                        .and_then(Value::as_str)
                        .filter(|name| !name.is_empty())
                        .unwrap_or(id);
                    let description = choice
                        .get("description")
                        .and_then(Value::as_str)
                        .filter(|d| !d.is_empty())
                        .map(str::to_owned);
                    models.push(Model {
                        id: id.to_owned(),
                        label: label.to_owned(),
                        description,
                        reasoning_levels: Vec::new(),
                        options: Vec::new(),
                    });
                }
            }
            Some("thought_level") => {
                for choice in choices.into_iter().flatten() {
                    if let Some(value) = choice.get("value").and_then(Value::as_str) {
                        thinking_levels.push(value.to_owned());
                    }
                }
            }
            _ => {}
        }
    }
    let reasoning: Vec<ReasoningLevel> = thinking_levels
        .iter()
        .filter_map(|value| thinking_level(value))
        .collect();
    let reasoning = if reasoning.is_empty() {
        vec![
            ReasoningLevel::Low,
            ReasoningLevel::Medium,
            ReasoningLevel::High,
        ]
    } else {
        reasoning
    };
    for model in &mut models {
        model.reasoning_levels = reasoning.clone();
    }
    models
}

fn thinking_level(value: &str) -> Option<ReasoningLevel> {
    Some(match value {
        "off" | "minimal" => ReasoningLevel::Minimal,
        "low" => ReasoningLevel::Low,
        "medium" => ReasoningLevel::Medium,
        "high" => ReasoningLevel::High,
        "xhigh" => ReasoningLevel::XHigh,
        "max" => ReasoningLevel::Max,
        "ultra" => ReasoningLevel::Ultra,
        _ => return None,
    })
}

fn thinking_value(level: ReasoningLevel) -> Option<&'static str> {
    Some(match level {
        ReasoningLevel::Minimal => "off",
        ReasoningLevel::Low => "low",
        ReasoningLevel::Medium => "medium",
        ReasoningLevel::High => "high",
        ReasoningLevel::XHigh => "xhigh",
        _ => return None,
    })
}

async fn initialize(client: &RpcClient) -> Result<(), HarnessError> {
    let response = client
        .request(
            "initialize",
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "clientCapabilities": {
                    "fs": { "readTextFile": true, "writeTextFile": true },
                },
                "clientInfo": {
                    "name": "wu",
                    "title": "Wu",
                    "version": env!("CARGO_PKG_VERSION"),
                },
            }),
        )
        .await?;
    // omp's `agent` auth method validates credentials already stored under ~/.omp;
    // authenticating is a no-op but keeps the session conformant with the flow
    // the agent advertised.
    let auth_method = response
        .get("authMethods")
        .and_then(Value::as_array)
        .and_then(|methods| methods.first())
        .and_then(|method| method.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    if let Some(method_id) = auth_method {
        client
            .request("authenticate", json!({ "methodId": method_id }))
            .await?;
    }
    Ok(())
}

async fn prompt_blocks(text: &str, attachments: &[String]) -> Result<Value, HarnessError> {
    let mut blocks = vec![json!({ "type": "text", "text": text })];
    let mut seen = HashSet::new();
    for path in attachments {
        if !seen.insert(path.clone()) {
            continue;
        }
        let bytes = tokio::fs::read(path).await.map_err(HarnessError::Io)?;
        blocks.push(json!({
            "type": "image",
            "data": base64::engine::general_purpose::STANDARD.encode(bytes),
            "mimeType": image_mime_type(path),
        }));
    }
    Ok(Value::Array(blocks))
}

fn image_mime_type(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else {
        "image/png"
    }
}

fn new_message_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn rotate(id: &mut String) -> (String, String) {
    let previous = std::mem::replace(id, new_message_id());
    (previous, id.clone())
}

async fn send(tx: &mpsc::Sender<Result<AgentEvent, HarnessError>>, event: AgentEvent) -> bool {
    tx.send(Ok(event)).await.is_ok()
}

async fn send_final(tx: &mpsc::Sender<Result<AgentEvent, HarnessError>>, event: AgentEvent) {
    if !send(tx, event).await {
        tracing::debug!(target: "agent_harness::acp", "event receiver gone, dropping final event");
    }
}

fn errored(message: String, session_id: Option<String>) -> AgentEvent {
    AgentEvent::Done {
        status: DoneStatus::Errored,
        result: None,
        error: Some(message),
        session_id,
    }
}

fn interrupted(session_id: Option<String>) -> AgentEvent {
    AgentEvent::Done {
        status: DoneStatus::Interrupted,
        result: None,
        error: None,
        session_id,
    }
}

#[async_trait]
impl Harness for AcpHarness {
    fn id(&self) -> HarnessId {
        self.agent.harness_id()
    }
    fn display_name(&self) -> &str {
        self.agent.display_name()
    }
    fn supports_steering(&self) -> bool {
        self.agent.steering_method().is_some()
    }
    fn steering_mode(&self) -> SteeringMode {
        SteeringMode::StepBoundary
    }
    fn reasoning_levels(&self) -> &[ReasoningLevel] {
        // Statically the safe subset: thinking levels vary per model, and an
        // unsupported value is tolerated by the config-option fallback in run_session.
        static LEVELS: [ReasoningLevel; 5] = [
            ReasoningLevel::Minimal,
            ReasoningLevel::Low,
            ReasoningLevel::Medium,
            ReasoningLevel::High,
            ReasoningLevel::XHigh,
        ];
        &LEVELS
    }
    fn installed(&self) -> bool {
        self.resolve_executable().is_ok()
    }
    fn executable_path(&self) -> Option<PathBuf> {
        self.resolve_executable().ok()
    }
    fn model_context(&self) -> Result<Option<ModelContext>, HarnessError> {
        let Ok(path) = self.resolve_executable() else {
            return Ok(None);
        };
        let context = crate::model_context::context(self.agent.harness_id(), &path, &[])?;
        Ok(Some(context))
    }
    fn fallback_models(&self) -> Vec<Model> {
        Vec::new()
    }
    async fn models(&self) -> Result<Vec<Model>, HarnessError> {
        self.model_catalog(false).await.map(|catalog| catalog.models)
    }
    async fn model_catalog(&self, force: bool) -> Result<ModelCatalog, HarnessError> {
        self.models_cache
            .get_with(
                force,
                || {
                    self.model_context()?.map_or_else(
                        || Err(HarnessError::NotInstalled("omp".into())),
                        |context| Ok(context.key()),
                    )
                },
                || self.discover_models(),
            )
            .await
    }
    async fn run(
        &self,
        request: RunRequest,
        controls: RunControls,
    ) -> Result<BoxStream<'static, Result<AgentEvent, HarnessError>>, HarnessError> {
        let exe = self.resolve_executable()?;
        let mut cmd = crate::process::command(&exe);
        for arg in self.agent.spawn_args() {
            cmd.arg(arg);
        }
        if request.ephemeral {
            cmd.current_dir(crate::executable::scratch_dir());
        } else {
            cmd.current_dir(&request.cwd);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                HarnessError::NotInstalled(crate::executable::binary_hint(&exe))
            } else {
                HarnessError::Io(e)
            }
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| HarnessError::Protocol("acp child has no stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| HarnessError::Protocol("acp child has no stdout".into()))?;
        let stderr_tail = StderrTail::default();
        if let Some(stderr) = child.stderr.take() {
            let tail = stderr_tail.clone();
            tokio::spawn(async move {
                let mut lines = tokio::io::BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::debug!(target: "agent_harness::acp", "stderr: {line}");
                    tail.push(&line);
                }
            });
        }

        let (client, incoming) = RpcClient::new(stdin, stdout);
        let (event_tx, event_rx) = mpsc::channel::<Result<AgentEvent, HarnessError>>(256);
        tokio::spawn(run_session(Session {
            agent: self.agent,
            child,
            client,
            incoming,
            event_tx,
            controls,
            request,
            interrupt_grace: self.interrupt_grace,
            kill_grace: self.kill_grace,
            stderr_tail,
        }));
        Ok(futures::stream::unfold(event_rx, |mut rx| async move {
            rx.recv().await.map(|event| (event, rx))
        })
        .boxed())
    }
}

struct Session {
    agent: AcpAgent,
    child: Child,
    client: RpcClient,
    incoming: mpsc::Receiver<Incoming>,
    event_tx: mpsc::Sender<Result<AgentEvent, HarnessError>>,
    controls: RunControls,
    request: RunRequest,
    interrupt_grace: Duration,
    kill_grace: Duration,
    stderr_tail: StderrTail,
}

async fn run_session(session: Session) {
    let Session {
        agent,
        mut child,
        client,
        mut incoming,
        event_tx,
        controls,
        request,
        interrupt_grace,
        kill_grace,
        stderr_tail,
    } = session;
    let RunControls {
        request_input,
        mut steering,
        interrupt,
        permission: mut permission_changes,
    } = controls;
    let request_input = Arc::new(request_input);

    let mut permission = request.permission.for_harness(agent.harness_id());
    let mut steering_open = true;
    let mut permission_open = true;
    let cwd = request.cwd.clone();
    let resume = request.resume.clone();
    let setup = async {
        initialize(&client).await?;
        // `session/new` echoes the id it created, but `session/load`'s response
        // only reports modes/configOptions, so resumed sessions reuse the id
        // from the request.
        let (session_id, config_options) = match &resume {
            Some(resume) => {
                let response = client
                    .request(
                        "session/load",
                        json!({
                            "sessionId": resume,
                            "cwd": cwd,
                            "mcpServers": [],
                        }),
                    )
                    .await?;
                let config_options =
                    response.get("configOptions").cloned().unwrap_or(Value::Null);
                (resume.clone(), config_options)
            }
            None => {
                let response = client
                    .request("session/new", json!({ "cwd": cwd, "mcpServers": [] }))
                    .await?;
                let config_options =
                    response.get("configOptions").cloned().unwrap_or(Value::Null);
                let session_id = response
                    .get("sessionId")
                    .and_then(Value::as_str)
                    .filter(|id| !id.is_empty())
                    .ok_or_else(|| {
                        HarnessError::Protocol(format!(
                            "{} session response has no sessionId: {response}",
                            agent.display_name()
                        ))
                    })?
                    .to_owned();
                (session_id, config_options)
            }
        };
        Ok::<(String, Value), HarnessError>((session_id, config_options))
    };
    let (session_id, bootstrap_updates, config_options) = tokio::select! {
        result = async {
            let (session_id, config_options) = setup.await?;
            let updates = collect_bootstrap_updates(&mut incoming).await;
            Ok::<(String, Vec<Value>, Value), HarnessError>((session_id, updates, config_options))
        } => match result {
            Ok(triple) => triple,
            Err(e) => {
                send_final(&event_tx, errored(e.to_string(), None)).await;
                shutdown_child(&mut child, kill_grace).await;
                return;
            }
        },
        _ = interrupt.cancelled() => {
            send_final(&event_tx, interrupted(None)).await;
            shutdown_child(&mut child, kill_grace).await;
            return;
        }
    };

    let mut assistant_message_id = new_message_id();
    if !send(
        &event_tx,
        AgentEvent::SessionStarted {
            harness: agent.harness_id(),
            model: request.model.clone().unwrap_or_default(),
            tools: Vec::new(),
            cwd: request.cwd.clone(),
            session_id: session_id.clone(),
            assistant_message_id: assistant_message_id.clone(),
        },
    )
    .await
    {
        shutdown_child(&mut child, kill_grace).await;
        return;
    }
    // A resumed session replays its transcript through bootstrap updates, but
    // the chat already shows that history; replaying it would duplicate every
    // message, so only fresh sessions apply the bootstrap.
    if resume.is_none() {
        for update in &bootstrap_updates {
            if !apply_session_update(&event_tx, update).await {
                shutdown_child(&mut child, kill_grace).await;
                return;
            }
        }
    }

    // Model and thinking are negotiated per session; a stale picker entry or an
    // unsupported thinking level must not kill the run, omp keeps its own default.
    for (option_id, value) in requested_config_options(&request, agent, &config_options) {
        if let Err(e) = client
            .request(
                "session/set_config_option",
                json!({
                    "sessionId": session_id,
                    "configId": option_id,
                    "value": value,
                }),
            )
            .await
        {
            tracing::debug!(target: "agent_harness::acp", "set_config_option {option_id} failed: {e}");
        }
    }

    let mut pending_turn: Option<SteerMessage> = None;
    let mut interrupted = false;
    let mut escalation: Option<tokio::task::JoinHandle<()>> = None;
    let mut turn_request = Some(request);
    loop {
        let request = match turn_request.take() {
            Some(request) => request,
            None => match pending_turn.take() {
                Some(message) => {
                    let (previous, next) = rotate(&mut assistant_message_id);
                    if !send(
                        &event_tx,
                        AgentEvent::Steered {
                            assistant_message_id: Some(previous),
                            next_assistant_message_id: Some(next),
                        },
                    )
                    .await
                    {
                        break;
                    }
                    steered_turn_request(message)
                }
                None => break,
            },
        };
        let blocks = match prompt_blocks(&request.prompt, &request.attachments).await {
            Ok(blocks) => blocks,
            Err(e) => {
                send_final(
                    &event_tx,
                    errored(e.to_string(), Some(session_id.clone())),
                )
                .await;
                break;
            }
        };
        let mut prompt_future =
            client.request_now("session/prompt", json!({ "sessionId": session_id, "prompt": blocks }));
        let mut turn_ended = false;
        while !turn_ended {
            tokio::select! {
                response = &mut prompt_future => {
                    // The agent writes trailing updates before the stop reason;
                    // both may be buffered in the channel by now, so flush the
                    // queue first or the last deltas are overtaken by the stop.
                    let mut receiver_gone = false;
                    loop {
                        tokio::task::yield_now().await;
                        let mut drained = false;
                        while let Ok(message) = incoming.try_recv() {
                            match message {
                                Incoming::Notification { method, params } => {
                                    if method == "session/update"
                                        && !apply_session_update(&event_tx, &params).await
                                    {
                                        receiver_gone = true;
                                        break;
                                    }
                                }
                                Incoming::Request { id, method, params } => {
                                    handle_server_request(
                                        &client,
                                        &id,
                                        &method,
                                        &params,
                                        permission,
                                        &request_input,
                                    );
                                }
                                Incoming::Eof => {}
                            }
                            drained = true;
                        }
                        if !drained || receiver_gone {
                            break;
                        }
                    }
                    if receiver_gone {
                        break;
                    }
                    let (status, error) = match response {
                        Ok(response) => {
                            if let Some(tokens) = response_tokens(&response) {
                                if !send(
                                    &event_tx,
                                    AgentEvent::ContextUsage { tokens: Some(tokens), window: None },
                                )
                                .await
                                {
                                    break;
                                }
                            }
                            stop_from_response(agent, &response)
                        }
                        Err(e) => (DoneStatus::Errored, Some(e.to_string())),
                    };
                    // A steer refused while idle is redelivered as a fresh turn
                    // instead of ending the run.
                    if status == DoneStatus::Completed && pending_turn.is_some() {
                        turn_request = None;
                    } else {
                        send_final(
                            &event_tx,
                            AgentEvent::Done {
                                status,
                                result: None,
                                error,
                                session_id: Some(session_id.clone()),
                            },
                        )
                        .await;
                    }
                    turn_ended = true;
                }
                message = incoming.recv() => match message {
                    Some(Incoming::Notification { method, params }) => {
                        if method == "session/update"
                            && !apply_session_update(&event_tx, &params).await
                        {
                            turn_ended = true;
                        }
                    }
                    Some(Incoming::Request { id, method, params }) => {
                        handle_server_request(
                            &client,
                            &id,
                            &method,
                            &params,
                            permission,
                            &request_input,
                        );
                    }
                    Some(Incoming::Eof) | None => {
                        let status = child.try_wait().ok().flatten();
                        let error = if interrupted {
                            None
                        } else {
                            Some(crate::crash_message(agent.crash_name(), status, &stderr_tail))
                        };
                        let status = if interrupted {
                            DoneStatus::Interrupted
                        } else {
                            DoneStatus::Errored
                        };
                        send_final(
                            &event_tx,
                            AgentEvent::Done {
                                status,
                                result: None,
                                error,
                                session_id: Some(session_id.clone()),
                            },
                        )
                        .await;
                        turn_ended = true;
                    }
                },
                steer = steering.recv(), if steering_open && escalation.is_none() => match steer {
                    Some(message) => {
                        if let Some(method) = agent.steering_method() {
                            let steer_params = json!({
                                "sessionId": session_id,
                                "prompt": [{ "type": "text", "text": message.prompt }],
                            });
                            match client.request(method, steer_params).await {
                                Ok(_) => {
                                    let (previous, next) = rotate(&mut assistant_message_id);
                                    if !send(
                                        &event_tx,
                                        AgentEvent::Steered {
                                            assistant_message_id: Some(previous),
                                            next_assistant_message_id: Some(next),
                                        },
                                    )
                                    .await
                                    {
                                        turn_ended = true;
                                    }
                                }
                                // The agent refuses a steer when no prompt owns the
                                // turn; redeliver it as the next turn's prompt.
                                Err(e) => {
                                    tracing::debug!(target: "agent_harness::acp", "steering refused, queuing as new turn: {e}");
                                    pending_turn = Some(message);
                                }
                            }
                        } else {
                            // No mid-turn injection on this agent; deliver it as
                            // the next turn's prompt.
                            pending_turn = Some(message);
                        }
                    }
                    None => steering_open = false,
                },
                _ = interrupt.cancelled(), if escalation.is_none() => {
                    interrupted = true;
                    client.notify("session/cancel", Some(json!({ "sessionId": session_id })));
                    escalation = crate::process::escalate_interrupt(&child, interrupt_grace, kill_grace);
                }
                changed = permission_changes.changed(), if permission_open && escalation.is_none() => {
                    match changed {
                        Ok(()) => {
                            permission =
                                permission_changes.borrow_and_update().for_harness(agent.harness_id());
                        }
                        Err(_) => permission_open = false,
                    }
                }
                _ = event_tx.closed() => {
                    turn_ended = true;
                }
            }
        }
        if event_tx.is_closed() {
            break;
        }
    }

    shutdown_child(&mut child, kill_grace).await;
    if let Some(handle) = escalation {
        handle.abort();
    }
}

/// The wire id for a config category differs per agent ("thinking" vs
/// "thought_level"), so resolve it from the session's advertised options.
fn config_option_id_for_category(config_options: &Value, category: &str) -> Option<String> {
    config_options
        .as_array()?
        .iter()
        .find(|option| option.get("category").and_then(Value::as_str) == Some(category))
        .and_then(|option| option.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn requested_config_options(
    request: &RunRequest,
    agent: AcpAgent,
    config_options: &Value,
) -> Vec<(String, Value)> {
    let mut options = Vec::new();
    let model_id = config_option_id_for_category(config_options, MODEL_CONFIG_CATEGORY)
        .unwrap_or_else(|| MODEL_CONFIG_CATEGORY.to_owned());
    let thinking_id = config_option_id_for_category(config_options, THINKING_CONFIG_CATEGORY)
        .unwrap_or_else(|| agent.thinking_config_id().to_owned());
    if let Some(model) = request.model.as_deref().filter(|model| !model.is_empty()) {
        options.push((model_id, Value::String(model.to_owned())));
    }
    if let Some(level) = request.reasoning.and_then(thinking_value) {
        options.push((thinking_id, Value::String(level.to_owned())));
    }
    options
}

fn steered_turn_request(message: SteerMessage) -> RunRequest {
    RunRequest {
        prompt: message.prompt,
        model: None,
        reasoning: None,
        model_options: serde_json::Map::new(),
        cwd: String::new(),
        permission: PermissionMode::default(),
        resume: None,
        fork: None,
        attachments: message.attachments,
        skills: message.skills,
        auto_compact_tokens: None,
        chrome: false,
        drop_prompt: None,
        ephemeral: true,
    }
}

/// Collects the bootstrap `session/update` notifications that `session/new` or
/// `session/load` emit before the first prompt.
async fn collect_bootstrap_updates(incoming: &mut mpsc::Receiver<Incoming>) -> Vec<Value> {
    let mut updates = Vec::new();
    while let Ok(Some(Incoming::Notification { method, params })) =
        tokio::time::timeout(Duration::from_millis(250), incoming.recv()).await
    {
        if method == "session/update" {
            updates.push(params);
        }
    }
    updates
}

fn response_tokens(response: &Value) -> Option<u64> {
    let usage = response.get("usage")?.as_object()?;
    usage
        .get("totalTokens")
        .and_then(Value::as_u64)
        .or_else(|| {
            let input = usage.get("inputTokens").and_then(Value::as_u64)?;
            let output = usage.get("outputTokens").and_then(Value::as_u64).unwrap_or(0);
            let thought = usage.get("thoughtTokens").and_then(Value::as_u64).unwrap_or(0);
            Some(input + output + thought)
        })
}

fn stop_from_response(agent: AcpAgent, response: &Value) -> (DoneStatus, Option<String>) {
    let reason = response.get("stopReason").and_then(Value::as_str);
    match reason {
        Some("cancelled") => (DoneStatus::Interrupted, None),
        Some("max_tokens") | Some("max_turn_requests") | Some("refusal") => (
            DoneStatus::Errored,
            Some(
                t!(
                    "{agent} stopped the turn: {reason}",
                    agent = agent.display_name(),
                    reason = reason.unwrap_or("unknown")
                )
                .to_string(),
            ),
        ),
        // `end_turn` and `end_sequence`, plus a missing reason on old agents.
        _ => (DoneStatus::Completed, None),
    }
}

/// Returns false when the event receiver is gone and the session loop should end.
async fn apply_session_update(
    event_tx: &mpsc::Sender<Result<AgentEvent, HarnessError>>,
    params: &Value,
) -> bool {
    let update = params.get("update");
    let Some(kind) = update
        .and_then(|update| update.get("sessionUpdate"))
        .and_then(Value::as_str)
    else {
        return true;
    };
    match kind {
        "agent_message_chunk" => match chunk_text(update) {
            Some(text) => send(event_tx, AgentEvent::TextDelta { text }).await,
            None => true,
        },
        "agent_thought_chunk" => match chunk_text(update) {
            Some(text) => send(event_tx, AgentEvent::ReasoningDelta { text }).await,
            None => true,
        },
        "tool_call" => match update.and_then(|update| update.get("toolCallId")).and_then(Value::as_str) {
            Some(id) => send(
                event_tx,
                AgentEvent::ToolCall {
                    id: id.to_owned(),
                    call: tool_call_from_update(update.unwrap_or(&Value::Null)),
                },
            )
            .await,
            None => true,
        },
        "tool_call_update" => match tool_result_from_update(update.unwrap_or(&Value::Null)) {
            Some(result) => send(event_tx, result).await,
            None => true,
        },
        "plan" => plan_events(event_tx, update.unwrap_or(&Value::Null)).await,
        _ => true,
    }
}

fn chunk_text(update: Option<&Value>) -> Option<String> {
    let content = update?.get("content")?;
    if content.get("type").and_then(Value::as_str) == Some("text") {
        return content
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_owned);
    }
    None
}

fn tool_call_from_update(update: &Value) -> ToolCall {
    let title = update
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let kind = update.get("kind").and_then(Value::as_str).unwrap_or("other");
    let raw_input = update.get("rawInput").cloned().unwrap_or(Value::Null);
    let location_path = || {
        update
            .get("locations")
            .and_then(Value::as_array)
            .and_then(|locations| locations.first())
            .and_then(|location| location.get("path"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let input_path = || {
        raw_input
            .get("path")
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    match kind {
        "execute" => {
            let command = raw_input
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or(&title)
                .to_owned();
            ToolCall::Exec { command }
        }
        "read" => ToolCall::ReadFile {
            path: location_path()
                .or_else(input_path)
                .unwrap_or_else(|| title.clone()),
        },
        "edit" | "delete" | "move" => ToolCall::EditFile {
            path: location_path()
                .or_else(input_path)
                .unwrap_or_else(|| title.clone()),
            old_string: None,
            new_string: None,
        },
        "search" => ToolCall::Search {
            pattern: raw_input
                .get("pattern")
                .and_then(Value::as_str)
                .unwrap_or(&title)
                .to_owned(),
            path: input_path(),
        },
        "fetch" => ToolCall::WebFetch {
            url: raw_input
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or(&title)
                .to_owned(),
            prompt: None,
        },
        _ => ToolCall::Unknown {
            name: title,
            input: Some(raw_input).filter(|input| !input.is_null()),
        },
    }
}

fn tool_result_from_update(update: &Value) -> Option<AgentEvent> {
    let id = update.get("toolCallId").and_then(Value::as_str)?;
    let status = update.get("status").and_then(Value::as_str)?;
    match status {
        "completed" | "failed" => {}
        _ => return None,
    }
    let mut output = Vec::new();
    let mut diff = None;
    for item in update
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match item.get("type").and_then(Value::as_str) {
            Some("content") => {
                let content = item.get("content");
                if content
                    .and_then(|content| content.get("type"))
                    .and_then(Value::as_str)
                    == Some("text")
                    && let Some(text) = content
                        .and_then(|content| content.get("text"))
                        .and_then(Value::as_str)
                {
                    output.push(text.to_owned());
                }
            }
            Some("diff") => {
                diff = Some(ToolDiff {
                    path: item
                        .get("path")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    old_text: item
                        .get("oldText")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    new_text: item
                        .get("newText")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                });
            }
            _ => {}
        }
    }
    if output.is_empty()
        && let Some(raw) = update.get("rawOutput")
    {
        for key in ["output", "stdout", "stderr", "result"] {
            if let Some(text) = raw.get(key).and_then(Value::as_str) {
                output.push(text.to_owned());
            }
        }
    }
    let output = (!output.is_empty()).then(|| output.join("\n"));
    Some(AgentEvent::ToolResult {
        id: id.to_owned(),
        is_error: status == "failed",
        output,
        diff,
    })
}

/// Returns false when the event receiver is gone and the session loop should end.
async fn plan_events(
    event_tx: &mpsc::Sender<Result<AgentEvent, HarnessError>>,
    update: &Value,
) -> bool {
    let Some(entries) = update.get("entries").and_then(Value::as_array) else {
        return true;
    };
    let items: Vec<TodoItem> = entries
        .iter()
        .filter_map(|entry| {
            let text = entry.get("content").and_then(Value::as_str)?;
            let status = entry.get("status").and_then(Value::as_str);
            let done = matches!(status, Some("completed") | Some("in_progress"));
            let status = match status {
                Some("completed") => TodoStatus::Completed,
                Some("in_progress") => TodoStatus::InProgress,
                _ => TodoStatus::Pending,
            };
            Some(TodoItem {
                text: text.to_owned(),
                done,
                status: Some(status),
            })
        })
        .collect();
    if items.is_empty() {
        return true;
    }
    let call = AgentEvent::ToolCall {
        id: crate::LIVE_PLAN_TOOL_ID.into(),
        call: ToolCall::Todo { items },
    };
    if !send(event_tx, call).await {
        return false;
    }
    send(
        event_tx,
        AgentEvent::ToolResult {
            id: crate::LIVE_PLAN_TOOL_ID.into(),
            is_error: false,
            output: None,
            diff: None,
        },
    )
    .await
}

fn handle_server_request(
    client: &RpcClient,
    id: &Value,
    method: &str,
    params: &Value,
    permission: PermissionMode,
    request_input: &Arc<RequestInputFn>,
) {
    match method {
        "session/request_permission" => {
            handle_permission_request(client, id, params, permission, request_input)
        }
        "fs/read_text_file" => handle_fs_read(client, id, params),
        "fs/write_text_file" => handle_fs_write(client, id, params),
        _ => {
            tracing::debug!(target: "agent_harness::acp", "unsupported server request: {method}");
            client.respond_error(id, -32601, &format!("method not supported: {method}"));
        }
    }
}

type RequestInputFn = Box<
    dyn Fn(Vec<UserInputQuestion>) -> tokio::sync::oneshot::Receiver<Vec<UserInputAnswer>>
        + Send
        + Sync,
>;

fn permission_options(params: &Value) -> Vec<(String, String)> {
    params
        .get("options")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|option| {
            let id = option.get("optionId").and_then(Value::as_str)?;
            let name = option.get("name").and_then(Value::as_str)?;
            Some((id.to_owned(), name.to_owned()))
        })
        .collect()
}

fn auto_allowed(permission: PermissionMode, params: &Value) -> bool {
    if permission.skips_prompts() {
        return true;
    }
    if permission != PermissionMode::AcceptEdits {
        return false;
    }
    let kind = params
        .pointer("/toolCall/kind")
        .and_then(Value::as_str)
        .unwrap_or("other");
    kind != "execute"
}

fn preferred_allow_option(options: &[(String, String)]) -> Option<String> {
    options
        .iter()
        .find(|(id, _)| id == "allow_once")
        .or_else(|| options.first())
        .map(|(id, _)| id.clone())
}

fn handle_permission_request(
    client: &RpcClient,
    id: &Value,
    params: &Value,
    permission: PermissionMode,
    request_input: &Arc<RequestInputFn>,
) {
    let options = permission_options(params);
    if options.is_empty() {
        client.respond(id, json!({ "outcome": { "outcome": "cancelled" } }));
        return;
    }
    if auto_allowed(permission, params) {
        match preferred_allow_option(&options) {
            Some(option_id) => client.respond(
                id,
                json!({ "outcome": { "outcome": "selected", "optionId": option_id } }),
            ),
            None => client.respond(id, json!({ "outcome": { "outcome": "cancelled" } })),
        }
        return;
    }
    let title = params
        .pointer("/toolCall/title")
        .and_then(Value::as_str)
        .unwrap_or("omp requests permission");
    let question = UserInputQuestion {
        id: uuid::Uuid::new_v4().to_string(),
        header: t!("Permission").to_string(),
        question: title.to_owned(),
        options: options.iter().map(|(_, name)| name.clone()).collect(),
        multi_select: false,
        prefill: None,
        multiline: false,
        accepts_typed_answer: false,
    };
    let client = client.clone();
    let request_input = Arc::clone(request_input);
    let options = Arc::new(options);
    let id = id.clone();
    tokio::spawn(async move {
        let answers = (request_input)(vec![question.clone()]).await.unwrap_or_default();
        let selected = answers
            .iter()
            .find(|answer| answer.question_id == question.id)
            .and_then(|answer| answer.labels.first())
            .and_then(|label| {
                options
                    .iter()
                    .find(|(_, name)| name == label)
                    .map(|(option_id, _)| option_id.clone())
            });
        match selected {
            Some(option_id) => client.respond(
                &id,
                json!({ "outcome": { "outcome": "selected", "optionId": option_id } }),
            ),
            None => client.respond(&id, json!({ "outcome": { "outcome": "cancelled" } })),
        }
    });
}

fn handle_fs_read(client: &RpcClient, id: &Value, params: &Value) {
    let Some(path) = params.get("path").and_then(Value::as_str).map(str::to_owned) else {
        client.respond_error(id, -32602, "fs/read_text_file requires a path");
        return;
    };
    let line = params.get("line").and_then(Value::as_u64);
    let limit = params.get("limit").and_then(Value::as_u64);
    let client = client.clone();
    let id = id.clone();
    tokio::spawn(async move {
        let read = async {
            let bytes = tokio::fs::read(&path).await?;
            let content = match (line, limit) {
                (Some(line), Some(limit)) => {
                    let lines: Vec<&[u8]> = bytes
                        .split(|byte| *byte == b'\n')
                        .skip(line.saturating_sub(1) as usize)
                        .take(limit as usize)
                        .collect();
                    let joined_len =
                        lines.iter().map(|slice| slice.len() + 1).sum::<usize>();
                    let mut joined = Vec::with_capacity(joined_len);
                    for slice in lines {
                        joined.extend_from_slice(slice);
                        joined.push(b'\n');
                    }
                    String::from_utf8_lossy(&joined).to_string()
                }
                _ => String::from_utf8_lossy(&bytes).to_string(),
            };
            Ok::<Value, std::io::Error>(json!({ "content": content }))
        };
        match read.await {
            Ok(result) => client.respond(&id, result),
            Err(e) => client.respond_error(&id, -32000, &format!("read failed: {e}")),
        }
    });
}

fn handle_fs_write(client: &RpcClient, id: &Value, params: &Value) {
    let Some(path) = params.get("path").and_then(Value::as_str).map(str::to_owned) else {
        client.respond_error(id, -32602, "fs/write_text_file requires a path");
        return;
    };
    let content = params
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let client = client.clone();
    let id = id.clone();
    tokio::spawn(async move {
        match tokio::fs::write(&path, content).await {
            Ok(_) => client.respond(&id, json!({})),
            Err(e) => client.respond_error(&id, -32000, &format!("write failed: {e}")),
        }
    });
}
