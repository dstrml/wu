use i18n::{SharedString, t};

pub fn single_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn plural(count: usize, one: SharedString, many: SharedString) -> String {
    if count == 1 {
        one.to_string()
    } else {
        many.to_string()
    }
}

fn search_detail(pattern: &str, path: Option<&str>) -> String {
    match path {
        Some(path) => t!("{pattern} in {path}", pattern = pattern, path = path).to_string(),
        None => pattern.to_owned(),
    }
}

pub fn tool_chip_content(call: &crate::ToolCall) -> (SharedString, String) {
    let (label, detail) = tool_chip_content_raw(call);
    (label, single_line(&detail))
}

fn tool_chip_content_raw(call: &crate::ToolCall) -> (SharedString, String) {
    use crate::ToolCall;
    match call {
        ToolCall::Exec { command } => (t!("Run"), command.clone()),
        ToolCall::ReadFile { path } => (t!("Read"), path.clone()),
        ToolCall::WriteFile { path, .. } => (t!("Write"), path.clone()),
        ToolCall::EditFile { path, .. } => (t!("Edit"), path.clone()),
        ToolCall::ApplyPatch { path } => (
            t!("Patch"),
            path.clone().unwrap_or_else(|| t!("workspace").to_string()),
        ),
        ToolCall::Search { pattern, path } => {
            (t!("Search"), search_detail(pattern, path.as_deref()))
        }
        ToolCall::Glob { pattern } => (t!("Glob"), pattern.clone()),
        ToolCall::WebFetch { url, .. } => (t!("Fetch"), url.clone()),
        ToolCall::WebSearch { query } => (t!("Web"), query.clone()),
        ToolCall::Todo { items } => {
            let done = items.iter().filter(|i| i.done).count();
            (
                t!("Todo"),
                t!("{done}/{total} done", done = done, total = items.len()).to_string(),
            )
        }
        ToolCall::Mcp { server, tool, .. } => ("MCP".into(), format!("{server} · {tool}")),
        ToolCall::Unknown { name, .. } => match name.strip_prefix("Agent: ") {
            Some(description) => (t!("Agent"), description.to_owned()),
            None if name == "Agent" => (t!("Agent"), String::new()),
            None => (t!("Tool"), tool_name_label(name)),
        },
    }
}

fn tool_name_label(name: &str) -> String {
    match name {
        "Generate image" => t!("Generate image"),
        "Send agent message" => t!("Send agent message"),
        "Wait for agents" => t!("Wait for agents"),
        "Close agent" => t!("Close agent"),
        "Resume agent" => t!("Resume agent"),
        _ => {
            return i18n::translate_filled(name, &[crate::codex::AGENT_CONTROL_TOOL])
                .map_or_else(|| name.to_owned(), |label| label.to_string());
        }
    }
    .to_string()
}

pub fn tool_group_summary(tools: &[(crate::ToolCall, bool)]) -> String {
    use crate::ToolCall;
    let mut commands = 0usize;
    let mut edited: Vec<&str> = Vec::new();
    let mut reads = 0usize;
    let mut searches = 0usize;
    let mut fetches = 0usize;
    let mut todos = 0usize;
    let mut other = 0usize;
    let mut failed = 0usize;
    for (call, is_error) in tools {
        if *is_error {
            failed += 1;
        }
        match call {
            ToolCall::Exec { .. } => commands += 1,
            ToolCall::WriteFile { path, .. } | ToolCall::EditFile { path, .. } => {
                if !edited.contains(&path.as_str()) {
                    edited.push(path);
                }
            }
            ToolCall::ApplyPatch { path } => {
                let p = path.as_deref().unwrap_or("patch");
                if !edited.contains(&p) {
                    edited.push(p);
                }
            }
            ToolCall::ReadFile { .. } => reads += 1,
            ToolCall::Search { .. } | ToolCall::Glob { .. } | ToolCall::WebSearch { .. } => {
                searches += 1
            }
            ToolCall::WebFetch { .. } => fetches += 1,
            ToolCall::Todo { .. } => todos += 1,
            ToolCall::Mcp { .. } | ToolCall::Unknown { .. } => other += 1,
        }
    }
    let mut segments: Vec<String> = Vec::new();
    if commands > 0 {
        segments.push(plural(
            commands,
            t!("ran 1 command"),
            t!("ran {count} commands", count = commands),
        ));
    }
    if !edited.is_empty() {
        segments.push(plural(
            edited.len(),
            t!("edited 1 file"),
            t!("edited {count} files", count = edited.len()),
        ));
    }
    if reads > 0 {
        segments.push(plural(
            reads,
            t!("read 1 file"),
            t!("read {count} files", count = reads),
        ));
    }
    if searches > 0 {
        segments.push(plural(
            searches,
            t!("searched 1 time"),
            t!("searched {count} times", count = searches),
        ));
    }
    if fetches > 0 {
        segments.push(plural(
            fetches,
            t!("fetched 1 page"),
            t!("fetched {count} pages", count = fetches),
        ));
    }
    if todos > 0 {
        segments.push(t!("updated todos").to_string());
    }
    if other > 0 {
        segments.push(plural(
            other,
            t!("called 1 tool"),
            t!("called {count} tools", count = other),
        ));
    }
    if segments.is_empty() {
        segments.push(plural(
            tools.len(),
            t!("1 tool"),
            t!("{count} tools", count = tools.len()),
        ));
    }
    if failed > 0 {
        segments.push(t!("{count} failed", count = failed).to_string());
    }
    let mut summary = segments.join(" · ");
    if let Some(first) = summary.get(0..1) {
        let upper = first.to_uppercase();
        summary.replace_range(0..1, &upper);
    }
    summary
}

pub fn tool_call_text(call: &crate::ToolCall) -> String {
    use crate::ToolCall;
    match call {
        ToolCall::Exec { command } => command.clone(),
        ToolCall::ReadFile { path } => path.clone(),
        ToolCall::WriteFile { path, content } => match content {
            Some(content) => format!("{path}\n{content}"),
            None => path.clone(),
        },
        ToolCall::EditFile { path, .. } => path.clone(),
        ToolCall::ApplyPatch { path } => {
            path.clone().unwrap_or_else(|| t!("workspace").to_string())
        }
        ToolCall::Search { pattern, path } => search_detail(pattern, path.as_deref()),
        ToolCall::Glob { pattern } => pattern.clone(),
        ToolCall::WebFetch { url, prompt } => match prompt {
            Some(prompt) => format!("{url}\n{prompt}"),
            None => url.clone(),
        },
        ToolCall::WebSearch { query } => query.clone(),
        ToolCall::Todo { items } => items
            .iter()
            .map(|item| {
                let mark = match item.status() {
                    crate::TodoStatus::Completed => "[x]",
                    crate::TodoStatus::InProgress => "[~]",
                    crate::TodoStatus::Pending => "[ ]",
                };
                format!("{mark} {}", item.text)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        ToolCall::Mcp {
            server,
            tool,
            input,
        } => match input {
            Some(input) => format!(
                "{server} · {tool}\n{}",
                serde_json::to_string_pretty(input).unwrap_or_default()
            ),
            None => format!("{server} · {tool}"),
        },
        ToolCall::Unknown { name, input } => match input {
            Some(input) => format!(
                "{name}\n{}",
                serde_json::to_string_pretty(input).unwrap_or_default()
            ),
            None => name.clone(),
        },
    }
}

pub fn default_model(models: &[crate::Model]) -> Option<&crate::Model> {
    models.first()
}

/// An explicit selection never silently becomes a different model after refresh.
pub fn selected_catalog_model<'a>(
    models: &'a [crate::Model],
    selected: Option<&str>,
) -> Option<&'a crate::Model> {
    match selected {
        Some(id) => models.iter().find(|model| model.id == id).or_else(|| {
            let (base, size) = crate::claude::catalog::split_context_size(id);
            size.and_then(|_| models.iter().find(|model| model.id == base))
        }),
        None => default_model(models),
    }
}

pub fn default_reasoning(ladder: &[crate::ReasoningLevel]) -> Option<crate::ReasoningLevel> {
    if ladder.contains(&crate::ReasoningLevel::High) {
        return Some(crate::ReasoningLevel::High);
    }
    if ladder.contains(&crate::ReasoningLevel::Medium) {
        return Some(crate::ReasoningLevel::Medium);
    }
    ladder.first().copied()
}

pub fn clamp_reasoning(
    level: Option<crate::ReasoningLevel>,
    ladder: &[crate::ReasoningLevel],
) -> Option<crate::ReasoningLevel> {
    match level {
        Some(level) if ladder.contains(&level) => Some(level),
        _ => default_reasoning(ladder),
    }
}

pub fn permission_label(mode: crate::PermissionMode, harness: crate::HarnessId) -> SharedString {
    use crate::PermissionMode;
    match mode {
        PermissionMode::Ask => t!("Ask"),
        PermissionMode::AcceptEdits => t!("Accept edits"),
        PermissionMode::Auto => t!("Auto"),
        PermissionMode::DontAsk => t!("Don't ask"),
        PermissionMode::ReadOnly => t!("Read only"),
        PermissionMode::ApproveForMe => t!("Approve for me"),
        PermissionMode::FullAccess => match harness {
            crate::HarnessId::ClaudeCode => t!("Bypass"),
            crate::HarnessId::Codex | crate::HarnessId::Opencode => t!("Full access"),
        },
    }
}

pub fn permission_description(
    mode: crate::PermissionMode,
    harness: crate::HarnessId,
) -> SharedString {
    use crate::{HarnessId, PermissionMode};
    match (mode, harness) {
        (PermissionMode::Ask, _) => t!("Asks before commands and file edits"),
        (PermissionMode::AcceptEdits, _) => t!("Edits files freely, asks before commands"),
        (PermissionMode::Auto, HarnessId::ClaudeCode) => {
            t!("Approves routine actions, asks about risky ones")
        }
        (PermissionMode::Auto, HarnessId::Codex) => {
            t!("Works freely in the project, asks to go beyond")
        }
        (PermissionMode::Auto, HarnessId::Opencode) => {
            t!("Follows your OpenCode permission settings")
        }
        (PermissionMode::DontAsk, _) => t!("Never asks, denies anything not already allowed"),
        (PermissionMode::ReadOnly, _) => t!("Reads files, asks before changing anything"),
        (PermissionMode::ApproveForMe, _) => t!("An automatic reviewer answers approval requests"),
        (PermissionMode::FullAccess, HarnessId::ClaudeCode) => t!("Skips every permission check"),
        (PermissionMode::FullAccess, HarnessId::Codex) => t!("No sandbox and no approval requests"),
        (PermissionMode::FullAccess, HarnessId::Opencode) => t!("Never asks for permission"),
    }
}

pub fn reasoning_label(level: crate::ReasoningLevel) -> SharedString {
    match level {
        crate::ReasoningLevel::Minimal => t!("Minimal"),
        crate::ReasoningLevel::Low => t!("Low"),
        crate::ReasoningLevel::Medium => t!("Medium"),
        crate::ReasoningLevel::High => t!("High"),
        crate::ReasoningLevel::XHigh => t!("X-High"),
        crate::ReasoningLevel::Max => t!("Max"),
        crate::ReasoningLevel::Ultra => "Ultra".into(),
        crate::ReasoningLevel::Ultracode => "Ultracode".into(),
        crate::ReasoningLevel::Ultrathink => "Ultrathink".into(),
    }
}

/// Harnesses apply some options blindly, so picks a model no longer offers must be dropped.
pub fn offered_options(
    model: &crate::Model,
    mut selections: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    selections.retain(|id, choice| {
        model.options.iter().any(|option| {
            option.id == *id
                && choice
                    .as_str()
                    .is_some_and(|choice| option.choices.iter().any(|c| c.id == choice))
        })
    });
    selections
}

pub fn fast_mode_values(option: &crate::ModelOption) -> Option<(&str, &str)> {
    let has = |id: &str| option.choices.iter().any(|choice| choice.id == id);
    let on = if matches!(option.id.as_str(), "fastMode" | "fast_mode") && has("on") {
        "on"
    } else if option.id == "fast" && has("true") {
        "true"
    } else if has("fast") {
        "fast"
    } else {
        return None;
    };
    let off = if option.default_choice != on && has(&option.default_choice) {
        option.default_choice.as_str()
    } else {
        option
            .choices
            .iter()
            .map(|choice| choice.id.as_str())
            .find(|id| *id != on)?
    };
    Some((on, off))
}
