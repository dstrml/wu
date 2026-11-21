use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::{
    AgentEvent, ExternalSession, ReasoningLevel, SlashCommand, TodoItem, TodoStatus, ToolCall,
    ToolDiff,
};

pub(super) struct Feed {
    history: bool,
    message_id: String,
    text_open: bool,
    pending_prompt: Option<String>,
    // Later updates to a tool call may leave out its kind.
    kinds: HashMap<String, String>,
    finished: HashSet<String>,
    command_output: fn(&str) -> &str,
}

impl Feed {
    pub(super) fn live(message_id: String, command_output: fn(&str) -> &str) -> Self {
        Self {
            history: false,
            message_id,
            text_open: false,
            pending_prompt: None,
            kinds: HashMap::new(),
            finished: HashSet::new(),
            command_output,
        }
    }

    pub(super) fn history(command_output: fn(&str) -> &str) -> Self {
        Self {
            history: true,
            ..Self::live(String::new(), command_output)
        }
    }

    pub(super) fn message_id(&self) -> &str {
        &self.message_id
    }

    pub(super) fn rotate(&mut self) -> (String, String) {
        self.text_open = false;
        let next = if self.history {
            String::new()
        } else {
            uuid::Uuid::new_v4().to_string()
        };
        let previous = std::mem::replace(&mut self.message_id, next);
        (previous, self.message_id.clone())
    }

    pub(super) fn finish_message(&mut self) -> AgentEvent {
        let (previous, _) = self.rotate();
        AgentEvent::AssistantMessageCompleted {
            assistant_message_id: previous,
        }
    }

    fn close_text(&mut self, events: &mut Vec<AgentEvent>) {
        if self.text_open {
            events.push(self.finish_message());
        }
    }

    pub(super) fn close_tools(&mut self) {
        self.finished.extend(self.kinds.keys().cloned());
    }

    pub(super) fn finish(&mut self) -> Vec<AgentEvent> {
        let mut events = Vec::new();
        if let Some(text) = self.pending_prompt.take() {
            events.push(AgentEvent::UserMessage { text });
        }
        self.close_text(&mut events);
        events
    }

    pub(super) fn update(&mut self, update: &Value) -> Vec<AgentEvent> {
        let mut events = Vec::new();
        let kind = str_at(update, "sessionUpdate");
        // A prompt can arrive in several chunks, so it's only complete once something else follows.
        if kind != "user_message_chunk"
            && let Some(text) = self.pending_prompt.take()
        {
            events.push(AgentEvent::UserMessage { text });
        }
        match kind {
            "agent_message_chunk" => {
                if let Some(text) = chunk_text(update) {
                    self.text_open = true;
                    events.push(AgentEvent::TextDelta { text });
                }
            }
            "agent_thought_chunk" => {
                if let Some(text) = chunk_text(update) {
                    self.close_text(&mut events);
                    events.push(AgentEvent::ReasoningDelta { text });
                }
            }
            "user_message_chunk" if self.history => {
                if let Some(text) = chunk_text(update) {
                    self.close_text(&mut events);
                    self.pending_prompt.get_or_insert_default().push_str(&text);
                }
            }
            "tool_call" | "tool_call_update" => self.tool(update, &mut events),
            "plan" => {
                self.close_text(&mut events);
                events.extend(plan_events(update));
            }
            "usage_update" if !self.history => events.push(AgentEvent::ContextUsage {
                tokens: update.get("used").and_then(Value::as_u64),
                window: update.get("size").and_then(Value::as_u64),
            }),
            "session_info_update" if !self.history => {
                if let Some(title) = update
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|title| !title.is_empty())
                {
                    events.push(AgentEvent::TitleChanged {
                        title: title.to_owned(),
                    });
                }
            }
            "available_commands_update" if !self.history => {
                events.push(AgentEvent::AvailableCommands {
                    commands: commands(update),
                });
            }
            _ => {}
        }
        events
    }

    fn tool(&mut self, update: &Value, events: &mut Vec<AgentEvent>) {
        let Some(id) = update
            .get("toolCallId")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
        else {
            return;
        };
        if self.finished.contains(id) {
            return;
        }
        if !self.kinds.contains_key(id) {
            self.kinds
                .insert(id.to_owned(), str_at(update, "kind").to_owned());
            self.close_text(events);
            events.push(AgentEvent::ToolCall {
                id: id.to_owned(),
                call: tool_call(update),
            });
        }
        let status = str_at(update, "status");
        if matches!(status, "completed" | "failed") {
            self.finished.insert(id.to_owned());
            let kind = self.kinds.get(id).map_or("", String::as_str);
            let (output, diff) = tool_output(update, kind, self.command_output);
            events.push(AgentEvent::ToolResult {
                id: id.to_owned(),
                is_error: status == "failed"
                    || update
                        .pointer("/rawOutput/exit_code")
                        .and_then(Value::as_i64)
                        .is_some_and(|code| code != 0),
                output,
                diff,
            });
        }
    }
}

pub(super) fn is_turn_content(update: &Value) -> bool {
    matches!(
        str_at(update, "sessionUpdate"),
        "agent_message_chunk" | "agent_thought_chunk" | "tool_call" | "tool_call_update" | "plan"
    )
}

fn str_at<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn chunk_text(update: &Value) -> Option<String> {
    let content = update.get("content")?;
    (str_at(content, "type") == "text")
        .then(|| str_at(content, "text"))
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn first_str(value: Option<&Value>, keys: &[&str]) -> Option<String> {
    let value = value?;
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn tool_input(update: &Value) -> Option<&Value> {
    update
        .get("rawInput")
        .or_else(|| update.get("args"))
        .filter(|input| input.is_object())
}

fn tool_path(update: &Value) -> Option<String> {
    update
        .pointer("/locations/0/path")
        .and_then(Value::as_str)
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            first_str(
                tool_input(update),
                &[
                    "Path",
                    "path",
                    "AbsolutePath",
                    "TargetFile",
                    "file_path",
                    "filePath",
                ],
            )
        })
}

pub(super) fn tool_call(update: &Value) -> ToolCall {
    let input = tool_input(update);
    let name = str_at(update, "name");
    let title = first_str(Some(update), &["title"]);
    let label = title.clone().unwrap_or_else(|| name.to_owned());
    match str_at(update, "kind") {
        "execute" => ToolCall::Exec {
            command: first_str(input, &["CommandLine", "command", "Command", "cmd"])
                .or(title)
                .unwrap_or_default(),
        },
        "read" => match tool_path(update) {
            Some(path) => ToolCall::ReadFile { path },
            None => unknown(label, input),
        },
        "edit" => match tool_path(update) {
            Some(path) if name == "write_file" || name == "write_to_file" => ToolCall::WriteFile {
                path,
                content: input.and_then(|input| {
                    ["Content", "content", "CodeContent"]
                        .iter()
                        .find_map(|key| input.get(*key).and_then(Value::as_str))
                        .map(str::to_owned)
                }),
            },
            Some(path) => ToolCall::EditFile {
                path,
                old_string: None,
                new_string: None,
            },
            None => unknown(label, input),
        },
        "search" => {
            let pattern = first_str(input, &["Query", "query", "Pattern", "pattern"]);
            match pattern {
                Some(pattern) if name.contains("find") || name.contains("glob") => {
                    ToolCall::Glob { pattern }
                }
                Some(pattern) => ToolCall::Search {
                    pattern,
                    path: first_str(input, &["SearchPath", "Path", "path", "SearchDirectory"]),
                },
                None => unknown(label, input),
            }
        }
        "fetch" => {
            if let Some(url) = first_str(input, &["Url", "url", "URL"]) {
                ToolCall::WebFetch { url, prompt: None }
            } else if let Some(query) = first_str(input, &["Query", "query"]) {
                ToolCall::WebSearch { query }
            } else {
                unknown(label, input)
            }
        }
        _ => unknown(label, input),
    }
}

fn unknown(name: String, input: Option<&Value>) -> ToolCall {
    ToolCall::Unknown {
        name,
        input: input.cloned(),
    }
}

fn tool_output(
    update: &Value,
    kind: &str,
    command_output: fn(&str) -> &str,
) -> (Option<String>, Option<ToolDiff>) {
    let mut texts = Vec::new();
    let mut diff = None;
    for item in update
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match str_at(item, "type") {
            "content" => {
                if let Some(text) = item
                    .get("content")
                    .filter(|content| str_at(content, "type") == "text")
                    .map(|content| str_at(content, "text"))
                    .filter(|text| !text.is_empty())
                {
                    texts.push(text.to_owned());
                }
            }
            "diff" if diff.is_none() => {
                diff = item
                    .get("path")
                    .and_then(Value::as_str)
                    .map(|path| ToolDiff {
                        path: path.to_owned(),
                        old_text: item
                            .get("oldText")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        new_text: str_at(item, "newText").to_owned(),
                    });
            }
            _ => {}
        }
    }
    if texts.is_empty()
        && let Some(text) = update
            .pointer("/rawOutput/output")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
    {
        texts.push(text.to_owned());
    }
    let output = texts.join("\n");
    let output = if kind == "execute" {
        command_output(&output)
    } else {
        &output
    }
    .replace("\r\n", "\n");
    (
        Some(output).filter(|output| !output.trim().is_empty()),
        diff,
    )
}

fn plan_events(update: &Value) -> Vec<AgentEvent> {
    let items = update
        .get("entries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|entry| {
            TodoItem::new(
                str_at(entry, "content"),
                TodoStatus::parse(str_at(entry, "status")),
            )
        })
        .collect();
    vec![
        AgentEvent::ToolCall {
            id: crate::LIVE_PLAN_TOOL_ID.into(),
            call: ToolCall::Todo { items },
        },
        AgentEvent::ToolResult {
            id: crate::LIVE_PLAN_TOOL_ID.into(),
            is_error: false,
            output: None,
            diff: None,
        },
    ]
}

pub(super) fn commands(update: &Value) -> Vec<SlashCommand> {
    update
        .get("availableCommands")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|command| {
            let name = command.get("name")?.as_str()?.trim();
            (!name.is_empty()).then(|| SlashCommand {
                name: name.to_owned(),
                description: str_at(command, "description").to_owned(),
                input_hint: command
                    .pointer("/input/hint")
                    .and_then(Value::as_str)
                    .filter(|hint| !hint.is_empty())
                    .map(str::to_owned),
                is_skill: false,
            })
        })
        .collect()
}

pub(super) fn external_sessions(page: &Value) -> Vec<ExternalSession> {
    page.get("sessions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|session| {
            let id = session.get("sessionId")?.as_str()?.to_owned();
            let title = session.get("title")?.as_str()?.trim();
            if id.is_empty() || title.is_empty() {
                return None;
            }
            let updated_at = session
                .get("updatedAt")
                .and_then(Value::as_str)
                .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
                .map_or(0, |at| at.timestamp());
            Some(ExternalSession {
                id,
                title: title.to_owned(),
                updated_at,
            })
        })
        .collect()
}

pub(super) fn reasoning_level(value: &str) -> Option<ReasoningLevel> {
    Some(match value {
        "minimal" => ReasoningLevel::Minimal,
        "low" => ReasoningLevel::Low,
        "medium" => ReasoningLevel::Medium,
        "high" => ReasoningLevel::High,
        "xhigh" | "extra" => ReasoningLevel::XHigh,
        "max" => ReasoningLevel::Max,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn as_is(text: &str) -> &str {
        text
    }

    fn command_update(status: &str, output: Option<&str>) -> Value {
        let mut update = json!({
            "sessionUpdate": if status == "in_progress" { "tool_call" } else { "tool_call_update" },
            "toolCallId": "call_1",
            "name": "run_command",
            "kind": "execute",
            "status": status,
            "title": "ls -1",
            "rawInput": { "CommandLine": "ls -1", "WaitMsBeforeAsync": 5000 },
        });
        if let Some(output) = output {
            update["rawOutput"] = json!({ "output": output, "exit_code": 0 });
            update["content"] =
                json!([{ "type": "content", "content": { "type": "text", "text": output } }]);
        }
        update
    }

    #[test]
    fn a_command_starts_once_and_finishes_with_only_its_output() {
        let mut feed = Feed::live("m1".into(), as_is);
        let output = "hello.txt\r\nnotes.txt\r\n";
        let mut events = feed.update(&command_update("in_progress", None));
        events.extend(feed.update(&command_update("pending", None)));
        events.extend(feed.update(&command_update("completed", Some(output))));
        events.extend(feed.update(&command_update("completed", Some(output))));
        assert_eq!(
            events,
            vec![
                AgentEvent::ToolCall {
                    id: "call_1".into(),
                    call: ToolCall::Exec {
                        command: "ls -1".into()
                    },
                },
                AgentEvent::ToolResult {
                    id: "call_1".into(),
                    is_error: false,
                    output: Some("hello.txt\nnotes.txt\n".into()),
                    diff: None,
                },
            ]
        );
    }

    #[test]
    fn file_tools_map_to_reads_writes_and_edits_with_diffs() {
        let read = json!({
            "toolCallId": "r", "name": "read_file", "kind": "read",
            "locations": [{ "path": "/p/notes.txt" }],
        });
        assert_eq!(
            tool_call(&read),
            ToolCall::ReadFile {
                path: "/p/notes.txt".into()
            }
        );
        let write = json!({
            "toolCallId": "w", "name": "write_file", "kind": "edit",
            "rawInput": { "Content": "hi\n", "Path": "/p/hello.txt" },
        });
        assert_eq!(
            tool_call(&write),
            ToolCall::WriteFile {
                path: "/p/hello.txt".into(),
                content: Some("hi\n".into()),
            }
        );
        let edit = json!({
            "sessionUpdate": "tool_call_update", "toolCallId": "e", "name": "replace_file_content",
            "kind": "edit", "status": "completed", "locations": [{ "path": "/p/a.rs" }],
            "content": [{ "type": "diff", "path": "/p/a.rs", "oldText": "a", "newText": "b" }],
        });
        let events = Feed::live("m".into(), as_is).update(&edit);
        assert_eq!(
            events,
            vec![
                AgentEvent::ToolCall {
                    id: "e".into(),
                    call: ToolCall::EditFile {
                        path: "/p/a.rs".into(),
                        old_string: None,
                        new_string: None,
                    },
                },
                AgentEvent::ToolResult {
                    id: "e".into(),
                    is_error: false,
                    output: None,
                    diff: Some(ToolDiff {
                        path: "/p/a.rs".into(),
                        old_text: Some("a".into()),
                        new_text: "b".into(),
                    }),
                },
            ]
        );
    }

    #[test]
    fn a_finishing_update_without_its_kind_still_cleans_command_output() {
        fn after_marker(text: &str) -> &str {
            text.split_once("output:\n")
                .map_or(text, |(_, output)| output)
        }
        let mut feed = Feed::live("m".into(), after_marker);
        feed.update(&command_update("in_progress", None));
        let events = feed.update(&json!({
            "sessionUpdate": "tool_call_update",
            "toolCallId": "call_1",
            "status": "completed",
            "rawOutput": { "output": "exit 0\noutput:\nfoo" },
        }));
        assert_eq!(
            events,
            vec![AgentEvent::ToolResult {
                id: "call_1".into(),
                is_error: false,
                output: Some("foo".into()),
                diff: None,
            }]
        );
    }

    #[test]
    fn a_failed_command_is_an_error() {
        let mut update = command_update("completed", Some("boom"));
        update["rawOutput"]["exit_code"] = json!(2);
        let events = Feed::live("m".into(), as_is).update(&update);
        assert!(matches!(
            events.last(),
            Some(AgentEvent::ToolResult { is_error: true, .. })
        ));
    }

    #[test]
    fn text_before_a_tool_closes_the_message() {
        let mut feed = Feed::live("first".into(), as_is);
        let text = |text: &str| json!({ "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": text } });
        let mut events = feed.update(&text("Look"));
        events.extend(feed.update(&text("ing")));
        events.extend(feed.update(&command_update("in_progress", None)));
        assert_eq!(events.len(), 4);
        assert_eq!(
            events[2],
            AgentEvent::AssistantMessageCompleted {
                assistant_message_id: "first".into()
            }
        );
        assert_ne!(feed.message_id(), "first");
    }

    #[test]
    fn history_replays_prompts_and_skips_live_only_updates() {
        let mut feed = Feed::history(as_is);
        let updates = [
            json!({ "sessionUpdate": "user_message_chunk", "content": { "type": "text", "text": "Do it" } }),
            json!({ "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": "Done" } }),
            json!({ "sessionUpdate": "usage_update", "used": 10, "size": 100 }),
            json!({ "sessionUpdate": "session_info_update", "title": "T" }),
            json!({ "sessionUpdate": "user_message_chunk", "content": { "type": "text", "text": "Again " } }),
            json!({ "sessionUpdate": "user_message_chunk", "content": { "type": "text", "text": "please" } }),
        ];
        let mut events: Vec<_> = updates.iter().flat_map(|u| feed.update(u)).collect();
        events.extend(feed.finish());
        assert_eq!(
            events,
            vec![
                AgentEvent::UserMessage {
                    text: "Do it".into()
                },
                AgentEvent::TextDelta {
                    text: "Done".into()
                },
                AgentEvent::AssistantMessageCompleted {
                    assistant_message_id: String::new()
                },
                AgentEvent::UserMessage {
                    text: "Again please".into()
                },
            ]
        );
    }

    #[test]
    fn live_updates_report_usage_titles_and_commands() {
        let mut feed = Feed::live("m".into(), as_is);
        assert_eq!(
            feed.update(&json!({ "sessionUpdate": "usage_update", "used": 9232, "size": 1048576 })),
            vec![AgentEvent::ContextUsage {
                tokens: Some(9232),
                window: Some(1048576)
            }]
        );
        assert_eq!(
            feed.update(&json!({ "sessionUpdate": "session_info_update", "title": "Fix it" })),
            vec![AgentEvent::TitleChanged {
                title: "Fix it".into()
            }]
        );
        assert_eq!(
            feed.update(&json!({
                "sessionUpdate": "available_commands_update",
                "availableCommands": [
                    { "name": "goal", "description": "Run until done." },
                    { "name": "", "description": "skipped" },
                    { "name": "plan", "description": "Plan first.", "input": { "hint": "task" } },
                ],
            })),
            vec![AgentEvent::AvailableCommands {
                commands: vec![
                    SlashCommand {
                        name: "goal".into(),
                        description: "Run until done.".into(),
                        input_hint: None,
                        is_skill: false,
                    },
                    SlashCommand {
                        name: "plan".into(),
                        description: "Plan first.".into(),
                        input_hint: Some("task".into()),
                        is_skill: false,
                    },
                ]
            }]
        );
    }

    #[test]
    fn plans_become_the_live_todo_list() {
        let events = Feed::live("m".into(), as_is).update(&json!({
            "sessionUpdate": "plan",
            "entries": [
                { "content": "Read", "priority": "high", "status": "completed" },
                { "content": "Write", "priority": "high", "status": "in_progress" },
            ],
        }));
        assert_eq!(
            events[0],
            AgentEvent::ToolCall {
                id: crate::LIVE_PLAN_TOOL_ID.into(),
                call: ToolCall::Todo {
                    items: vec![
                        TodoItem::new("Read", TodoStatus::Completed),
                        TodoItem::new("Write", TodoStatus::InProgress),
                    ]
                },
            }
        );
    }

    #[test]
    fn saved_chats_need_an_id_and_a_title() {
        let sessions = external_sessions(&json!({
            "sessions": [
                { "sessionId": "a", "title": "Run Shell Command", "updatedAt": "2026-11-21T17:28:46Z" },
                { "sessionId": "b", "title": "  " },
                { "title": "No id" },
            ]
        }));
        assert_eq!(
            sessions,
            vec![ExternalSession {
                id: "a".into(),
                title: "Run Shell Command".into(),
                updated_at: 1_795_282_126,
            }]
        );
    }
}
