use super::{ChatView, icon};
use crate::chat_style::{Chip, format_elapsed, ink, popover_card, text_faint, ui};
use crate::session::AgentSession;
use crate::usage_rings::menu_heading;
use agent_harness::{BackgroundTaskKind, ToolCall};
use gpui::{
    Anchor, AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    IntoElement, ParentElement, Render, SharedString, Styled, Task, Window, div, point, px,
};
use i18n::t;
use std::time::Duration;
use theme::ActiveTheme as _;
use ui::{PopoverMenu, prelude::*};

const TICK: Duration = Duration::from_secs(1);
const CARD_MAX_WIDTH: f32 = 360.;

struct TaskRow {
    kind: BackgroundTaskKind,
    title: SharedString,
    command: Option<SharedString>,
    elapsed: SharedString,
}

fn running_label(kind: BackgroundTaskKind) -> SharedString {
    match kind {
        BackgroundTaskKind::Shell => t!("Command running"),
        BackgroundTaskKind::Agent => t!("Agent running"),
        BackgroundTaskKind::Other => t!("Task running"),
    }
}

fn untitled_task(kind: BackgroundTaskKind) -> SharedString {
    match kind {
        BackgroundTaskKind::Shell => t!("Background command"),
        BackgroundTaskKind::Agent => t!("Background agent"),
        BackgroundTaskKind::Other => t!("Background task"),
    }
}

impl ChatView {
    pub(super) fn ensure_background_tick(&mut self, cx: &mut Context<Self>) {
        if self.session.read(cx).background_tasks().is_empty() || self.background_ticking {
            return;
        }
        self.background_ticking = true;
        self._background_tick = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                let running = this
                    .update(cx, |this, cx| {
                        let running = !this.session.read(cx).background_tasks().is_empty();
                        if running {
                            cx.notify();
                        } else {
                            this.background_ticking = false;
                        }
                        running
                    })
                    .unwrap_or(false);
                if !running {
                    return;
                }
            }
        });
    }

    pub(super) fn render_background_indicator(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let tasks = self.session.read(cx).background_tasks();
        let oldest = tasks.iter().map(|task| task.started_at).min()?;
        let label: SharedString = match tasks {
            [task] => running_label(task.kind),
            _ => t!("{count} tasks running", count = tasks.len()),
        };
        let elapsed = format_elapsed(oldest.elapsed().as_secs());
        let colors = cx.theme().colors();
        let session = self.session.clone();
        let summary = h_flex()
            .debug_selector(|| "agent-background-tasks".into())
            .h(px(20.))
            .px(px(7.))
            .gap(px(5.))
            .text_size(ui(11.))
            .child(
                div()
                    .size(px(6.))
                    .rounded_full()
                    .bg(cx.theme().status().info),
            )
            .child(label)
            .child(div().text_color(text_faint(cx)).child(elapsed));
        Some(
            PopoverMenu::new(SharedString::from(format!(
                "agent-background-tasks-{}",
                cx.entity_id()
            )))
            .trigger(
                Chip::new("agent-background-tasks", 6., summary)
                    .hover_background(ink(0.06, cx))
                    .text_colors(colors.text_muted, colors.text),
            )
            .menu(move |_, cx| {
                let session = session.clone();
                Some(cx.new(|cx| BackgroundTasksCard::new(session, cx)))
            })
            .anchor(Anchor::BottomLeft)
            .attach(Anchor::TopLeft)
            .offset(point(px(0.), px(-6.)))
            .into_any_element(),
        )
    }
}

fn background_rows(session: &AgentSession) -> Vec<TaskRow> {
    session
        .background_tasks()
        .iter()
        .map(|task| {
            let command = task
                .tool_use_id
                .as_deref()
                .and_then(|id| session.tool_entry(id))
                .and_then(|tool| match &tool.call {
                    ToolCall::Exec { command } => Some(SharedString::from(command.clone())),
                    _ => None,
                });
            let title: SharedString = if !task.description.trim().is_empty() {
                task.description.clone().into()
            } else if let Some(command) = &command {
                command.clone()
            } else {
                untitled_task(task.kind)
            };
            TaskRow {
                kind: task.kind,
                command: command.filter(|command| *command != title),
                title,
                elapsed: format_elapsed(task.started_at.elapsed().as_secs()).into(),
            }
        })
        .collect()
}

struct BackgroundTasksCard {
    session: Entity<AgentSession>,
    focus_handle: FocusHandle,
    _tick: Task<()>,
}

impl BackgroundTasksCard {
    fn new(session: Entity<AgentSession>, cx: &mut Context<Self>) -> Self {
        let tick = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    return;
                }
            }
        });
        Self {
            session,
            focus_handle: cx.focus_handle(),
            _tick: tick,
        }
    }
}

impl EventEmitter<DismissEvent> for BackgroundTasksCard {}

impl Focusable for BackgroundTasksCard {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for BackgroundTasksCard {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = background_rows(self.session.read(cx));
        let muted = cx.theme().colors().text_muted;
        let faint = text_faint(cx);
        let code_font = super::code_font(cx);
        popover_card(cx)
            .key_context("menu")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|_, _: &menu::Cancel, _, cx| cx.emit(DismissEvent)))
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(DismissEvent)))
            .max_w(px(CARD_MAX_WIDTH))
            .child(menu_heading(&t!("Background tasks"), cx))
            .when(rows.is_empty(), |this| {
                this.child(
                    div()
                        .px(px(8.))
                        .pb(px(6.))
                        .text_size(ui(12.))
                        .text_color(muted)
                        .child(t!("Nothing is running in the background.")),
                )
            })
            .child(
                v_flex()
                    .px(px(8.))
                    .pb(px(6.))
                    .gap(px(6.))
                    .children(rows.into_iter().map(move |row| {
                        v_flex()
                            .gap(px(2.))
                            .text_size(ui(12.))
                            .child(
                                h_flex()
                                    .gap(px(8.))
                                    .child(
                                        icon(
                                            match row.kind {
                                                BackgroundTaskKind::Shell => {
                                                    IconName::AgentTerminal
                                                }
                                                BackgroundTaskKind::Agent => IconName::AgentBot,
                                                BackgroundTaskKind::Other => IconName::AgentWidget,
                                            },
                                            14.,
                                        )
                                        .text_color(muted),
                                    )
                                    .child(div().flex_1().min_w_0().truncate().child(row.title))
                                    .child(div().flex_none().text_color(faint).child(row.elapsed)),
                            )
                            .when_some(row.command, |this, command| {
                                this.child(
                                    div()
                                        .pl(px(22.))
                                        .text_size(ui(11.))
                                        .text_color(muted)
                                        .font(code_font.clone())
                                        .child(command),
                                )
                            })
                    })),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AgentKind,
        session::tests::{feed_events, new_store},
    };
    use agent_harness::AgentEvent;
    use gpui::{TestAppContext, WeakEntity};
    use project::FakeFs;

    #[gpui::test]
    async fn the_popup_lists_each_task_once(cx: &mut TestAppContext) {
        cx.update(|cx| {
            workspace::AppState::test(cx);
            editor::init(cx);
        });
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = new_store(directory.path(), cx);
        store.update(cx, |store, _| store.skip_plan_usage());
        let project = project::Project::test(FakeFs::new(cx.executor()), [], cx).await;
        let session = store.update(cx, |store, cx| {
            store.create_session(AgentKind::Claude, directory.path().to_path_buf(), cx)
        });
        feed_events(
            &session,
            vec![
                AgentEvent::ToolCall {
                    id: "toolu_bash".into(),
                    call: ToolCall::Exec {
                        command: "cargo test --workspace".into(),
                    },
                },
                AgentEvent::ToolCall {
                    id: "toolu_agent".into(),
                    call: ToolCall::Unknown {
                        name: "Agent: scan the repo".into(),
                        input: None,
                    },
                },
                AgentEvent::TaskStarted {
                    task_id: "bg1".into(),
                    tool_use_id: Some("toolu_bash".into()),
                    kind: BackgroundTaskKind::Shell,
                    description: String::new(),
                },
                AgentEvent::TaskStarted {
                    task_id: "a1".into(),
                    tool_use_id: Some("toolu_agent".into()),
                    kind: BackgroundTaskKind::Agent,
                    description: "Scan the repo".into(),
                },
                AgentEvent::BackgroundTasksChanged {
                    tasks: vec![
                        agent_harness::BackgroundTaskInfo {
                            task_id: "bg1".into(),
                            kind: BackgroundTaskKind::Shell,
                            description: String::new(),
                        },
                        agent_harness::BackgroundTaskInfo {
                            task_id: "a1".into(),
                            kind: BackgroundTaskKind::Agent,
                            description: String::new(),
                        },
                    ],
                },
            ],
            cx,
        );
        let (chat, cx) = cx.add_window_view(|window, cx| {
            ChatView::new(
                session.clone(),
                store.clone(),
                project,
                WeakEntity::new_invalid(),
                window,
                cx,
            )
        });
        chat.update(cx, |chat, cx| {
            assert!(chat.render_background_indicator(cx).is_some());
        });
        session.read_with(cx, |session, _| {
            let rows = background_rows(session);
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].title.as_ref(), "cargo test --workspace");
            assert!(
                rows[0].command.is_none(),
                "the command is the title already"
            );
            assert_eq!(rows[1].title.as_ref(), "Scan the repo");
            assert!(rows[1].command.is_none());
        });
        feed_events(
            &session,
            vec![AgentEvent::BackgroundTasksChanged { tasks: Vec::new() }],
            cx,
        );
        chat.update(cx, |chat, cx| {
            assert!(chat.render_background_indicator(cx).is_none())
        });
    }
}
