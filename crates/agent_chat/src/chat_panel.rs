use crate::{
    AgentChatSettings, AgentKind, MoveChatToPanel, MoveChatToTab, ToggleChatPanel,
    agent_panel::{agent_chat_enabled, create_session, store_for},
    chat_view::ChatView,
    session::{AgentSession, AgentStore},
};
use anyhow::Result;
use db::kvp::KeyValueStore;
use gpui::{
    Action, App, AsyncWindowContext, Context, Entity, EventEmitter, ExternalPaths, FocusHandle,
    Focusable, Global, IntoElement, ParentElement, Pixels, Render, Styled, WeakEntity, Window, div,
    px,
};
use i18n::{n, t};
use project::Project;
use serde::{Deserialize, Serialize};
use settings::{DockSide, Settings as _};
use std::sync::Arc;
use ui::{ContextMenu, IconButton, IconName, PopoverMenu, Tooltip, prelude::*};
use util::ResultExt as _;
use workspace::{
    DraggedSelection, ItemHandle, Pane, PaneSearchBarCallbacks, Workspace, WorkspaceId,
    dock::{DockPosition, Panel, PanelEvent},
    pane::{self, DraggedTab},
};

const CHAT_PANEL_KEY: &str = "AgentChatPanel";
const OPENS_IN_PANEL_KEY: &str = "agent_chat_opens_in_panel";

struct OpensInPanel(bool);

impl Global for OpensInPanel {}

fn opens_in_panel(cx: &App) -> bool {
    cx.try_global::<OpensInPanel>()
        .is_some_and(|opens_in_panel| opens_in_panel.0)
}

fn remember_where_chats_open(in_panel: bool, cx: &mut App) {
    if opens_in_panel(cx) == in_panel {
        return;
    }
    cx.set_global(OpensInPanel(in_panel));
    let db = KeyValueStore::global(cx);
    db::write_and_log(cx, move || async move {
        if in_panel {
            db.write_kvp(OPENS_IN_PANEL_KEY.into(), "1".into()).await
        } else {
            db.delete_kvp(OPENS_IN_PANEL_KEY.into()).await
        }
    });
}

pub(crate) fn visible_chat(workspace: &Workspace, cx: &App) -> Option<Entity<ChatView>> {
    if !panel_is_visible(workspace, cx) {
        return None;
    }
    workspace.panel::<ChatPanel>(cx)?.read(cx).active_chat(cx)
}

pub(crate) fn docked_chat_for(
    workspace: &Workspace,
    session: &Entity<AgentSession>,
    cx: &App,
) -> Option<Entity<ChatView>> {
    let panel = workspace.panel::<ChatPanel>(cx)?;
    chat_in_pane(&panel.read(cx).pane, session, cx)
}

fn chat_in_pane(
    pane: &Entity<Pane>,
    session: &Entity<AgentSession>,
    cx: &App,
) -> Option<Entity<ChatView>> {
    pane.read(cx)
        .items_of_type::<ChatView>()
        .find(|view| view.read(cx).session() == session)
}

pub(crate) fn should_open_in_panel(workspace: &Workspace, cx: &App) -> bool {
    opens_in_panel(cx) && workspace.panel::<ChatPanel>(cx).is_some()
}

pub fn init(cx: &mut App) {
    let saved = KeyValueStore::global(cx)
        .read_kvp(OPENS_IN_PANEL_KEY)
        .log_err()
        .flatten()
        .is_some();
    cx.set_global(OpensInPanel(saved));
    cx.observe_new(|workspace: &mut Workspace, _, _| {
        workspace
            .register_action(|workspace, _: &ToggleChatPanel, window, cx| {
                let has_chats = workspace
                    .panel::<ChatPanel>(cx)
                    .is_some_and(|panel| panel.read(cx).has_chats(cx));
                if agent_chat_enabled(cx) && (has_chats || panel_is_visible(workspace, cx)) {
                    workspace.toggle_panel_focus::<ChatPanel>(window, cx);
                }
            })
            .register_action(|workspace, _: &MoveChatToPanel, window, cx| {
                let active_chat = workspace
                    .active_item(cx)
                    .and_then(|item| item.downcast::<ChatView>())
                    .filter(|view| !view.read(cx).is_subagent());
                if let Some(view) = active_chat {
                    move_chat_to_panel(workspace, view, window, cx);
                }
            })
            .register_action(|workspace, _: &MoveChatToTab, window, cx| {
                let active_chat = workspace
                    .panel::<ChatPanel>(cx)
                    .and_then(|panel| panel.read(cx).active_chat(cx));
                if let Some(view) = active_chat {
                    move_chat_to_tab(workspace, view, window, cx);
                }
            });
    })
    .detach();
}

fn panel_is_visible(workspace: &Workspace, cx: &App) -> bool {
    workspace.all_docks().iter().any(|dock| {
        dock.read(cx)
            .visible_panel()
            .is_some_and(|panel| panel.panel_key() == CHAT_PANEL_KEY)
    })
}

pub(crate) fn show_chat_in_panel(
    workspace: &mut Workspace,
    session: Entity<AgentSession>,
    store: Entity<AgentStore>,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let view = match crate::agent_panel::chat_view_for(workspace, &session, cx) {
        Some(view) => view,
        None => {
            let project = workspace.project().clone();
            let weak_workspace = cx.entity().downgrade();
            cx.new(|cx| ChatView::new(session, store, project, weak_workspace, window, cx))
        }
    };
    move_chat_to_panel(workspace, view, window, cx);
}

pub(crate) fn move_chat_to_panel(
    workspace: &mut Workspace,
    view: Entity<ChatView>,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    if !agent_chat_enabled(cx) || view.read(cx).is_subagent() {
        return;
    }
    let Some(panel) = workspace.panel::<ChatPanel>(cx) else {
        return;
    };
    remember_where_chats_open(true, cx);
    for pane in workspace.panes().to_vec() {
        pane.update(cx, |pane, cx| {
            if pane.index_for_item(&view).is_some() {
                pane.remove_item(view.entity_id(), false, true, window, cx);
            }
        });
    }
    let pane = panel.read(cx).pane.clone();
    pane.update(cx, |pane, cx| {
        pane.add_item(Box::new(view), true, false, None, window, cx)
    });
    cx.defer_in(window, |workspace, window, cx| {
        workspace.focus_panel::<ChatPanel>(window, cx);
    });
}

pub(crate) fn reveal_chat(
    workspace: &mut Workspace,
    view: &Entity<ChatView>,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let Some(panel) = workspace.panel::<ChatPanel>(cx) else {
        return;
    };
    panel.read(cx).pane.clone().update(cx, |pane, cx| {
        if let Some(index) = pane.index_for_item(view) {
            pane.activate_item(index, true, false, window, cx);
        }
    });
    cx.defer_in(window, |workspace, window, cx| {
        workspace.focus_panel::<ChatPanel>(window, cx);
    });
}

pub(crate) fn move_chat_to_tab(
    workspace: &mut Workspace,
    view: Entity<ChatView>,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let Some(panel) = workspace.panel::<ChatPanel>(cx) else {
        return;
    };
    let pane = panel.read(cx).pane.clone();
    if pane.read(cx).index_for_item(&view).is_none() {
        return;
    }
    remember_where_chats_open(false, cx);
    pane.update(cx, |pane, cx| {
        pane.remove_item(view.entity_id(), false, false, window, cx)
    });
    view.update(cx, |view, cx| view.set_docked(false, window, cx));
    workspace.add_item_to_active_pane(Box::new(view), None, true, window, cx);
}

fn new_chat_in_panel(
    workspace: &mut Workspace,
    kind: AgentKind,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let Some(session) = create_session(workspace, kind, cx) else {
        return;
    };
    let store = store_for(workspace, cx);
    store.update(cx, |store, cx| store.update_agent_if_due(kind, cx));
    let project = workspace.project().clone();
    let weak_workspace = cx.entity().downgrade();
    let view = cx.new(|cx| ChatView::new(session, store, project, weak_workspace, window, cx));
    move_chat_to_panel(workspace, view, window, cx);
}

fn saved_chats_key(workspace_id: WorkspaceId) -> String {
    format!("agent_chat_panel_{}", i64::from(workspace_id))
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
struct SavedChats {
    chats: Vec<String>,
    active: usize,
}

impl SavedChats {
    fn parse(value: &str) -> Self {
        serde_json::from_str(value).unwrap_or_else(|_| Self {
            chats: vec![value.to_string()],
            active: 0,
        })
    }
}

fn new_chat_menu(workspace: WeakEntity<Workspace>) -> impl IntoElement {
    PopoverMenu::new("chat-panel-new-chat")
        .trigger_with_tooltip(
            IconButton::new("chat-panel-new-chat-trigger", IconName::Plus)
                .icon_size(IconSize::Small),
            Tooltip::text(t!("New Chat")),
        )
        .anchor(gpui::Anchor::TopLeft)
        .menu(move |window, cx| {
            let workspace = workspace.clone();
            let agents = AgentChatSettings::get_global(cx).agents().to_vec();
            Some(ContextMenu::build(window, cx, move |menu, _, _| {
                agents.into_iter().fold(menu, |menu, kind| {
                    let workspace = workspace.clone();
                    menu.entry(kind.label(), None, move |window, cx| {
                        workspace
                            .update(cx, |workspace, cx| {
                                new_chat_in_panel(workspace, kind, window, cx)
                            })
                            .log_err();
                    })
                })
            }))
        })
}

fn tab_bar_buttons(workspace: WeakEntity<Workspace>) -> AnyElement {
    let open_in_tab = IconButton::new("chat-panel-open-in-tab", IconName::Maximize)
        .icon_size(IconSize::Small)
        .tooltip(Tooltip::text(t!("Open in Tab")))
        .on_click({
            let workspace = workspace.clone();
            move |_, window, cx| {
                let workspace = workspace.clone();
                window.defer(cx, move |window, cx| {
                    workspace
                        .update(cx, |workspace, cx| {
                            let active_chat = workspace
                                .panel::<ChatPanel>(cx)
                                .and_then(|panel| panel.read(cx).active_chat(cx));
                            if let Some(view) = active_chat {
                                move_chat_to_tab(workspace, view, window, cx);
                            }
                        })
                        .log_err();
                });
            }
        });
    let close = IconButton::new("chat-panel-close", IconName::Close)
        .icon_size(IconSize::Small)
        .tooltip(Tooltip::text(t!("Close")))
        .on_click(move |_, window, cx| {
            workspace
                .update(cx, |workspace, cx| {
                    workspace.close_panel::<ChatPanel>(window, cx)
                })
                .log_err();
        });
    h_flex()
        .gap_1()
        .child(open_in_tab)
        .child(close)
        .into_any_element()
}

fn is_panel_chat(item: &dyn ItemHandle, cx: &App) -> bool {
    item.downcast::<ChatView>()
        .is_some_and(|view| !view.read(cx).is_subagent())
}

fn can_drop_in_panel(dragged: &dyn std::any::Any, _: &mut Window, cx: &mut App) -> bool {
    if let Some(tab) = dragged.downcast_ref::<DraggedTab>() {
        return is_panel_chat(tab.item.as_ref(), cx);
    }
    dragged.is::<ExternalPaths>() || dragged.is::<DraggedSelection>()
}

pub struct ChatPanel {
    workspace: WeakEntity<Workspace>,
    pane: Entity<Pane>,
    restoring: bool,
}

impl ChatPanel {
    pub async fn load(
        workspace: WeakEntity<Workspace>,
        mut cx: AsyncWindowContext,
    ) -> Result<Entity<Self>> {
        let (panel, saved, store) = workspace.update_in(&mut cx, |workspace, window, cx| {
            let saved = workspace
                .database_id()
                .filter(|_| agent_chat_enabled(cx))
                .and_then(|id| {
                    KeyValueStore::global(cx)
                        .read_kvp(&saved_chats_key(id))
                        .log_err()
                        .flatten()
                })
                .map(|value| SavedChats::parse(&value));
            let weak_workspace = cx.entity().downgrade();
            let project = workspace.project().clone();
            let panel = cx.new(|cx| Self::new(weak_workspace, project, window, cx));
            (panel, saved, store_for(workspace, cx))
        })?;
        if let Some(saved) = saved {
            panel.update_in(&mut cx, |panel, window, cx| {
                panel.restore(saved, store, window, cx)
            })?;
        }
        Ok(panel)
    }

    fn new(
        workspace: WeakEntity<Workspace>,
        project: Entity<Project>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let pane = cx.new(|cx| {
            let mut pane = Pane::new(
                workspace.clone(),
                project.clone(),
                Default::default(),
                Some(Arc::new(can_drop_in_panel)),
                Box::new(gpui::NoAction),
                false,
                window,
                cx,
            );
            pane.set_can_navigate(false, cx);
            pane.display_nav_history_buttons(None);
            pane.set_should_display_tab_bar(|_, _| true);
            pane.set_can_split(Some(Arc::new(|_, _, _, _| false)));
            pane.set_can_toggle_zoom(false, cx);
            pane.set_close_pane_if_empty(false, cx);
            pane.set_zoom_out_on_close(false);
            pane.set_render_trailing_tab_button(cx, {
                let workspace = workspace.clone();
                move |_, _, _| Some(new_chat_menu(workspace.clone()).into_any_element())
            });
            pane.set_render_tab_bar_buttons(cx, {
                let workspace = workspace.clone();
                move |_, _, _| (None, Some(tab_bar_buttons(workspace.clone())))
            });
            if let Some(callbacks) = cx.try_global::<PaneSearchBarCallbacks>() {
                let languages = Some(project.read(cx).languages().clone());
                (callbacks.setup_search_bar)(languages, pane.toolbar(), window, cx);
            }
            pane
        });
        cx.subscribe_in(&pane, window, Self::handle_pane_event)
            .detach();
        cx.observe(&pane, |_, _, cx| cx.notify()).detach();
        Self {
            workspace,
            pane,
            restoring: false,
        }
    }

    fn handle_pane_event(
        &mut self,
        pane: &Entity<Pane>,
        event: &pane::Event,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            pane::Event::AddItem { item } => {
                if !is_panel_chat(item.as_ref(), cx) {
                    self.move_to_center(item.boxed_clone(), window, cx);
                    return;
                }
                if let Some(view) = item.downcast::<ChatView>() {
                    view.update(cx, |view, cx| view.set_docked(true, window, cx));
                }
                if let Some(workspace) = self.workspace.upgrade() {
                    workspace.update(cx, |workspace, cx| {
                        item.added_to_pane(workspace, pane.clone(), window, cx)
                    });
                }
                self.save(cx);
            }
            pane::Event::RemovedItem { item } => {
                if let Some(view) = item.downcast::<ChatView>() {
                    view.update(cx, |view, cx| view.set_docked(false, window, cx));
                }
                self.save(cx);
                if !self.has_chats(cx) {
                    cx.emit(PanelEvent::Close);
                }
            }
            pane::Event::ActivateItem { .. } => self.save(cx),
            _ => {}
        }
    }

    fn move_to_center(
        &self,
        item: Box<dyn ItemHandle>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pane = self.pane.clone();
        let workspace = self.workspace.clone();
        window.defer(cx, move |window, cx| {
            pane.update(cx, |pane, cx| {
                pane.remove_item(item.item_id(), false, false, window, cx)
            });
            workspace
                .update(cx, |workspace, cx| {
                    workspace.add_item_to_active_pane(item, None, true, window, cx)
                })
                .log_err();
        });
    }

    fn restore(
        &mut self,
        saved: SavedChats,
        store: Entity<AgentStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let openings: Vec<_> = saved
            .chats
            .iter()
            .map(|id| {
                store.update(cx, |store, cx| {
                    store.open_saved_session(id.clone().into(), cx)
                })
            })
            .collect();
        self.restoring = true;
        cx.spawn_in(window, async move |this, cx| {
            let mut sessions = Vec::with_capacity(openings.len());
            for opening in openings {
                sessions.push(opening.await.log_err());
            }
            this.update_in(cx, |this, window, cx| {
                this.restoring = false;
                let workspace = this.workspace.clone();
                let mut active = None;
                if let Some(center) = workspace.upgrade().filter(|_| agent_chat_enabled(cx)) {
                    let project = center.read(cx).project().clone();
                    for (index, session) in sessions.into_iter().enumerate() {
                        let Some(session) = session else {
                            continue;
                        };
                        let shown_in_tab =
                            center.read(cx).items_of_type::<ChatView>(cx).any(|view| {
                                let view = view.read(cx);
                                view.session() == &session && !view.is_subagent()
                            });
                        if shown_in_tab || chat_in_pane(&this.pane, &session, cx).is_some() {
                            continue;
                        }
                        let kind = session.read(cx).kind();
                        store.update(cx, |store, cx| store.update_agent_if_due(kind, cx));
                        let view = cx.new(|cx| {
                            ChatView::new(
                                session,
                                store.clone(),
                                project.clone(),
                                workspace.clone(),
                                window,
                                cx,
                            )
                        });
                        view.update(cx, |view, cx| view.set_docked(true, window, cx));
                        this.pane.update(cx, |pane, cx| {
                            pane.add_item(Box::new(view), false, false, None, window, cx);
                            if index == saved.active {
                                active = Some(pane.items_len() - 1);
                            }
                        });
                    }
                }
                if let Some(active) = active {
                    this.pane.update(cx, |pane, cx| {
                        pane.activate_item(active, false, false, window, cx)
                    });
                }
                this.save(cx);
                if !this.has_chats(cx) {
                    cx.emit(PanelEvent::Close);
                }
            })
            .log_err();
        })
        .detach();
    }

    pub(crate) fn pane(&self) -> &Entity<Pane> {
        &self.pane
    }

    pub(crate) fn has_chats(&self, cx: &App) -> bool {
        self.pane.read(cx).items_len() > 0
    }

    pub(crate) fn active_chat(&self, cx: &App) -> Option<Entity<ChatView>> {
        self.pane.read(cx).active_item()?.downcast::<ChatView>()
    }

    fn saved_chats(&self, cx: &App) -> SavedChats {
        let pane = self.pane.read(cx);
        SavedChats {
            chats: pane
                .items_of_type::<ChatView>()
                .map(|view| view.read(cx).session().read(cx).metadata().id.clone())
                .collect(),
            active: pane.active_item_index(),
        }
    }

    fn save(&self, cx: &App) {
        if self.restoring {
            return;
        }
        let Some(workspace_id) = self
            .workspace
            .upgrade()
            .and_then(|workspace| workspace.read(cx).database_id())
        else {
            return;
        };
        let key = saved_chats_key(workspace_id);
        let saved = self.saved_chats(cx);
        let value = (!saved.chats.is_empty())
            .then(|| serde_json::to_string(&saved).log_err())
            .flatten();
        let db = KeyValueStore::global(cx);
        db::write_and_log(cx, move || async move {
            match value {
                Some(value) => db.write_kvp(key, value).await,
                None => db.delete_kvp(key).await,
            }
        });
    }
}

impl EventEmitter<PanelEvent> for ChatPanel {}

impl Focusable for ChatPanel {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.pane.focus_handle(cx)
    }
}

impl Render for ChatPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = cx
            .try_global::<PaneSearchBarCallbacks>()
            .map(|callbacks| (callbacks.wrap_div_with_search_actions)(div(), self.pane.clone()))
            .unwrap_or_else(div);
        root.key_context("ChatPanel")
            .size_full()
            .bg(cx.theme().colors().editor_background)
            .child(self.pane.clone())
    }
}

impl Panel for ChatPanel {
    fn persistent_name() -> &'static str {
        "Chat Panel"
    }

    fn panel_key() -> &'static str {
        CHAT_PANEL_KEY
    }

    fn position(&self, _: &Window, cx: &App) -> DockPosition {
        match AgentChatSettings::get_global(cx).chat_panel_dock {
            DockSide::Left => DockPosition::Left,
            DockSide::Right => DockPosition::Right,
        }
    }

    fn position_is_valid(&self, position: DockPosition) -> bool {
        matches!(position, DockPosition::Left | DockPosition::Right)
    }

    fn set_position(&mut self, position: DockPosition, _: &mut Window, cx: &mut Context<Self>) {
        let dock = match position {
            DockPosition::Left => DockSide::Left,
            DockPosition::Right | DockPosition::Bottom => DockSide::Right,
        };
        settings::update_settings_file(<dyn fs::Fs>::global(cx), cx, move |content, _| {
            content.agent_chat.get_or_insert_default().chat_panel_dock = Some(dock);
        });
    }

    fn default_size(&self, _: &Window, _: &App) -> Pixels {
        px(420.)
    }

    fn min_size(&self, _: &Window, _: &App) -> Option<Pixels> {
        Some(px(300.))
    }

    fn icon(&self, _: &Window, _: &App) -> Option<IconName> {
        None
    }

    fn icon_tooltip(&self, _: &Window, _: &App) -> Option<&'static str> {
        Some(n!("Chat Panel"))
    }

    fn toggle_action(&self) -> Box<dyn Action> {
        Box::new(ToggleChatPanel)
    }

    fn activation_priority(&self) -> u32 {
        9
    }

    fn pane(&self) -> Option<Entity<Pane>> {
        Some(self.pane.clone())
    }

    fn enabled(&self, cx: &App) -> bool {
        agent_chat_enabled(cx) && (self.has_chats(cx) || self.restoring)
    }

    fn set_active(&mut self, active: bool, _: &mut Window, cx: &mut Context<Self>) {
        let empty = !self.has_chats(cx) && !self.restoring;
        if active && (empty || !agent_chat_enabled(cx)) {
            cx.emit(PanelEvent::Close);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{agent_panel::chat_view_for, session::tests::new_store};
    use gpui::{TestAppContext, VisualTestContext};
    use project::FakeFs;

    struct Setup {
        workspace: Entity<Workspace>,
        panel: Entity<ChatPanel>,
        store: Entity<AgentStore>,
        project: Entity<Project>,
        directory: tempfile::TempDir,
    }

    async fn setup(cx: &mut TestAppContext) -> (Setup, &mut VisualTestContext) {
        cx.update(|cx| {
            workspace::AppState::test(cx);
            editor::init(cx);
            init(cx);
        });
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = new_store(directory.path(), cx);
        store.update(cx, |store, _| store.skip_plan_usage());
        let project = Project::test(FakeFs::new(cx.executor()), [], cx).await;
        let (multi_workspace, cx) = cx.add_window_view(|window, cx| {
            workspace::MultiWorkspace::test_new(project.clone(), window, cx)
        });
        let workspace = multi_workspace.read_with(cx, |multi, _| multi.workspace().clone());
        let panel = workspace.update_in(cx, |workspace, window, cx| {
            let weak_workspace = cx.entity().downgrade();
            let project = workspace.project().clone();
            let panel = cx.new(|cx| ChatPanel::new(weak_workspace, project, window, cx));
            workspace.add_panel(panel.clone(), window, cx);
            panel
        });
        (
            Setup {
                workspace,
                panel,
                store,
                project,
                directory,
            },
            cx,
        )
    }

    fn new_session(setup: &Setup, cx: &mut VisualTestContext) -> Entity<AgentSession> {
        setup.store.update(cx, |store, cx| {
            store.create_session(AgentKind::Claude, setup.directory.path().to_path_buf(), cx)
        })
    }

    fn open_tab(setup: &Setup, cx: &mut VisualTestContext) -> Entity<ChatView> {
        let session = new_session(setup, cx);
        setup.workspace.update_in(cx, |workspace, window, cx| {
            let weak_workspace = cx.entity().downgrade();
            let (store, project) = (setup.store.clone(), setup.project.clone());
            let view =
                cx.new(|cx| ChatView::new(session, store, project, weak_workspace, window, cx));
            workspace.add_item_to_active_pane(Box::new(view.clone()), None, true, window, cx);
            view
        })
    }

    fn dock(setup: &Setup, view: &Entity<ChatView>, cx: &mut VisualTestContext) {
        setup.workspace.update_in(cx, |workspace, window, cx| {
            move_chat_to_panel(workspace, view.clone(), window, cx)
        });
        cx.run_until_parked();
    }

    fn undock(setup: &Setup, view: &Entity<ChatView>, cx: &mut VisualTestContext) {
        setup.workspace.update_in(cx, |workspace, window, cx| {
            move_chat_to_tab(workspace, view.clone(), window, cx)
        });
        cx.run_until_parked();
    }

    fn tabs(setup: &Setup, cx: &mut VisualTestContext) -> Vec<Entity<ChatView>> {
        setup.workspace.read_with(cx, |workspace, cx| {
            workspace.items_of_type::<ChatView>(cx).collect()
        })
    }

    fn docked(setup: &Setup, cx: &mut VisualTestContext) -> Vec<Entity<ChatView>> {
        setup.panel.read_with(cx, |panel, cx| {
            panel.pane.read(cx).items_of_type::<ChatView>().collect()
        })
    }

    fn active(setup: &Setup, cx: &mut VisualTestContext) -> Option<Entity<ChatView>> {
        setup.panel.read_with(cx, |panel, cx| panel.active_chat(cx))
    }

    fn is_docked(view: &Entity<ChatView>, cx: &mut VisualTestContext) -> bool {
        view.read_with(cx, |view, _| view.is_docked())
    }

    fn panel_open(setup: &Setup, cx: &mut VisualTestContext) -> bool {
        setup
            .workspace
            .read_with(cx, |workspace, cx| panel_is_visible(workspace, cx))
    }

    #[gpui::test]
    async fn chats_move_between_tabs_and_panel_tabs(cx: &mut TestAppContext) {
        let (setup, cx) = setup(cx).await;
        let first = open_tab(&setup, cx);
        let second = open_tab(&setup, cx);
        dock(&setup, &first, cx);
        dock(&setup, &second, cx);
        assert!(tabs(&setup, cx).is_empty());
        assert_eq!(docked(&setup, cx), vec![first.clone(), second.clone()]);
        assert_eq!(active(&setup, cx), Some(second.clone()));
        assert!(is_docked(&first, cx) && is_docked(&second, cx));
        assert!(panel_open(&setup, cx));
        let found = setup.workspace.read_with(cx, |workspace, cx| {
            chat_view_for(workspace, first.read(cx).session(), cx)
        });
        assert_eq!(found, Some(first.clone()));

        undock(&setup, &first, cx);
        assert_eq!(tabs(&setup, cx), vec![first.clone()]);
        assert_eq!(docked(&setup, cx), vec![second.clone()]);
        assert!(!is_docked(&first, cx));
        assert!(panel_open(&setup, cx));

        undock(&setup, &second, cx);
        assert!(docked(&setup, cx).is_empty());
        assert_eq!(tabs(&setup, cx).len(), 2);
        assert!(!panel_open(&setup, cx));
    }

    #[gpui::test]
    async fn the_chat_moves_itself_both_ways(cx: &mut TestAppContext) {
        let (setup, cx) = setup(cx).await;
        let chat = open_tab(&setup, cx);
        chat.update_in(cx, |chat, window, cx| {
            chat.move_to_panel(&MoveChatToPanel, window, cx)
        });
        cx.run_until_parked();
        assert!(tabs(&setup, cx).is_empty());
        assert_eq!(docked(&setup, cx), vec![chat.clone()]);

        chat.update_in(cx, |chat, window, cx| {
            chat.move_to_tab(&MoveChatToTab, window, cx)
        });
        cx.run_until_parked();
        assert_eq!(tabs(&setup, cx), vec![chat.clone()]);
        assert!(docked(&setup, cx).is_empty());
    }

    #[gpui::test]
    async fn the_docked_flag_follows_the_pane_a_chat_is_dragged_into(cx: &mut TestAppContext) {
        let (setup, cx) = setup(cx).await;
        let chat = open_tab(&setup, cx);
        let center = setup
            .workspace
            .read_with(cx, |workspace, _| workspace.active_pane().clone());
        let panel_pane = setup.panel.read_with(cx, |panel, _| panel.pane.clone());
        cx.update(|window, cx| {
            workspace::move_item(&center, &panel_pane, chat.entity_id(), 0, true, window, cx)
        });
        cx.run_until_parked();
        assert!(is_docked(&chat, cx));
        assert_eq!(docked(&setup, cx), vec![chat.clone()]);

        cx.update(|window, cx| {
            workspace::move_item(&panel_pane, &center, chat.entity_id(), 0, true, window, cx)
        });
        cx.run_until_parked();
        assert!(!is_docked(&chat, cx));
        assert_eq!(tabs(&setup, cx), vec![chat]);
        assert!(!panel_open(&setup, cx));
    }

    #[gpui::test]
    async fn deleting_a_docked_chat_closes_only_its_tab(cx: &mut TestAppContext) {
        let (setup, cx) = setup(cx).await;
        let kept = open_tab(&setup, cx);
        let deleted = open_tab(&setup, cx);
        dock(&setup, &kept, cx);
        dock(&setup, &deleted, cx);
        let id = deleted.read_with(cx, |chat, cx| chat.session().read(cx).metadata().id.clone());
        setup
            .store
            .update(cx, |store, cx| store.delete_session(&id, cx));
        cx.run_until_parked();
        assert_eq!(docked(&setup, cx), vec![kept]);
        assert!(tabs(&setup, cx).is_empty());
    }

    #[gpui::test]
    async fn closing_agent_chats_empties_the_panel(cx: &mut TestAppContext) {
        let (setup, cx) = setup(cx).await;
        let first = open_tab(&setup, cx);
        let second = open_tab(&setup, cx);
        dock(&setup, &first, cx);
        dock(&setup, &second, cx);
        cx.update(|_, cx| cx.defer(crate::close_agent_chats));
        cx.run_until_parked();
        assert!(docked(&setup, cx).is_empty());
        assert!(tabs(&setup, cx).is_empty());
        assert!(!panel_open(&setup, cx));
    }

    #[gpui::test]
    async fn picking_a_chat_opens_it_as_a_panel_tab(cx: &mut TestAppContext) {
        let (setup, cx) = setup(cx).await;
        let first = open_tab(&setup, cx);
        dock(&setup, &first, cx);

        let unopened = new_session(&setup, cx);
        setup.workspace.update_in(cx, |workspace, window, cx| {
            show_chat_in_panel(workspace, unopened.clone(), setup.store.clone(), window, cx)
        });
        cx.run_until_parked();
        let sessions: Vec<_> = docked(&setup, cx)
            .iter()
            .map(|view| view.read_with(cx, |view, _| view.session().clone()))
            .collect();
        let first_session = first.read_with(cx, |view, _| view.session().clone());
        assert_eq!(sessions, vec![first_session.clone(), unopened.clone()]);

        setup.workspace.update_in(cx, |workspace, window, cx| {
            show_chat_in_panel(workspace, first_session, setup.store.clone(), window, cx)
        });
        cx.run_until_parked();
        assert_eq!(docked(&setup, cx).len(), 2);
        assert_eq!(active(&setup, cx), Some(first.clone()));

        let tab = open_tab(&setup, cx);
        let tab_session = tab.read_with(cx, |view, _| view.session().clone());
        setup.workspace.update_in(cx, |workspace, window, cx| {
            show_chat_in_panel(workspace, tab_session, setup.store.clone(), window, cx)
        });
        cx.run_until_parked();
        assert_eq!(docked(&setup, cx).len(), 3);
        assert_eq!(active(&setup, cx), Some(tab));
        assert!(tabs(&setup, cx).is_empty());
    }

    #[gpui::test]
    async fn closing_the_last_panel_tab_closes_the_panel(cx: &mut TestAppContext) {
        let (setup, cx) = setup(cx).await;
        let chat = open_tab(&setup, cx);
        dock(&setup, &chat, cx);
        let panel_pane = setup.panel.read_with(cx, |panel, _| panel.pane.clone());
        panel_pane
            .update_in(cx, |pane, window, cx| {
                pane.close_item_by_id(chat.entity_id(), workspace::SaveIntent::Skip, window, cx)
            })
            .await
            .expect("chat closes");
        cx.run_until_parked();
        assert!(docked(&setup, cx).is_empty());
        assert!(!panel_open(&setup, cx));
        assert!(
            !setup
                .panel
                .read_with(cx, |panel, cx| Panel::enabled(panel, cx))
        );
    }

    fn toggle_panel(setup: &Setup, cx: &mut VisualTestContext) {
        setup.workspace.update_in(cx, |workspace, window, cx| {
            window.focus(&workspace.active_pane().focus_handle(cx), cx)
        });
        cx.dispatch_action(ToggleChatPanel);
    }

    #[gpui::test]
    async fn the_panel_never_opens_empty(cx: &mut TestAppContext) {
        let (setup, cx) = setup(cx).await;
        toggle_panel(&setup, cx);
        cx.run_until_parked();
        assert!(!panel_open(&setup, cx));

        let chat = open_tab(&setup, cx);
        dock(&setup, &chat, cx);
        assert!(panel_open(&setup, cx));
        setup.workspace.update_in(cx, |workspace, window, cx| {
            workspace.close_panel::<ChatPanel>(window, cx)
        });
        cx.run_until_parked();
        assert!(!panel_open(&setup, cx));
        toggle_panel(&setup, cx);
        cx.run_until_parked();
        assert!(panel_open(&setup, cx));

        undock(&setup, &chat, cx);
        assert!(!panel_open(&setup, cx));
        toggle_panel(&setup, cx);
        cx.run_until_parked();
        assert!(!panel_open(&setup, cx));
    }

    #[gpui::test]
    async fn chats_open_where_the_last_one_was(cx: &mut TestAppContext) {
        let (setup, cx) = setup(cx).await;
        let opens_in_panel = |cx: &mut VisualTestContext| {
            setup
                .workspace
                .read_with(cx, |workspace, cx| should_open_in_panel(workspace, cx))
        };
        assert!(!opens_in_panel(cx));

        let chat = open_tab(&setup, cx);
        dock(&setup, &chat, cx);
        assert!(opens_in_panel(cx));

        setup.workspace.update_in(cx, |workspace, window, cx| {
            workspace.close_panel::<ChatPanel>(window, cx)
        });
        assert!(opens_in_panel(cx));

        undock(&setup, &chat, cx);
        assert!(!opens_in_panel(cx));
    }

    #[gpui::test]
    async fn moving_the_only_tab_of_a_split_keeps_the_other_chats(cx: &mut TestAppContext) {
        let (setup, cx) = setup(cx).await;
        let stays = open_tab(&setup, cx);
        let in_split = open_tab(&setup, cx);
        setup.workspace.update_in(cx, |workspace, window, cx| {
            workspace.split_item(
                workspace::SplitDirection::Right,
                Box::new(in_split.clone()),
                window,
                cx,
            )
        });
        cx.run_until_parked();
        dock(&setup, &in_split, cx);
        assert_eq!(docked(&setup, cx), vec![in_split]);
        assert_eq!(tabs(&setup, cx), vec![stays]);
    }

    #[gpui::test]
    async fn an_empty_panel_is_disabled_so_its_dock_moves_on(cx: &mut TestAppContext) {
        let (setup, cx) = setup(cx).await;
        let enabled = |cx: &mut VisualTestContext| {
            setup
                .panel
                .read_with(cx, |panel, cx| Panel::enabled(panel, cx))
        };
        assert!(!enabled(cx));
        let chat = open_tab(&setup, cx);
        dock(&setup, &chat, cx);
        assert!(enabled(cx));
        undock(&setup, &chat, cx);
        assert!(!enabled(cx));
        assert!(!panel_open(&setup, cx));
    }

    #[gpui::test]
    async fn the_panel_remembers_its_tabs_and_the_active_one(cx: &mut TestAppContext) {
        let (setup, cx) = setup(cx).await;
        let first = open_tab(&setup, cx);
        let second = open_tab(&setup, cx);
        dock(&setup, &first, cx);
        dock(&setup, &second, cx);
        setup.workspace.update_in(cx, |workspace, window, cx| {
            reveal_chat(workspace, &first, window, cx)
        });
        cx.run_until_parked();
        let id = |view: &Entity<ChatView>, cx: &mut VisualTestContext| {
            view.read_with(cx, |view, cx| view.session().read(cx).metadata().id.clone())
        };
        let expected = SavedChats {
            chats: vec![id(&first, cx), id(&second, cx)],
            active: 0,
        };
        let saved = setup.panel.read_with(cx, |panel, cx| panel.saved_chats(cx));
        assert_eq!(saved, expected);
        let encoded = serde_json::to_string(&saved).expect("saved chats encode");
        assert_eq!(SavedChats::parse(&encoded), expected);
    }

    #[gpui::test]
    async fn restoring_reopens_saved_tabs_except_ones_open_elsewhere(cx: &mut TestAppContext) {
        let (setup, cx) = setup(cx).await;
        let first = new_session(&setup, cx);
        let second = new_session(&setup, cx);
        let in_tab = open_tab(&setup, cx);
        let id = |session: &Entity<AgentSession>, cx: &mut VisualTestContext| {
            session.read_with(cx, |session, _| session.metadata().id.clone())
        };
        let in_tab_session = in_tab.read_with(cx, |view, _| view.session().clone());
        let saved = SavedChats {
            chats: vec![id(&first, cx), id(&in_tab_session, cx), id(&second, cx)],
            active: 2,
        };
        setup.panel.update_in(cx, |panel, window, cx| {
            panel.restore(saved, setup.store.clone(), window, cx)
        });
        cx.run_until_parked();
        let restored: Vec<_> = docked(&setup, cx)
            .iter()
            .map(|view| view.read_with(cx, |view, _| view.session().clone()))
            .collect();
        assert_eq!(restored, vec![first, second.clone()]);
        let active_session =
            active(&setup, cx).map(|view| view.read_with(cx, |view, _| view.session().clone()));
        assert_eq!(active_session, Some(second));
        assert!(docked(&setup, cx).iter().all(|view| is_docked(view, cx)));
        assert_eq!(tabs(&setup, cx), vec![in_tab]);
    }

    #[gpui::test]
    async fn only_chat_tabs_can_be_dropped_into_the_panel(cx: &mut TestAppContext) {
        let (setup, cx) = setup(cx).await;
        let chat = open_tab(&setup, cx);
        let center = setup
            .workspace
            .read_with(cx, |workspace, _| workspace.active_pane().clone());
        let other = cx.new(workspace::item::test::TestItem::new);
        let dragged = |item: Box<dyn workspace::ItemHandle>| DraggedTab {
            pane: center.clone(),
            item,
            ix: 0,
            detail: 0,
            is_active: true,
        };
        let chat_tab = dragged(Box::new(chat));
        let other_tab = dragged(Box::new(other));
        cx.update(|window, cx| {
            assert!(can_drop_in_panel(&chat_tab, window, cx));
            assert!(!can_drop_in_panel(&other_tab, window, cx));
            assert!(can_drop_in_panel(&ExternalPaths::default(), window, cx));
            let entry = project::ProjectEntryId::from_usize(1);
            assert!(!can_drop_in_panel(&entry, window, cx));
        });
    }

    #[gpui::test]
    async fn anything_but_a_chat_landing_in_the_panel_moves_to_the_center(cx: &mut TestAppContext) {
        let (setup, cx) = setup(cx).await;
        let chat = open_tab(&setup, cx);
        dock(&setup, &chat, cx);
        let file = cx.new(workspace::item::test::TestItem::new);
        let panel_pane = setup.panel.read_with(cx, |panel, _| panel.pane.clone());
        panel_pane.update_in(cx, |pane, window, cx| {
            pane.add_item(Box::new(file.clone()), true, true, None, window, cx)
        });
        cx.run_until_parked();
        assert_eq!(docked(&setup, cx), vec![chat]);
        let in_center = setup.workspace.read_with(cx, |workspace, cx| {
            workspace
                .active_pane()
                .read(cx)
                .index_for_item(&file)
                .is_some()
        });
        assert!(in_center);
        assert!(panel_open(&setup, cx));
    }

    #[test]
    fn a_single_saved_chat_from_before_tabs_still_restores() {
        assert_eq!(
            SavedChats::parse("chat-1"),
            SavedChats {
                chats: vec!["chat-1".into()],
                active: 0,
            }
        );
    }
}
