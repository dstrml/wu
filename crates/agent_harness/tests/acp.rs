#![cfg(unix)]

use std::path::PathBuf;
use std::time::Duration;

use futures::StreamExt;
use tokio::sync::{mpsc, oneshot};

use agent_harness::{
    AcpAgent, AcpHarness, AgentEvent, CancellationToken, DoneStatus, Harness, HarnessId,
    PermissionMode, ReasoningLevel, RunControls, RunRequest, Steer, SteerMessage, ToolCall,
    TurnSettings, UserInputAnswer,
};

fn as_is(text: &str) -> &str {
    text
}

static OLDER: AcpAgent = AcpAgent {
    id: HarnessId::Antigravity,
    name: "Older",
    program: "fake-acp-agent.sh",
    args: &["acp", "--legacy"],
    executable_env: "WU_TEST_OLDER_ACP_AGENT",
    install_paths: Vec::new,
    searched: "nowhere",
    update_args: &[],
    modes: &[
        (PermissionMode::Ask, "ask"),
        (PermissionMode::FullAccess, "yolo"),
    ],
    steers_mid_turn: false,
    sign_in_hint: "Sign in to Older.",
    command_output: as_is,
    reasoning_levels: &[],
};

fn harness() -> AcpHarness {
    with_fixture(AcpHarness::new(&agent_harness::antigravity::ANTIGRAVITY))
}

fn older() -> AcpHarness {
    with_fixture(AcpHarness::new(&OLDER))
}

fn with_fixture(harness: AcpHarness) -> AcpHarness {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("fake-acp-agent.sh");
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(error) = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        {
            eprintln!("could not mark fake-acp-agent.sh executable: {error}");
        }
    }
    harness
        .with_executable(path)
        .with_graces(Duration::from_millis(200), Duration::from_millis(200))
}

fn request(prompt: &str) -> RunRequest {
    RunRequest {
        prompt: prompt.into(),
        model: None,
        reasoning: None,
        model_options: serde_json::Map::new(),
        cwd: std::env::temp_dir().to_string_lossy().into_owned(),
        permission: PermissionMode::Ask,
        resume: None,
        fork: None,
        attachments: Vec::new(),
        skills: Vec::new(),
        auto_compact_tokens: None,
        chrome: false,
        drop_prompt: None,
        ephemeral: false,
    }
}

fn controls(answer: &'static str) -> (RunControls, mpsc::Sender<Steer>, CancellationToken) {
    let (steer_tx, steer_rx) = mpsc::channel(8);
    let token = CancellationToken::new();
    let controls = RunControls {
        request_input: Box::new(move |questions| {
            let (tx, rx) = oneshot::channel();
            let answers = questions
                .iter()
                .map(|question| UserInputAnswer {
                    question_id: question.id.clone(),
                    labels: vec![answer.into()],
                })
                .collect();
            tx.send(answers).ok();
            rx
        }),
        steering: steer_rx,
        interrupt: token.clone(),
        permission: tokio::sync::watch::channel(PermissionMode::Ask).1,
    };
    (controls, steer_tx, token)
}

async fn collect_until_done(
    stream: &mut futures::stream::BoxStream<
        'static,
        Result<AgentEvent, agent_harness::HarnessError>,
    >,
) -> Vec<AgentEvent> {
    let mut events = Vec::new();
    while let Some(event) = tokio::time::timeout(Duration::from_secs(10), stream.next())
        .await
        .expect("the fake agent answers in time")
    {
        let event = event.expect("no stream errors");
        let done = matches!(event, AgentEvent::Done { .. });
        events.push(event);
        if done {
            break;
        }
    }
    events
}

fn texts(events: &[AgentEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::TextDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn done(events: &[AgentEvent]) -> (DoneStatus, Option<String>) {
    match events.last() {
        Some(AgentEvent::Done { status, error, .. }) => (*status, error.clone()),
        other => panic!("expected Done last, got {other:?}"),
    }
}

#[tokio::test]
async fn a_turn_applies_settings_asks_permission_and_reports_usage() {
    let mut run = request("hello");
    run.model = Some("gemini-flash".into());
    run.reasoning = Some(ReasoningLevel::Low);
    run.permission = PermissionMode::AcceptEdits;
    let (run_controls, _steer, _interrupt) = controls("Allow");
    let mut stream = harness().run(run, run_controls).await.unwrap();
    let events = collect_until_done(&mut stream).await;

    assert!(matches!(
        &events[0],
        AgentEvent::SessionStarted { session_id, model, .. } if session_id == "s1" && model == "gemini-flash"
    ));
    assert_eq!(
        texts(&events),
        "model=gemini-flash level=low mode=accept-editsallowed"
    );
    assert!(events.contains(&AgentEvent::ToolCall {
        id: "c1".into(),
        call: ToolCall::Exec {
            command: "ls".into()
        },
    }));
    assert!(events.contains(&AgentEvent::ToolResult {
        id: "c1".into(),
        is_error: false,
        output: Some("notes.txt\n".into()),
        diff: None,
    }));
    assert!(events.contains(&AgentEvent::ContextUsage {
        tokens: Some(900),
        window: Some(1000)
    }));
    assert!(events.contains(&AgentEvent::TitleChanged {
        title: "List files".into()
    }));
    assert!(events.contains(&AgentEvent::Usage {
        input_tokens: 12,
        output_tokens: 3
    }));
    assert_eq!(done(&events), (DoneStatus::Completed, None));
}

#[tokio::test]
async fn a_denied_permission_fails_the_tool() {
    let (run_controls, _steer, _interrupt) = controls("Deny");
    let mut stream = harness().run(request("hello"), run_controls).await.unwrap();
    let events = collect_until_done(&mut stream).await;
    assert!(texts(&events).ends_with("denied"));
    assert!(events.contains(&AgentEvent::ToolResult {
        id: "c1".into(),
        is_error: true,
        output: None,
        diff: None,
    }));
}

#[tokio::test]
async fn full_access_answers_permission_without_asking() {
    let mut run = request("hello");
    run.permission = PermissionMode::FullAccess;
    let (run_controls, _steer, _interrupt) = controls("Deny");
    let mut stream = harness().run(run, run_controls).await.unwrap();
    let events = collect_until_done(&mut stream).await;
    assert_eq!(
        texts(&events),
        "model=gemini-flash level=high mode=autoallowed"
    );
}

#[tokio::test]
async fn models_without_thinking_levels_skip_the_level() {
    let mut run = request("hello");
    run.model = Some("claude-opus".into());
    run.reasoning = Some(ReasoningLevel::Low);
    let (run_controls, _steer, _interrupt) = controls("Allow");
    let mut stream = harness().run(run, run_controls).await.unwrap();
    let events = collect_until_done(&mut stream).await;
    assert!(texts(&events).starts_with("model=claude-opus level=high mode=default"));
}

#[tokio::test]
async fn steering_mid_turn_ends_in_one_done() {
    let (run_controls, steer, _interrupt) = controls("Allow");
    let mut stream = harness().run(request("steer"), run_controls).await.unwrap();
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        let event = event.unwrap();
        let first_text = matches!(&event, AgentEvent::TextDelta { text } if text == "first");
        events.push(event);
        if first_text {
            break;
        }
    }
    steer
        .send(Steer::Message(SteerMessage {
            prompt: "and more".into(),
            message_id: None,
            attachments: Vec::new(),
            skills: Vec::new(),
            settings: None,
        }))
        .await
        .unwrap();
    events.extend(collect_until_done(&mut stream).await);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentEvent::Steered { .. }))
    );
    assert_eq!(texts(&events), "firstsecond");
    let dones = events
        .iter()
        .filter(|event| matches!(event, AgentEvent::Done { .. }))
        .count();
    assert_eq!(dones, 1);
    assert_eq!(done(&events), (DoneStatus::Completed, None));
}

#[tokio::test]
async fn stopping_cancels_the_prompt() {
    let (run_controls, _steer, interrupt) = controls("Allow");
    let mut stream = harness().run(request("wait"), run_controls).await.unwrap();
    while let Some(event) = stream.next().await {
        if matches!(event.unwrap(), AgentEvent::TextDelta { .. }) {
            break;
        }
    }
    interrupt.cancel();
    let events = collect_until_done(&mut stream).await;
    assert_eq!(done(&events), (DoneStatus::Interrupted, None));
}

#[tokio::test]
async fn signed_out_runs_say_how_to_sign_in() {
    let dir = tempfile::Builder::new()
        .prefix("signed-out")
        .tempdir()
        .unwrap();
    let mut run = request("hello");
    run.cwd = dir.path().to_string_lossy().into_owned();
    let (run_controls, _steer, _interrupt) = controls("Allow");
    let mut stream = harness().run(run, run_controls).await.unwrap();
    let events = collect_until_done(&mut stream).await;
    let (status, error) = done(&events);
    assert_eq!(status, DoneStatus::Errored);
    assert!(error.unwrap().contains("Run `agy`"));
}

#[tokio::test]
async fn a_crash_reports_the_agents_last_words() {
    let (run_controls, _steer, _interrupt) = controls("Allow");
    let mut stream = harness().run(request("crash"), run_controls).await.unwrap();
    let events = collect_until_done(&mut stream).await;
    let (status, error) = done(&events);
    assert_eq!(status, DoneStatus::Errored);
    assert!(error.unwrap().contains("lost connection"));
}

#[tokio::test]
async fn resuming_keeps_the_session_and_a_lost_one_starts_fresh() {
    let mut run = request("again");
    run.resume = Some("abc".into());
    let (run_controls, _steer, _interrupt) = controls("Allow");
    let mut stream = harness().run(run, run_controls).await.unwrap();
    let events = collect_until_done(&mut stream).await;
    assert!(matches!(
        &events[0],
        AgentEvent::SessionStarted { session_id, .. } if session_id == "abc"
    ));
    assert_eq!(texts(&events), "said: again");

    let mut run = request("again");
    run.resume = Some("gone".into());
    let (run_controls, _steer, _interrupt) = controls("Allow");
    let mut stream = harness().run(run, run_controls).await.unwrap();
    let events = collect_until_done(&mut stream).await;
    assert!(matches!(
        &events[0],
        AgentEvent::SessionStarted { session_id, .. } if session_id == "s1"
    ));
    assert!(events.iter().any(
        |event| matches!(event, AgentEvent::Error { message } if message.contains("new one"))
    ));
    assert_eq!(done(&events), (DoneStatus::Completed, None));
}

#[tokio::test]
async fn forks_are_refused_so_the_chat_sends_context_instead() {
    let mut run = request("hello");
    run.fork = Some("parent".into());
    let (run_controls, _steer, _interrupt) = controls("Allow");
    assert!(harness().run(run, run_controls).await.is_err());
}

#[tokio::test]
async fn the_model_list_carries_each_models_thinking_levels() {
    let models = harness().models().await.unwrap();
    let summary: Vec<_> = models
        .iter()
        .map(|model| {
            (
                model.id.as_str(),
                model.label.as_str(),
                model.reasoning_levels.clone(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            (
                "gemini-flash",
                "Gemini Flash",
                vec![ReasoningLevel::Low, ReasoningLevel::High]
            ),
            ("claude-opus", "Claude Opus", Vec::new()),
        ]
    );
}

#[tokio::test]
async fn commands_saved_chats_and_history_come_from_the_agent() {
    let dir = tempfile::Builder::new()
        .prefix("commands")
        .tempdir()
        .unwrap();
    let commands = harness().commands(dir.path()).await.unwrap();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].name, "goal");

    let sessions = harness().external_sessions(dir.path()).await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "old");
    assert_eq!(sessions[0].title, "Fix the build");

    let history = harness().external_history(dir.path(), "old").await.unwrap();
    assert_eq!(
        history,
        vec![
            AgentEvent::UserMessage {
                text: "Fix the build".into()
            },
            AgentEvent::ToolCall {
                id: "c1".into(),
                call: ToolCall::Exec {
                    command: "make".into()
                },
            },
            AgentEvent::ToolResult {
                id: "c1".into(),
                is_error: false,
                output: Some("ok".into()),
                diff: None,
            },
            AgentEvent::TextDelta {
                text: "Fixed.".into()
            },
            AgentEvent::AssistantMessageCompleted {
                assistant_message_id: String::new()
            },
        ]
    );
}

#[tokio::test]
async fn older_agents_switch_model_and_mode_with_their_own_requests() {
    let mut run = request("hello");
    run.model = Some("m2".into());
    run.reasoning = Some(ReasoningLevel::Low);
    run.permission = PermissionMode::FullAccess;
    let (run_controls, _steer, _interrupt) = controls("Deny");
    let mut stream = older().run(run, run_controls).await.unwrap();
    let events = collect_until_done(&mut stream).await;
    assert!(matches!(
        &events[0],
        AgentEvent::SessionStarted { model, .. } if model == "m2"
    ));
    assert_eq!(texts(&events), "model=m2 level=none mode=yoloallowed");

    let models = older().models().await.unwrap();
    let summary: Vec<_> = models
        .iter()
        .map(|model| (model.id.as_str(), model.label.as_str()))
        .collect();
    assert_eq!(summary, [("m1", "One"), ("m2", "Two")]);
}

#[tokio::test]
async fn messages_wait_for_the_turn_when_the_agent_cant_steer() {
    let (run_controls, steer, _interrupt) = controls("Allow");
    let mut stream = older().run(request("slow"), run_controls).await.unwrap();
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        let event = event.unwrap();
        let first_text = matches!(&event, AgentEvent::TextDelta { text } if text == "first");
        events.push(event);
        if first_text {
            break;
        }
    }
    steer
        .send(Steer::Message(SteerMessage {
            prompt: "and more".into(),
            message_id: None,
            attachments: Vec::new(),
            skills: Vec::new(),
            settings: None,
        }))
        .await
        .unwrap();
    events.extend(collect_until_done(&mut stream).await);
    events.extend(collect_until_done(&mut stream).await);
    let position = |wanted: fn(&AgentEvent) -> bool| events.iter().position(wanted).unwrap();
    assert!(
        position(|event| matches!(event, AgentEvent::Done { .. }))
            < position(|event| matches!(event, AgentEvent::Steered { .. }))
    );
    assert_eq!(texts(&events), "firstsaid: and more");
    assert_eq!(done(&events), (DoneStatus::Completed, None));
}

#[tokio::test]
async fn one_off_and_folderless_runs_open_in_wus_scratch_folder() {
    let dir = tempfile::Builder::new()
        .prefix("signed-out")
        .tempdir()
        .unwrap();
    let mut run = request("quick");
    run.cwd = dir.path().to_string_lossy().into_owned();
    run.ephemeral = true;
    let (run_controls, _steer, _interrupt) = controls("Allow");
    let mut stream = harness().run(run, run_controls).await.unwrap();
    let events = collect_until_done(&mut stream).await;
    assert_eq!(texts(&events), "said: quick");

    let mut run = request("quick");
    run.cwd = String::new();
    let (run_controls, _steer, _interrupt) = controls("Allow");
    let mut stream = harness().run(run, run_controls).await.unwrap();
    let events = collect_until_done(&mut stream).await;
    assert_eq!(done(&events), (DoneStatus::Completed, None));
}

#[tokio::test]
async fn agents_without_resume_reopen_chats_by_loading_them_quietly() {
    let mut run = request("again");
    run.resume = Some("abc".into());
    let (run_controls, _steer, _interrupt) = controls("Allow");
    let mut stream = older().run(run, run_controls).await.unwrap();
    let events = collect_until_done(&mut stream).await;
    assert!(matches!(
        &events[0],
        AgentEvent::SessionStarted { session_id, .. } if session_id == "abc"
    ));
    assert!(!events.iter().any(|event| matches!(
        event,
        AgentEvent::Error { .. } | AgentEvent::ToolCall { .. }
    )));
    assert_eq!(texts(&events), "said: again");
}

fn stop() -> Steer {
    Steer::StopTurn(oneshot::channel().0)
}

fn message(prompt: &str, settings: Option<TurnSettings>) -> Steer {
    Steer::Message(SteerMessage {
        prompt: prompt.into(),
        message_id: None,
        attachments: Vec::new(),
        skills: Vec::new(),
        settings,
    })
}

fn settings(model: Option<&str>, reasoning: Option<ReasoningLevel>) -> TurnSettings {
    TurnSettings {
        model: model.map(str::to_owned),
        reasoning,
        model_options: serde_json::Map::new(),
    }
}

type Events = futures::stream::BoxStream<'static, Result<AgentEvent, agent_harness::HarnessError>>;

async fn wait_for_text(stream: &mut Events, wanted: &str) {
    while let Some(event) = tokio::time::timeout(Duration::from_secs(10), stream.next())
        .await
        .expect("the fake agent answers in time")
    {
        if matches!(event.unwrap(), AgentEvent::TextDelta { text } if text == wanted) {
            return;
        }
    }
    panic!("the stream ended before {wanted}");
}

async fn assert_quiet(stream: &mut Events) {
    let event = tokio::time::timeout(Duration::from_millis(300), stream.next()).await;
    assert!(event.is_err(), "unexpected event: {event:?}");
}

#[tokio::test]
async fn stopping_a_turn_keeps_the_agent_for_the_next_message() {
    let (run_controls, steer, _interrupt) = controls("Allow");
    let mut stream = harness().run(request("pid"), run_controls).await.unwrap();
    let first = collect_until_done(&mut stream).await;
    assert_eq!(done(&first), (DoneStatus::Completed, None));

    steer.send(message("wait", None)).await.unwrap();
    wait_for_text(&mut stream, "waiting").await;
    let (acknowledge, mut acknowledged) = oneshot::channel();
    steer.send(Steer::StopTurn(acknowledge)).await.unwrap();
    steer.send(message("pid", None)).await.unwrap();
    let stopped = collect_until_done(&mut stream).await;
    assert_eq!(done(&stopped), (DoneStatus::Interrupted, None));
    assert!(
        acknowledged.try_recv().is_ok(),
        "the stop is confirmed first"
    );
    assert_eq!(texts(&stopped), "");

    let next = collect_until_done(&mut stream).await;
    assert_eq!(done(&next), (DoneStatus::Completed, None));
    assert_eq!(texts(&next), texts(&first), "the same process answers");
    assert_quiet(&mut stream).await;
}

#[tokio::test]
async fn a_stopped_turn_withdraws_its_questions_and_drops_late_output() {
    let (asked_tx, mut asked) = mpsc::unbounded_channel();
    let held = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let (steer, steer_rx) = mpsc::channel(8);
    let run_controls = RunControls {
        request_input: Box::new({
            let held = held.clone();
            move |_| {
                let (tx, rx) = oneshot::channel();
                held.lock().unwrap().push(tx);
                asked_tx.send(()).unwrap();
                rx
            }
        }),
        steering: steer_rx,
        interrupt: CancellationToken::new(),
        permission: tokio::sync::watch::channel(PermissionMode::Ask).1,
    };
    let mut stream = harness().run(request("halt"), run_controls).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), asked.recv())
        .await
        .expect("the agent asks for permission")
        .unwrap();
    steer.send(stop()).await.unwrap();
    let stopped = collect_until_done(&mut stream).await;
    assert_eq!(done(&stopped), (DoneStatus::Interrupted, None));
    assert_eq!(texts(&stopped), "halting");
    assert_quiet(&mut stream).await;
    assert!(held.lock().unwrap().iter().all(|answer| answer.is_closed()));

    steer.send(message("pid", None)).await.unwrap();
    let next = collect_until_done(&mut stream).await;
    assert_eq!(done(&next), (DoneStatus::Completed, None));
    assert!(texts(&next).starts_with("pid="), "{}", texts(&next));
}

#[tokio::test]
async fn output_trailing_a_stopped_turn_stays_out_of_the_next_reply() {
    let (asked_tx, mut asked) = mpsc::unbounded_channel();
    let held = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let (steer, steer_rx) = mpsc::channel(8);
    let run_controls = RunControls {
        request_input: Box::new({
            let held = held.clone();
            move |_| {
                let (tx, rx) = oneshot::channel();
                held.lock().unwrap().push(tx);
                asked_tx.send(()).unwrap();
                rx
            }
        }),
        steering: steer_rx,
        interrupt: CancellationToken::new(),
        permission: tokio::sync::watch::channel(PermissionMode::Ask).1,
    };
    let mut stream = harness().run(request("halt"), run_controls).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), asked.recv())
        .await
        .expect("the agent asks for permission")
        .unwrap();
    steer.send(stop()).await.unwrap();
    steer.send(message("pid", None)).await.unwrap();
    let stopped = collect_until_done(&mut stream).await;
    assert_eq!(done(&stopped), (DoneStatus::Interrupted, None));

    let next = collect_until_done(&mut stream).await;
    assert_eq!(done(&next), (DoneStatus::Completed, None));
    assert!(texts(&next).starts_with("pid="), "{}", texts(&next));
}

#[tokio::test]
async fn stopping_between_turns_does_nothing() {
    let (run_controls, steer, _interrupt) = controls("Allow");
    let mut stream = harness().run(request("pid"), run_controls).await.unwrap();
    let first = collect_until_done(&mut stream).await;
    steer.send(stop()).await.unwrap();
    assert_quiet(&mut stream).await;
    steer.send(message("pid", None)).await.unwrap();
    let next = collect_until_done(&mut stream).await;
    assert_eq!(done(&next), (DoneStatus::Completed, None));
    assert_eq!(texts(&next), texts(&first));
}

#[tokio::test]
async fn a_stop_the_agent_ignores_ends_the_run() {
    let (run_controls, steer, _interrupt) = controls("Allow");
    let mut stream = harness().run(request("deaf"), run_controls).await.unwrap();
    wait_for_text(&mut stream, "stuck").await;
    let (acknowledge, mut acknowledged) = oneshot::channel();
    steer.send(Steer::StopTurn(acknowledge)).await.unwrap();
    let events = collect_until_done(&mut stream).await;
    assert_eq!(done(&events), (DoneStatus::Interrupted, None));
    assert!(
        acknowledged.try_recv().is_err(),
        "a killed agent confirms nothing"
    );
    let end = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .expect("the run ends once the agent is killed");
    assert!(end.is_none(), "{end:?}");
}

#[tokio::test]
async fn new_settings_apply_to_the_turn_their_message_starts() {
    let (run_controls, steer, _interrupt) = controls("Allow");
    let mut stream = harness().run(request("hello"), run_controls).await.unwrap();
    let first = collect_until_done(&mut stream).await;
    assert!(texts(&first).starts_with("model=gemini-flash level=high"));

    let changed = settings(Some("gemini-flash"), Some(ReasoningLevel::Low));
    steer.send(message("hello", Some(changed))).await.unwrap();
    let second = collect_until_done(&mut stream).await;
    assert!(texts(&second).starts_with("model=gemini-flash level=low"));

    steer.send(message("hello", None)).await.unwrap();
    let third = collect_until_done(&mut stream).await;
    assert!(texts(&third).starts_with("model=gemini-flash level=low"));
}

#[tokio::test]
async fn settings_sent_mid_turn_wait_for_the_next_turn() {
    let (run_controls, steer, _interrupt) = controls("Allow");
    let mut stream = harness().run(request("steer"), run_controls).await.unwrap();
    wait_for_text(&mut stream, "first").await;
    let changed = settings(None, Some(ReasoningLevel::Low));
    steer
        .send(message("and more", Some(changed)))
        .await
        .unwrap();
    assert_eq!(
        done(&collect_until_done(&mut stream).await),
        (DoneStatus::Completed, None)
    );
    steer.send(message("hello", None)).await.unwrap();
    let next = collect_until_done(&mut stream).await;
    assert!(texts(&next).starts_with("model=gemini-flash level=low"));

    let (run_controls, steer, _interrupt) = controls("Allow");
    let mut stream = older().run(request("slow"), run_controls).await.unwrap();
    wait_for_text(&mut stream, "first").await;
    steer
        .send(message("hello", Some(settings(Some("m2"), None))))
        .await
        .unwrap();
    collect_until_done(&mut stream).await;
    let queued = collect_until_done(&mut stream).await;
    assert!(texts(&queued).starts_with("model=m2 level=none"));
}

#[test]
fn only_settings_the_agent_can_be_told_are_applied_live() {
    let harness = harness();
    let high = settings(Some("gemini-flash"), Some(ReasoningLevel::High));
    let low = settings(Some("claude-opus"), Some(ReasoningLevel::Low));
    assert!(harness.stops_turn_in_place());
    assert!(harness.applies_settings_live(&high, &low));
    assert!(harness.applies_settings_live(&settings(None, None), &low));
    assert!(!harness.applies_settings_live(&high, &settings(None, Some(ReasoningLevel::High))));
    assert!(!harness.applies_settings_live(&high, &settings(Some("gemini-flash"), None)));
    let mut with_option = high.clone();
    with_option
        .model_options
        .insert("agent".into(), "build".into());
    assert!(!harness.applies_settings_live(&high, &with_option));
}

#[tokio::test]
async fn lookups_return_before_the_helper_process_exits() {
    let dir = tempfile::Builder::new()
        .prefix("stubborn-commands")
        .tempdir()
        .unwrap();
    let harness = harness().with_graces(Duration::from_millis(200), Duration::from_secs(30));
    let started = std::time::Instant::now();
    let commands = harness.commands(dir.path()).await.unwrap();
    assert_eq!(commands[0].name, "goal");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the lookup waited for the helper to exit"
    );
}
