use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Debug)]
pub(crate) enum Frame {
    System(SystemFrame),
    StreamEvent(StreamEventFrame),
    Assistant(MessageFrame),
    User(MessageFrame),
    RateLimit(RateLimitFrame),
    Result(ResultFrame),
    ControlRequest(ControlRequestFrame),
    Other,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct SystemFrame {
    #[serde(default)]
    pub subtype: String,
    #[serde(default, rename = "permissionMode")]
    pub permission_mode: Option<String>,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default, alias = "toolUseId")]
    pub tool_use_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default, alias = "taskId")]
    pub task_id: Option<String>,
    #[serde(default)]
    pub subagent_type: Option<String>,
    #[serde(default, alias = "compactMetadata")]
    pub compact_metadata: Option<Value>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub task_type: Option<String>,
    #[serde(default)]
    pub tasks: Vec<WireBackgroundTask>,
    #[serde(default)]
    pub compact_result: Option<String>,
    #[serde(default)]
    pub compact_error: Option<String>,
    #[serde(default)]
    pub attempt: Option<Value>,
    #[serde(default)]
    pub max_retries: Option<Value>,
    #[serde(default)]
    pub retry_delay_ms: Option<Value>,
    #[serde(default)]
    pub error_status: Option<Value>,
    #[serde(default)]
    pub error: Option<Value>,
    #[serde(default)]
    pub no_response: Option<Value>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct WireBackgroundTask {
    #[serde(default)]
    pub task_id: String,
    #[serde(default)]
    pub task_type: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub ambient: bool,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct StreamEventFrame {
    #[serde(default)]
    pub parent_tool_use_id: Option<String>,
    #[serde(default)]
    pub event: StreamEventBody,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct StreamEventBody {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub delta: Delta,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct Delta {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub thinking: String,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct MessageFrame {
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub parent_tool_use_id: Option<String>,
    #[serde(default)]
    pub message: MessageBody,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct MessageBody {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub usage: Option<Value>,
    #[serde(default)]
    pub content: Value,
}

impl MessageBody {
    pub fn blocks(&self) -> impl Iterator<Item = ContentBlock> + '_ {
        self.content
            .as_array()
            .map(|a| a.as_slice())
            .unwrap_or_default()
            .iter()
            .filter_map(|b| serde_json::from_value(b.clone()).ok())
    }
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct ContentBlock {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub input: Value,
    #[serde(default)]
    pub tool_use_id: String,
    #[serde(default)]
    pub is_error: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct RateLimitFrame {
    #[serde(default)]
    pub rate_limit_info: RateLimitInfo,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct RateLimitInfo {
    #[serde(default)]
    pub status: String,
    #[serde(rename = "rateLimitType", default)]
    pub rate_limit_type: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct ResultFrame {
    #[serde(default, rename = "modelUsage")]
    pub model_usage: std::collections::BTreeMap<String, Value>,
    #[serde(default)]
    pub subtype: String,
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default)]
    pub errors: Vec<Value>,
    #[serde(default)]
    pub usage: UsageBody,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct UsageBody {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct ControlRequestFrame {
    #[serde(default)]
    pub request_id: String,
    #[serde(default)]
    pub request: ControlRequestBody,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct ControlRequestBody {
    #[serde(default)]
    pub subtype: String,
    #[serde(default)]
    pub tool_name: String,
    #[serde(default)]
    pub input: Value,
    #[serde(flatten)]
    pub other: serde_json::Map<String, Value>,
}

impl ControlRequestBody {
    pub fn text(&self, key: &str) -> Option<&str> {
        self.other
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
    }
}

/// The id of a control request too malformed to parse, so it can still be answered.
pub(crate) fn unreadable_control_request_id(line: &str) -> Option<String> {
    let value: Value = serde_json::from_str(line).ok()?;
    if value.get("type").and_then(Value::as_str) != Some("control_request") {
        return None;
    }
    value
        .get("request_id")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// The error Claude answered one of Wu's own control requests with.
pub(crate) fn control_error(line: &str, request_id_prefix: &str) -> Option<String> {
    if !line.contains(request_id_prefix) {
        return None;
    }
    let value: Value = serde_json::from_str(line).ok()?;
    let response = value.get("response")?;
    let rejected = value.get("type").and_then(Value::as_str) == Some("control_response")
        && response.get("subtype").and_then(Value::as_str) == Some("error")
        && response
            .get("request_id")
            .and_then(Value::as_str)
            .is_some_and(|id| id.starts_with(request_id_prefix));
    rejected.then(|| {
        response
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    })
}

pub(crate) fn control_response_to(line: &str, request_id: &str) -> Option<Value> {
    if !line.contains(request_id) {
        return None;
    }
    let mut value: Value = serde_json::from_str(line).ok()?;
    if value.get("type").and_then(Value::as_str) != Some("control_response") {
        return None;
    }
    let response = value.get_mut("response")?.take();
    (response.get("request_id").and_then(Value::as_str) == Some(request_id)).then_some(response)
}

pub(crate) fn parse_frame(line: &str) -> Result<Frame, serde_json::Error> {
    let value: Value = serde_json::from_str(line)?;
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
    let frame = match kind {
        "system" => Frame::System(serde_json::from_value(value)?),
        "stream_event" => Frame::StreamEvent(serde_json::from_value(value)?),
        "assistant" => Frame::Assistant(serde_json::from_value(value)?),
        "user" => Frame::User(serde_json::from_value(value)?),
        "rate_limit_event" => Frame::RateLimit(serde_json::from_value(value)?),
        "result" => Frame::Result(serde_json::from_value(value)?),
        "control_request" => Frame::ControlRequest(serde_json::from_value(value)?),
        _ => Frame::Other,
    };
    Ok(frame)
}

pub(crate) struct ImageBlock {
    pub media_type: &'static str,
    pub base64_data: String,
}

fn user_content(text: &str, images: &[ImageBlock]) -> Value {
    if images.is_empty() {
        return Value::String(text.to_owned());
    }
    let mut blocks: Vec<Value> = images
        .iter()
        .map(|image| {
            json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": image.media_type,
                    "data": image.base64_data,
                },
            })
        })
        .collect();
    // The API rejects empty text blocks, which image-only messages would otherwise carry.
    if !text.trim().is_empty() {
        blocks.push(json!({ "type": "text", "text": text }));
    }
    Value::Array(blocks)
}

pub(crate) fn user_message_line(text: &str, images: &[ImageBlock], id: &str) -> String {
    json!({
        "type": "user",
        "uuid": id,
        "message": { "role": "user", "content": user_content(text, images) },
        "parent_tool_use_id": null,
    })
    .to_string()
}

/// Only pass `immediate` when no tool is open: priority `now` aborts in-flight tool calls.
pub(crate) fn steer_message_line(
    text: &str,
    images: &[ImageBlock],
    id: &str,
    immediate: bool,
) -> String {
    json!({
        "type": "user",
        "uuid": id,
        "priority": if immediate { "now" } else { "next" },
        "message": { "role": "user", "content": user_content(text, images) },
        "parent_tool_use_id": null,
    })
    .to_string()
}

pub(crate) fn control_response_line(request_id: &str, response: Value) -> String {
    json!({
        "type": "control_response",
        "response": {
            "subtype": "success",
            "request_id": request_id,
            "response": response,
        },
    })
    .to_string()
}

/// The CLI rejects the pending request with this message instead of waiting on it.
pub(crate) fn control_error_line(request_id: &str, error: &str) -> String {
    json!({
        "type": "control_response",
        "response": {
            "subtype": "error",
            "request_id": request_id,
            "error": error,
        },
    })
    .to_string()
}

pub(crate) fn allow_response(updated_input: Value) -> Value {
    json!({ "behavior": "allow", "updatedInput": updated_input })
}

pub(crate) fn deny_response(message: &str) -> Value {
    json!({ "behavior": "deny", "message": message })
}

pub(crate) fn set_permission_mode_line(request_id: &str, mode: &str) -> String {
    json!({
        "type": "control_request",
        "request_id": request_id,
        "request": { "subtype": "set_permission_mode", "mode": mode },
    })
    .to_string()
}

pub(crate) fn set_model_line(request_id: &str, model: &str) -> String {
    json!({
        "type": "control_request",
        "request_id": request_id,
        "request": { "subtype": "set_model", "model": model },
    })
    .to_string()
}

pub(crate) fn initialize_request_line(request_id: &str) -> String {
    json!({
        "type": "control_request",
        "request_id": request_id,
        "request": { "subtype": "initialize" },
    })
    .to_string()
}

pub(crate) fn interrupt_request_line(request_id: &str) -> String {
    json!({
        "type": "control_request",
        "request_id": request_id,
        "request": { "subtype": "interrupt" },
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_errors_are_read_only_for_wu_requests() {
        let rejected = r#"{"type":"control_response","response":{"subtype":"error","request_id":"wu-set-mode-1","error":"auto mode unavailable for this model"}}"#;
        assert_eq!(
            control_error(rejected, "wu-set-mode-").as_deref(),
            Some("auto mode unavailable for this model")
        );
        let accepted = r#"{"type":"control_response","response":{"subtype":"success","request_id":"wu-set-mode-1"}}"#;
        assert_eq!(control_error(accepted, "wu-set-mode-"), None);
        let other = r#"{"type":"control_response","response":{"subtype":"error","request_id":"int_1","error":"x"}}"#;
        assert_eq!(control_error(other, "wu-set-mode-"), None);
    }

    #[test]
    fn control_responses_are_matched_by_request_id() {
        let answer = r#"{"type":"control_response","response":{"subtype":"success","request_id":"wu-run-initialize","response":{"commands":[]}}}"#;
        let response = control_response_to(answer, "wu-run-initialize").expect("matched");
        assert_eq!(response["response"]["commands"], json!([]));
        assert_eq!(control_response_to(answer, "wu-other"), None);
        let echoed = r#"{"type":"user","message":{"content":"wu-run-initialize"}}"#;
        assert_eq!(control_response_to(echoed, "wu-run-initialize"), None);
    }

    #[test]
    fn image_only_messages_carry_no_empty_text_block() {
        let image = ImageBlock {
            media_type: "image/png",
            base64_data: "AAAA".into(),
        };
        let content = user_content("  ", std::slice::from_ref(&image));
        assert_eq!(content.as_array().map(Vec::len), Some(1));
        assert_eq!(content[0]["type"], "image");
        assert_eq!(user_content("hi", &[]), serde_json::Value::String("hi".into()));
    }

    #[test]
    fn steer_priority_follows_tool_state() {
        let now: serde_json::Value =
            serde_json::from_str(&steer_message_line("hi", &[], "u1", true)).unwrap();
        let next: serde_json::Value =
            serde_json::from_str(&steer_message_line("hi", &[], "u2", false)).unwrap();
        assert_eq!(now["priority"], "now");
        assert_eq!(next["priority"], "next");
        assert_eq!(next["uuid"], "u2");
    }

    #[test]
    fn parses_known_and_unknown_frames() {
        let init = r#"{"type":"system","subtype":"init","model":"m","tools":["Bash"],"cwd":"/x","session_id":"s1"}"#;
        match parse_frame(init).expect("parses") {
            Frame::System(f) => {
                assert_eq!(f.subtype, "init");
                assert_eq!(f.session_id, "s1");
            }
            other => panic!("unexpected frame: {other:?}"),
        }
        assert!(matches!(
            parse_frame(r#"{"type":"mystery_frame"}"#).expect("parses"),
            Frame::Other
        ));
        assert!(parse_frame("not json").is_err());
    }

    #[test]
    fn user_line_shape_matches_protocol() {
        let line = user_message_line("hi", &[], "u1");
        let v: Value = serde_json::from_str(&line).expect("json");
        assert_eq!(v["type"], "user");
        assert_eq!(v["uuid"], "u1");
        assert_eq!(v["message"]["content"], "hi");
        assert!(v["parent_tool_use_id"].is_null());
    }

    #[test]
    fn images_precede_the_text_block() {
        let images = [ImageBlock {
            media_type: "image/png",
            base64_data: "AAAA".into(),
        }];
        for line in [
            user_message_line("look", &images, "u1"),
            steer_message_line("look", &images, "u1", true),
        ] {
            let v: Value = serde_json::from_str(&line).expect("json");
            let content = v["message"]["content"].as_array().expect("blocks");
            assert_eq!(content.len(), 2);
            assert_eq!(content[0]["type"], "image");
            assert_eq!(content[0]["source"]["type"], "base64");
            assert_eq!(content[0]["source"]["media_type"], "image/png");
            assert_eq!(content[0]["source"]["data"], "AAAA");
            assert_eq!(content[1], json!({ "type": "text", "text": "look" }));
        }
    }
}
