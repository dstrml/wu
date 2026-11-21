#![cfg(unix)]

use std::path::PathBuf;
use std::time::Duration;

use futures::StreamExt;
use tokio::sync::{mpsc, oneshot};

use agent_harness::{
    AcpAgent, AcpHarness, AgentEvent, CancellationToken, DoneStatus, Harness, HarnessId,
    PermissionMode, ReasoningLevel, RunControls, RunRequest, SteerMessage, ToolCall,
    UserInputAnswer,
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

fn controls(answer: &'static str) -> (RunControls, mpsc::Sender<SteerMessage>, CancellationToken) {
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
        .send(SteerMessage {
            prompt: "and more".into(),
            message_id: None,
            attachments: Vec::new(),
            skills: Vec::new(),
        })
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
        .send(SteerMessage {
            prompt: "and more".into(),
            message_id: None,
            attachments: Vec::new(),
            skills: Vec::new(),
        })
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
