#![cfg(unix)]

use std::path::PathBuf;
use std::time::Duration;

use futures::StreamExt;
use tokio::sync::{mpsc, oneshot};

use agent_harness::{
    AcpHarness, AgentEvent, CancellationToken, DoneStatus, Harness, HarnessError, HarnessId,
    PermissionMode, ReasoningLevel, RunControls, RunRequest, UserInputAnswer,
};

fn fixture_path() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("fake-pi.sh");
    use std::os::unix::fs::PermissionsExt;
    if let Err(error) = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)) {
        eprintln!("could not mark fake-pi.sh executable: {error}");
    }
    path
}

fn harness() -> AcpHarness {
    AcpHarness::pi().with_executable(fixture_path())
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

fn controls(answer_label: &'static str) -> RunControls {
    let (_steer_tx, steer_rx) = mpsc::channel(8);
    RunControls {
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
        interrupt: CancellationToken::new(),
        permission: tokio::sync::watch::channel(PermissionMode::Ask).1,
    }
}

async fn run_to_end(req: RunRequest, controls: RunControls) -> Vec<AgentEvent> {
    let stream = harness().run(req, controls).await.expect("run starts");
    tokio::time::timeout(
        Duration::from_secs(15),
        stream.map(|result| result.expect("stream event")).collect::<Vec<_>>(),
    )
    .await
    .expect("run finished in time")
}

fn done(events: &[AgentEvent]) -> DoneStatus {
    let dones: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::Done { status, .. } => Some(*status),
            _ => None,
        })
        .collect();
    assert_eq!(dones.len(), 1, "{events:?}");
    dones[0]
}

#[tokio::test]
async fn happy_path_reports_pi_identity_and_deltas() {
    let events = run_to_end(request("happy"), controls("Yes")).await;

    let starts: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::SessionStarted { harness, model, cwd, session_id, .. } => {
                Some((*harness, model.clone(), cwd.clone(), session_id.clone()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(starts.len(), 1, "{events:?}");
    let (harness_id, model, cwd, session_id) = &starts[0];
    assert_eq!(*harness_id, HarnessId::Pi);
    assert_eq!(model, "m1");
    assert_eq!(cwd, "/tmp");
    assert_eq!(session_id, "s-1");
    assert!(events.contains(&AgentEvent::ReasoningDelta { text: "thinking".into() }));
    assert!(events.contains(&AgentEvent::TextDelta { text: "Hello".into() }));
    assert!(events.contains(&AgentEvent::TextDelta { text: " world".into() }));
    assert!(events.contains(&AgentEvent::ContextUsage { tokens: Some(900), window: None }));
    assert_eq!(done(&events), DoneStatus::Completed);
}

#[tokio::test]
async fn full_access_auto_answers_yes() {
    let mut req = request("permission");
    req.permission = PermissionMode::FullAccess;
    let events = run_to_end(req, controls("No")).await;
    // The fixture echoes the chosen option id into the tool output.
    let tool_output = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolResult { output, .. } => output.clone(),
            _ => None,
        })
        .next()
        .expect("tool result");
    assert_eq!(tool_output, "yes");
    assert_eq!(done(&events), DoneStatus::Completed);
}

#[tokio::test]
async fn ask_mode_surfaces_yes_and_no() {
    let events = run_to_end(request("permission"), controls("Yes")).await;
    let tool_output = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolResult { output, .. } => output.clone(),
            _ => None,
        })
        .next()
        .expect("tool result");
    assert_eq!(tool_output, "yes");
}

#[tokio::test]
async fn catalog_resolves_the_thought_level_option_id() {
    let catalog = tokio::time::timeout(Duration::from_secs(15), harness().model_catalog(false))
        .await
        .expect("catalog in time")
        .expect("catalog ok");
    assert_eq!(catalog.models.len(), 2, "{catalog:?}");
    assert_eq!(catalog.models[0].id, "m1");
    // Thinking options carry XHigh even though pi names the option 'thought_level'.
    assert!(catalog.models[0]
        .reasoning_levels
        .contains(&ReasoningLevel::XHigh));
}

#[tokio::test]
async fn missing_pi_adapter_reports_agent_missing() {
    let harness = AcpHarness::pi().with_executable("/nonexistent/pi-acp-fake");
    let (_steer_tx, steer_rx) = mpsc::channel(8);
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
    let error = match harness.run(request("happy"), controls).await {
        Err(error) => error,
        Ok(_) => panic!("run must fail without the pi-acp adapter"),
    };
    assert!(
        matches!(
            error,
            HarnessError::AgentMissing(HarnessId::Pi) | HarnessError::NotInstalled(_)
        ),
        "{error}"
    );
}

#[tokio::test]
async fn resume_reuses_the_requested_session_id() {
    let mut req = request("happy");
    req.resume = Some("s-1".into());
    let events = run_to_end(req, controls("Yes")).await;
    // pi-acp's load response carries no sessionId; the requested id is reused.
    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::SessionStarted { harness: HarnessId::Pi, session_id, .. } if session_id == "s-1"
    )));
    assert_eq!(done(&events), DoneStatus::Completed);
}
