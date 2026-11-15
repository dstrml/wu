#![cfg(unix)]

use std::path::PathBuf;
use std::time::Duration;

use futures::StreamExt;
use tokio::sync::{mpsc, oneshot};

use agent_harness::{
    AcpHarness, AgentEvent, CancellationToken, DoneStatus, Harness, HarnessError, HarnessId,
    Model, PermissionMode, ReasoningLevel, RunControls, RunRequest, SteerMessage, TodoItem,
    TodoStatus, ToolCall, UserInputAnswer,
};

fn fixture_path() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("fake-omp.sh");
    use std::os::unix::fs::PermissionsExt;
    if let Err(error) = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)) {
        eprintln!("could not mark fake-omp.sh executable: {error}");
    }
    path
}

fn harness() -> AcpHarness {
    AcpHarness::omp().with_executable(fixture_path())
}

fn request(scenario: &str) -> RunRequest {
    RunRequest {
        prompt: format!("scenario:{scenario}"),
        model: Some("m1".into()),
        reasoning: Some(ReasoningLevel::High),
        model_options: serde_json::Map::new(),
        cwd: "/tmp".into(),
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

fn controls(
    answer_label: &'static str,
) -> (RunControls, mpsc::Sender<SteerMessage>, CancellationToken) {
    let (steer_tx, steer_rx) = mpsc::channel(8);
    let token = CancellationToken::new();
    let controls = RunControls {
        request_input: Box::new(move |questions| {
            let (tx, rx) = oneshot::channel();
            let answers: Vec<UserInputAnswer> = questions
                .iter()
                .map(|question| UserInputAnswer {
                    question_id: question.id.clone(),
                    labels: vec![answer_label.into()],
                })
                .collect();
            tx.send(answers).expect("receiver is returned alongside");
            rx
        }),
        steering: steer_rx,
        interrupt: token.clone(),
        permission: tokio::sync::watch::channel(PermissionMode::Ask).1,
    };
    (controls, steer_tx, token)
}

async fn run_to_end(
    harness: &AcpHarness,
    req: RunRequest,
    controls: RunControls,
) -> Vec<AgentEvent> {
    let stream = harness.run(req, controls).await.expect("run starts");
    tokio::time::timeout(
        Duration::from_secs(15),
        stream.map(|result| result.expect("stream event")).collect::<Vec<_>>(),
    )
    .await
    .expect("run finished in time")
}

fn done(events: &[AgentEvent]) -> (DoneStatus, Option<String>) {
    let dones: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::Done {
                status, error, ..
            } => Some((*status, error.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(dones.len(), 1, "{events:?}");
    dones[0].clone()
}

fn text_of(events: &[AgentEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::TextDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn happy_path_maps_deltas_usage_and_done() {
    let (controls, _steer, _token) = controls("Allow once");
    let events = run_to_end(&harness(), request("happy"), controls).await;

    let starts: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::SessionStarted {
                harness,
                model,
                cwd,
                session_id,
                ..
            } => Some((*harness, model.clone(), cwd.clone(), session_id.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(starts.len(), 1, "{events:?}");
    let (harness_id, model, cwd, session_id) = &starts[0];
    assert_eq!(*harness_id, HarnessId::Omp);
    assert_eq!(model, "m1");
    assert_eq!(cwd, "/tmp");
    assert_eq!(session_id, "s-1");

    assert!(events.contains(&AgentEvent::ReasoningDelta {
        text: "thinking".into()
    }));
    assert_eq!(text_of(&events), "Hello world");

    assert!(events.contains(&AgentEvent::ContextUsage {
        tokens: Some(900),
        window: None
    }));

    assert_eq!(done(&events), (DoneStatus::Completed, None));
}

#[tokio::test]
async fn tools_map_kinds_outputs_and_diffs() {
    let (controls, _steer, _token) = controls("Allow once");
    let events = run_to_end(&harness(), request("tools"), controls).await;

    assert!(events.contains(&AgentEvent::ToolCall {
        id: "c1".into(),
        call: ToolCall::Exec {
            command: "ls -la".into()
        },
    }));
    assert!(events.contains(&AgentEvent::ToolResult {
        id: "c1".into(),
        is_error: false,
        output: Some("file.txt".into()),
        diff: None,
    }));
    assert!(events.contains(&AgentEvent::ToolCall {
        id: "c2".into(),
        call: ToolCall::EditFile {
            path: "/w/f.txt".into(),
            old_string: None,
            new_string: None,
        },
    }));
    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::ToolResult {
            id,
            diff: Some(diff),
            ..
        } if id == "c2" && diff.path == "/w/f.txt" && diff.new_text == "new"
    )));
    assert!(events.contains(&AgentEvent::ToolCall {
        id: "c3".into(),
        call: ToolCall::Search {
            pattern: "TODO".into(),
            path: None,
        },
    }));
    assert!(events.contains(&AgentEvent::ToolResult {
        id: "c3".into(),
        is_error: true,
        output: Some("boom".into()),
        diff: None,
    }));
    assert_eq!(done(&events), (DoneStatus::Completed, None));
}

#[tokio::test]
async fn plan_updates_map_to_the_live_plan_tool() {
    let (controls, _steer, _token) = controls("Allow once");
    let events = run_to_end(&harness(), request("plan"), controls).await;

    assert!(events.contains(&AgentEvent::ToolCall {
        id: agent_harness::LIVE_PLAN_TOOL_ID.into(),
        call: ToolCall::Todo {
            items: vec![
                TodoItem {
                    text: "read the code".into(),
                    done: true,
                    status: Some(TodoStatus::Completed),
                },
                TodoItem {
                    text: "fix the bug".into(),
                    done: true,
                    status: Some(TodoStatus::InProgress),
                },
                TodoItem {
                    text: "run tests".into(),
                    done: false,
                    status: Some(TodoStatus::Pending),
                },
            ],
        },
    }));
    assert_eq!(done(&events), (DoneStatus::Completed, None));
}

#[tokio::test]
async fn full_access_answers_permission_requests_without_asking() {
    let was_asked = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (steer_tx, steer_rx) = mpsc::channel(8);
    let flag = std::sync::Arc::clone(&was_asked);
    let controls = RunControls {
        request_input: Box::new(move |questions| {
            let (tx, rx) = oneshot::channel();
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            let answers: Vec<UserInputAnswer> = questions
                .iter()
                .map(|question| UserInputAnswer {
                    question_id: question.id.clone(),
                    labels: vec!["Reject".into()],
                })
                .collect();
            tx.send(answers).expect("receiver is returned alongside");
            rx
        }),
        steering: steer_rx,
        interrupt: CancellationToken::new(),
        permission: tokio::sync::watch::channel(PermissionMode::FullAccess).1,
    };
    let mut req = request("permission");
    req.permission = PermissionMode::FullAccess;
    let events = run_to_end(&harness(), req, controls).await;

    assert!(
        !was_asked.load(std::sync::atomic::Ordering::SeqCst),
        "full access must not ask the user"
    );
    // The fixture echoes the chosen option into the tool output.
    let tool_output = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolResult { output, .. } => output.clone(),
            _ => None,
        })
        .next()
        .expect("tool result");
    assert_eq!(tool_output, "allow_once");
    assert_eq!(done(&events), (DoneStatus::Completed, None));
    drop(steer_tx);
}

#[tokio::test]
async fn ask_mode_surfaces_permission_options() {
    let (controls, _steer, _token) = controls("Allow once");
    let events = run_to_end(&harness(), request("permission"), controls).await;
    let tool_output = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolResult { output, .. } => output.clone(),
            _ => None,
        })
        .next()
        .expect("tool result");
    assert_eq!(tool_output, "allow_once");
}

#[tokio::test]
async fn unknown_answer_cancels_the_permission_request() {
    let (controls, _steer, _token) = controls("Nope");
    let events = run_to_end(&harness(), request("permission"), controls).await;
    let tool_output = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolResult { output, .. } => output.clone(),
            _ => None,
        })
        .next()
        .expect("tool result");
    assert_eq!(tool_output, "reject_once");
}

#[tokio::test]
async fn mid_turn_steering_is_admitted_and_keeps_the_turn() {
    let (controls, steer_tx, _token) = controls("Allow once");
    let mut stream = harness().run(request("steer"), controls).await.expect("run starts");
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let collector = tokio::spawn(async move {
        while let Some(event) = stream.next().await {
            let event = event.expect("stream event");
            if matches!(event, AgentEvent::Done { .. }) {
                let _ = event_tx.send(event);
                break;
            }
            let _ = event_tx.send(event);
        }
    });

    // Steer as soon as the first mid-turn delta arrives.
    loop {
        let event = tokio::time::timeout(Duration::from_secs(15), event_rx.recv())
            .await
            .expect("event in time")
            .expect("stream open");
        if matches!(event, AgentEvent::TextDelta { .. }) {
            steer_tx
                .send(SteerMessage {
                    prompt: "and more".into(),
                    message_id: None,
                    attachments: Vec::new(),
                    skills: Vec::new(),
                })
                .await
                .expect("steering channel open");
            break;
        }
    }

    let mut events = vec![];
    while let Some(event) = tokio::time::timeout(Duration::from_secs(15), event_rx.recv())
        .await
        .expect("event in time")
    {
        events.push(event);
    }
    collector.await.expect("collector");
    let steered = events
        .iter()
        .filter(|event| matches!(event, AgentEvent::Steered { .. }))
        .count();
    assert_eq!(steered, 1, "{events:?}");
    assert!(events.contains(&AgentEvent::TextDelta { text: "b".into() }));
    assert_eq!(done(&events), (DoneStatus::Completed, None));
}

#[tokio::test]
async fn idle_steering_is_refused_and_redelivered_as_a_new_turn() {
    let (steer_tx, steer_rx) = mpsc::channel(8);
    let token = CancellationToken::new();
    let controls = RunControls {
        request_input: Box::new(|questions| {
            let (tx, rx) = oneshot::channel();
            let answers: Vec<UserInputAnswer> = questions
                .iter()
                .map(|question| UserInputAnswer {
                    question_id: question.id.clone(),
                    labels: vec!["Allow once".into()],
                })
                .collect();
            tx.send(answers).expect("receiver is returned alongside");
            rx
        }),
        steering: steer_rx,
        interrupt: token.clone(),
        permission: tokio::sync::watch::channel(PermissionMode::Ask).1,
    };
    steer_tx
        .send(SteerMessage {
            prompt: "queued".into(),
            message_id: None,
            attachments: Vec::new(),
            skills: Vec::new(),
        })
        .await
        .expect("steering channel open");
    let events = run_to_end(&harness(), request("idle-steer"), controls).await;

    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, AgentEvent::Steered { .. }))
            .count(),
        1,
        "{events:?}"
    );
    assert_eq!(text_of(&events), "queued");
    assert_eq!(done(&events), (DoneStatus::Completed, None));
}

#[tokio::test]
async fn cancel_interrupts_the_turn() {
    let (controls, steer_tx, token) = controls("Allow once");
    let harness = harness();
    let mut stream = harness.run(request("cancel"), controls).await.expect("run starts");
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(event) = stream.next().await {
            let event = event.expect("stream event");
            let finished = matches!(event, AgentEvent::Done { .. });
            let _ = event_tx.send(event);
            if finished {
                break;
            }
        }
    });

    // Cancel as soon as the session starts.
    loop {
        let event = tokio::time::timeout(Duration::from_secs(15), event_rx.recv())
            .await
            .expect("event in time")
            .expect("stream open");
        if matches!(event, AgentEvent::SessionStarted { .. }) {
            token.cancel();
            break;
        }
    }

    let mut events = vec![];
    while let Some(event) = tokio::time::timeout(Duration::from_secs(15), event_rx.recv())
        .await
        .expect("event in time")
    {
        events.push(event);
    }
    assert_eq!(done(&events), (DoneStatus::Interrupted, None));
    drop(steer_tx);
}

#[tokio::test]
async fn resume_loads_the_session_and_ignores_bootstrap_echoes() {
    let (controls, _steer, _token) = controls("Allow once");
    let mut req = request("happy");
    req.resume = Some("s-1".into());
    let events = run_to_end(&harness(), req, controls).await;

    // The bootstrap user_message_chunk replay is dropped.
    assert!(!events.contains(&AgentEvent::TextDelta { text: "earlier".into() }));
    assert_eq!(text_of(&events), "Hello world");
    assert_eq!(done(&events), (DoneStatus::Completed, None));
}

#[tokio::test]
async fn serves_the_agents_fs_reads() {
    let (controls, _steer, _token) = controls("Allow once");
    let events = run_to_end(&harness(), request("fs"), controls).await;
    assert!(events.contains(&AgentEvent::ToolCall {
        id: "c1".into(),
        call: ToolCall::ReadFile {
            path: "/etc/hosts".into(),
        },
    }));
    assert_eq!(done(&events), (DoneStatus::Completed, None));
}

#[tokio::test]
async fn catalog_reads_models_and_thinking_levels() {
    let harness = harness();
    let catalog = tokio::time::timeout(Duration::from_secs(15), harness.model_catalog(false))
        .await
        .expect("catalog in time")
        .expect("catalog ok");
    let models: Vec<&Model> = catalog.models.iter().collect();
    assert_eq!(models.len(), 2, "{catalog:?}");
    assert_eq!(models[0].id, "m1");
    assert_eq!(models[0].label, "Model One");
    assert_eq!(
        models[0].description.as_deref(),
        Some("provider/a"),
        "{catalog:?}"
    );
    assert!(models[0]
        .reasoning_levels
        .contains(&ReasoningLevel::XHigh));
}

#[tokio::test]
async fn missing_executable_reports_agent_missing() {
    let harness = AcpHarness::omp().with_executable("/nonexistent/omp-fake");
    let req = request("happy");
    let (steer_tx, steer_rx) = mpsc::channel(8);
    let controls = RunControls {
        request_input: Box::new(|_questions| {
            let (tx, rx) = oneshot::channel::<Vec<UserInputAnswer>>();
            tx.send(Vec::new()).expect("send empty answers");
            rx
        }),
        steering: steer_rx,
        interrupt: CancellationToken::new(),
        permission: tokio::sync::watch::channel(PermissionMode::Ask).1,
    };
    let error = match harness.run(req, controls).await {
        Err(error) => error,
        Ok(_) => panic!("run must fail without an omp executable"),
    };
    assert!(
        matches!(
            error,
            HarnessError::AgentMissing(HarnessId::Omp) | HarnessError::NotInstalled(_)
        ),
        "{error}"
    );
    drop(steer_tx);
}

// Live probe against the real `omp` on PATH. Run manually:
//   cargo test -p agent_harness --test omp live_omp -- --ignored --nocapture
#[tokio::test]
#[ignore]
async fn live_omp_session() {
    let cwd = std::env::var("OMP_LIVE_CWD").unwrap_or_else(|_| "/tmp".into());
    let harness = AcpHarness::omp();
    let req = RunRequest {
        prompt: "say hi in one short sentence".into(),
        model: std::env::var("OMP_LIVE_MODEL").ok().filter(|m| !m.is_empty()),
        reasoning: None,
        model_options: serde_json::Map::new(),
        cwd: cwd.clone(),
        permission: PermissionMode::FullAccess,
        resume: None,
        fork: None,
        attachments: Vec::new(),
        skills: Vec::new(),
        auto_compact_tokens: None,
        chrome: false,
        drop_prompt: None,
        ephemeral: false,
    };
    let (steer_tx, steer_rx) = mpsc::channel(8);
    let controls = RunControls {
        request_input: Box::new(|questions| {
            let (tx, rx) = oneshot::channel();
            let _ = questions;
            tx.send(Vec::new()).expect("send");
            rx
        }),
        steering: steer_rx,
        interrupt: CancellationToken::new(),
        permission: tokio::sync::watch::channel(PermissionMode::FullAccess).1,
    };
    let mut stream = harness.run(req, controls).await.expect("run starts");
    let mut count = 0;
    while let Some(event) =
        tokio::time::timeout(Duration::from_secs(30), stream.next()).await.expect("in time")
    {
        let event = event.expect("stream event");
        eprintln!("{event:?}");
        count += 1;
        if matches!(event, AgentEvent::Done { .. }) {
            break;
        }
    }
    eprintln!("total events: {count}");
    drop(steer_tx);
}
