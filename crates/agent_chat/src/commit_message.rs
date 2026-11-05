use std::{path::PathBuf, sync::Arc};

use agent_harness::{
    AgentEvent, CancellationToken, DoneStatus, Harness, Model, PermissionMode, ReasoningLevel,
    RunControls, RunRequest,
};
use anyhow::{Result, anyhow};
use futures::StreamExt as _;
use gpui::{App, AppContext as _, Entity, SharedString, Task, Window};
use i18n::t;
use language::LanguageRegistry;
use settings::Settings as _;
use ui::{ContextMenu, IconPosition};

use crate::{AgentChatSettings, AgentKind, session::AgentStore};

pub fn generate_commit_message(
    prompt: String,
    cwd: PathBuf,
    languages: Arc<LanguageRegistry>,
    cx: &mut App,
) -> Task<Result<String>> {
    let store = AgentStore::global(languages, cx);
    let settings = AgentChatSettings::get_global(cx);
    let chosen = settings.commit_message.agent;
    let chosen_model = settings.commit_message.model.clone();
    let kinds = match chosen {
        Some(kind) => vec![kind],
        None => settings.agents().to_vec(),
    };
    let candidates: Vec<Candidate> = kinds
        .into_iter()
        .map(|kind| {
            let discovered_models = store
                .read(cx)
                .discovered_models(kind)
                .map(<[Model]>::to_vec);
            let harness = store.update(cx, |store, cx| {
                if kind == AgentKind::Codex {
                    store.ensure_models(kind, cx);
                }
                store.harness(kind)
            });
            Candidate {
                kind,
                harness,
                discovered_models,
            }
        })
        .collect();
    let env_loaded = crate::session::shell_env_loaded(cx);
    let checks_install = chosen.is_none();
    let work = gpui_tokio::Tokio::spawn(cx, async move {
        env_loaded.await;
        let candidate = candidates
            .into_iter()
            .find(|candidate| !checks_install || candidate.harness.installed());
        let Some(candidate) = candidate else {
            return Err(anyhow!(t!(
                "Install Claude Code, Codex or OpenCode to write commit messages."
            )));
        };
        let model = match chosen_model {
            Some(model) => Some(model),
            None => small_model(&candidate).await,
        };
        run_once(candidate.kind, candidate.harness, model, prompt, cwd).await
    });
    cx.background_spawn(async move { work.await? })
}

struct Candidate {
    kind: AgentKind,
    harness: Arc<dyn Harness>,
    discovered_models: Option<Vec<Model>>,
}

async fn small_model(candidate: &Candidate) -> Option<String> {
    match candidate.kind {
        AgentKind::Claude => Some("haiku".to_owned()),
        AgentKind::Codex => {
            let models = match &candidate.discovered_models {
                Some(models) => models.clone(),
                None => match candidate.harness.models().await {
                    Ok(models) if !models.is_empty() => models,
                    _ => candidate.harness.fallback_models(),
                },
            };
            models
                .into_iter()
                .find(|model| model.id.contains("mini"))
                .map(|model| model.id)
        }
        AgentKind::Opencode => None,
    }
}

async fn run_once(
    kind: AgentKind,
    harness: Arc<dyn Harness>,
    model: Option<String>,
    prompt: String,
    cwd: PathBuf,
) -> Result<String> {
    let permission = match kind {
        AgentKind::Codex => PermissionMode::ReadOnly,
        AgentKind::Claude | AgentKind::Opencode => PermissionMode::Ask,
    };
    let reasoning = match kind {
        AgentKind::Claude | AgentKind::Codex => Some(ReasoningLevel::Low),
        AgentKind::Opencode => None,
    };
    let request = RunRequest {
        prompt,
        model,
        reasoning,
        model_options: serde_json::Map::new(),
        cwd: cwd.to_string_lossy().into_owned(),
        permission,
        resume: None,
        fork: None,
        attachments: Vec::new(),
        skills: Vec::new(),
        auto_compact_tokens: None,
        chrome: false,
        drop_prompt: None,
        ephemeral: true,
    };
    let (steering_tx, steering) = tokio::sync::mpsc::channel(1);
    let (permission_tx, permission_changes) = tokio::sync::watch::channel(permission);
    let interrupt = CancellationToken::new();
    let _interrupt_when_dropped = interrupt.clone().drop_guard();
    let controls = RunControls {
        request_input: Box::new(|_| {
            let (_, declined) = tokio::sync::oneshot::channel();
            declined
        }),
        steering,
        interrupt,
        permission: permission_changes,
    };
    let mut events = harness.run(request, controls).await?;
    let mut current = String::new();
    let mut finished = String::new();
    let mut last_error = None;
    while let Some(event) = events.next().await {
        match event? {
            AgentEvent::TextDelta { text } => current.push_str(&text),
            AgentEvent::AssistantMessageCompleted { .. } => {
                if !current.trim().is_empty() {
                    finished = std::mem::take(&mut current);
                }
            }
            AgentEvent::Error { message } => last_error = Some(message),
            AgentEvent::Done {
                status,
                result,
                error,
                ..
            } => {
                if status == DoneStatus::Errored {
                    let message = error
                        .or(last_error)
                        .unwrap_or_else(|| t!("The agent stopped without a reply.").to_string());
                    return Err(anyhow!(message));
                }
                if current.trim().is_empty() && finished.trim().is_empty() {
                    finished = result.unwrap_or_default();
                }
                break;
            }
            _ => {}
        }
    }
    drop((steering_tx, permission_tx));
    let reply = if current.trim().is_empty() {
        finished
    } else {
        current
    };
    let message = clean_reply(&reply);
    if message.is_empty() {
        return Err(anyhow!(t!("The agent didn't write a commit message.")));
    }
    Ok(message)
}

fn clean_reply(reply: &str) -> String {
    let trimmed = reply.trim();
    if trimmed.trim_matches('`').trim().is_empty() {
        return String::new();
    }
    let Some(inner) = trimmed
        .strip_prefix("```")
        .and_then(|rest| rest.strip_suffix("```"))
    else {
        return trimmed.to_owned();
    };
    let body = match inner.split_once('\n') {
        Some((info, body))
            if !info.trim().contains(char::is_whitespace) && !body.trim().is_empty() =>
        {
            body
        }
        _ => inner,
    };
    body.trim().to_owned()
}

pub fn commit_message_agent_label(cx: &App) -> Option<SharedString> {
    let settings = &AgentChatSettings::get_global(cx).commit_message;
    let kind = settings.agent?;
    Some(match &settings.model {
        Some(model) => format!("{} · {model}", kind.label()).into(),
        None => kind.label().into(),
    })
}

pub fn commit_message_menu(
    languages: Arc<LanguageRegistry>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<ContextMenu> {
    let store = AgentStore::global(languages, cx);
    let kinds = AgentChatSettings::get_global(cx).agents().to_vec();
    store.update(cx, |store, cx| {
        for kind in &kinds {
            store.ensure_models(*kind, cx);
        }
    });
    ContextMenu::build(window, cx, move |mut menu, _, cx| {
        let picked = AgentChatSettings::get_global(cx).commit_message.clone();
        menu = menu.toggleable_entry(
            t!("Automatic"),
            picked.agent.is_none(),
            IconPosition::Start,
            None,
            |_, cx| pick(None, None, cx),
        );
        for kind in &kinds {
            let kind = *kind;
            let store = store.clone();
            let picked = picked.clone();
            menu = menu.submenu(kind.label(), move |mut menu, _, cx| {
                let this_agent = picked.agent == Some(kind);
                menu = menu.toggleable_entry(
                    t!("Default Model"),
                    this_agent && picked.model.is_none(),
                    IconPosition::Start,
                    None,
                    move |_, cx| pick(Some(kind), None, cx),
                );
                let models = store.read(cx).models(kind).to_vec();
                if models.is_empty() {
                    menu = menu.label(t!("Loading models…"));
                }
                for model in models {
                    let selected = this_agent && picked.model.as_deref() == Some(model.id.as_str());
                    let id = model.id.clone();
                    menu = menu.toggleable_entry(
                        model.label.clone(),
                        selected,
                        IconPosition::Start,
                        None,
                        move |_, cx| pick(Some(kind), Some(id.clone()), cx),
                    );
                }
                menu
            });
        }
        menu
    })
}

fn pick(kind: Option<AgentKind>, model: Option<String>, cx: &mut App) {
    settings::update_settings_file(<dyn fs::Fs>::global(cx), cx, move |content, _| {
        let commit_message = content
            .agent_chat
            .get_or_insert_default()
            .commit_message
            .get_or_insert_default();
        commit_message.agent = kind.map(AgentKind::to_setting);
        commit_message.model = model;
    });
}

#[cfg(test)]
mod tests {
    use super::clean_reply;

    #[test]
    fn replies_lose_code_fences_and_padding() {
        assert_eq!(clean_reply("  Fix crash\n"), "Fix crash");
        assert_eq!(
            clean_reply("```text\nFix crash\n\nBody line\n```"),
            "Fix crash\n\nBody line"
        );
        assert_eq!(clean_reply("```\nFix crash\n```"), "Fix crash");
        assert_eq!(clean_reply("```Fix crash```"), "Fix crash");
        assert_eq!(clean_reply("```Refactor\n```"), "Refactor");
        assert_eq!(clean_reply("```"), "");
    }
}
