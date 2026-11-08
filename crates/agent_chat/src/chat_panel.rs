use crate::{
    AgentChatSettings, AgentKind, MoveChatToPanel, MoveChatToTab, ToggleChatPanel,
    agent_panel::{
        HEADER_CONTROL_SIZE, HeaderButton, agent_chat_enabled, chat_view_for, create_session,
        store_for,
    },
    chat_style::ui,
    chat_view::ChatView,
    model_picker::agent_icon,
    session::{AgentSession, AgentStore},
};
use anyhow::Result;
use db::kvp::KeyValueStore;
use gpui::{
    Action, App, AsyncWindowContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, Global, IntoElement, ParentElement, Pixels, Render, Styled, Subscription,
    WeakEntity, Window, div, px,
};
use i18n::{n, t};
use project::Project;
use settings::{DockSide, Settings as _};
use ui::{Clickable, ContextMenu, IconName, PopoverMenu, prelude::*};
use util::ResultExt as _;
use workspace::{
    Workspace, WorkspaceId,
    dock::{DockPosition, Panel, PanelEvent},
    item::ItemEvent,
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
                let has_chat = workspace
                    .panel::<ChatPanel>(cx)
                    .is_some_and(|panel| panel.read(cx).chat().is_some());
                if agent_chat_enabled(cx) && (has_chat || panel_is_visible(workspace, cx)) {
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
                move_chat_to_tab(workspace, window, cx);
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
    let view = match chat_view_for(workspace, &session, cx) {
        Some(view) => view,
        None => {
            let project = workspace.project().clone();
            let weak_workspace = cx.entity().downgrade();
            cx.new(|cx| ChatView::new(session, store, project, weak_workspace, window, cx))
        }
    };
    dock_chat(workspace, view, false, window, cx);
}

pub(crate) fn move_chat_to_panel(
    workspace: &mut Workspace,
    view: Entity<ChatView>,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    dock_chat(workspace, view, true, window, cx);
}

fn dock_chat(
    workspace: &mut Workspace,
    view: Entity<ChatView>,
    previous_to_tab: bool,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    if !agent_chat_enabled(cx) {
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
    let previous = panel.update(cx, |panel, cx| panel.set_chat(Some(view.clone()), cx));
    view.update(cx, |view, cx| view.set_docked(true, window, cx));
    if let Some(previous) = previous.filter(|previous| *previous != view) {
        previous.update(cx, |previous, cx| previous.set_docked(false, window, cx));
        if previous_to_tab {
            workspace.add_item_to_active_pane(Box::new(previous), None, false, window, cx);
        }
    }
    workspace.focus_panel::<ChatPanel>(window, cx);
}

pub(crate) fn move_chat_to_tab(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let Some(panel) = workspace.panel::<ChatPanel>(cx) else {
        return;
    };
    let Some(view) = panel.update(cx, |panel, cx| panel.set_chat(None, cx)) else {
        return;
    };
    remember_where_chats_open(false, cx);
    view.update(cx, |view, cx| view.set_docked(false, window, cx));
    workspace.close_panel::<ChatPanel>(window, cx);
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

fn saved_chat_key(workspace_id: WorkspaceId) -> String {
    format!("agent_chat_panel_{}", i64::from(workspace_id))
}

struct DockedChat {
    view: Entity<ChatView>,
    _events: Subscription,
}

pub struct ChatPanel {
    workspace: WeakEntity<Workspace>,
    workspace_id: Option<WorkspaceId>,
    focus_handle: FocusHandle,
    chat: Option<DockedChat>,
    restoring: bool,
}

impl ChatPanel {
    pub async fn load(
        workspace: WeakEntity<Workspace>,
        mut cx: AsyncWindowContext,
    ) -> Result<Entity<Self>> {
        let (panel, saved_chat, store, project) =
            workspace.update_in(&mut cx, |workspace, _, cx| {
                let workspace_id = workspace.database_id();
                let saved_chat = workspace_id
                    .filter(|_| agent_chat_enabled(cx))
                    .and_then(|id| {
                        KeyValueStore::global(cx)
                            .read_kvp(&saved_chat_key(id))
                            .log_err()
                            .flatten()
                    });
                let weak_workspace = cx.entity().downgrade();
                let panel = cx.new(|cx| Self::new(weak_workspace, workspace_id, cx));
                (
                    panel,
                    saved_chat,
                    store_for(workspace, cx),
                    workspace.project().clone(),
                )
            })?;
        if let Some(chat_id) = saved_chat {
            panel.update_in(&mut cx, |panel, window, cx| {
                panel.restore(chat_id, store, project, window, cx)
            })?;
        }
        Ok(panel)
    }

    fn new(
        workspace: WeakEntity<Workspace>,
        workspace_id: Option<WorkspaceId>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            workspace,
            workspace_id,
            focus_handle: cx.focus_handle(),
            chat: None,
            restoring: false,
        }
    }

    fn restore(
        &mut self,
        chat_id: String,
        store: Entity<AgentStore>,
        project: Entity<Project>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let opening = store.update(cx, |store, cx| store.open_saved_session(chat_id.into(), cx));
        let workspace = self.workspace.clone();
        self.restoring = true;
        cx.spawn_in(window, async move |this, cx| {
            let session = opening.await.log_err();
            this.update_in(cx, |this, window, cx| {
                this.restoring = false;
                if let Some(session) = session {
                    let shown_in_tab = workspace.upgrade().is_some_and(|workspace| {
                        workspace
                            .read(cx)
                            .items_of_type::<ChatView>(cx)
                            .any(|view| view.read(cx).session() == &session)
                    });
                    if this.chat.is_none() && !shown_in_tab && agent_chat_enabled(cx) {
                        let kind = session.read(cx).kind();
                        store.update(cx, |store, cx| store.update_agent_if_due(kind, cx));
                        let view = cx.new(|cx| {
                            ChatView::new(session, store, project, workspace, window, cx)
                        });
                        view.update(cx, |view, cx| view.set_docked(true, window, cx));
                        this.set_chat(Some(view), cx);
                    }
                }
                if this.chat.is_none() {
                    cx.emit(PanelEvent::Close);
                }
            })
            .log_err();
        })
        .detach();
    }

    pub(crate) fn chat(&self) -> Option<&Entity<ChatView>> {
        self.chat.as_ref().map(|chat| &chat.view)
    }

    pub(crate) fn set_chat(
        &mut self,
        view: Option<Entity<ChatView>>,
        cx: &mut Context<Self>,
    ) -> Option<Entity<ChatView>> {
        let chat = view.map(|view| DockedChat {
            _events: cx.subscribe(&view, |this, _, event: &ItemEvent, cx| match event {
                ItemEvent::CloseItem => {
                    this.set_chat(None, cx);
                    cx.emit(PanelEvent::Close);
                }
                ItemEvent::UpdateTab => cx.notify(),
                _ => {}
            }),
            view,
        });
        let previous = std::mem::replace(&mut self.chat, chat).map(|chat| chat.view);
        self.save(cx);
        cx.notify();
        previous
    }

    fn save(&self, cx: &App) {
        let Some(workspace_id) = self.workspace_id else {
            return;
        };
        let key = saved_chat_key(workspace_id);
        let chat_id = self
            .chat()
            .map(|view| view.read(cx).session().read(cx).metadata().id.clone());
        let db = KeyValueStore::global(cx);
        db::write_and_log(cx, move || async move {
            match chat_id {
                Some(chat_id) => db.write_kvp(key, chat_id).await,
                None => db.delete_kvp(key).await,
            }
        });
    }

    fn new_chat_menu(&self) -> impl IntoElement {
        let workspace = self.workspace.clone();
        PopoverMenu::new("chat-panel-new-chat")
            .trigger(HeaderButton::new(
                "chat-panel-new-chat-trigger",
                IconName::Plus,
                t!("New Chat"),
            ))
            .anchor(gpui::Anchor::TopRight)
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

    fn render_header(&self, cx: &Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let (icon, title) = match self.chat() {
            Some(view) => {
                let view = view.read(cx);
                (
                    Some(agent_icon(view.session().read(cx).kind())),
                    view.title(cx),
                )
            }
            None => (None, t!("Chat")),
        };
        h_flex()
            .flex_none()
            .gap(px(4.))
            .px(px(8.))
            .pt(px(8.))
            .pb(px(4.))
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .h(px(HEADER_CONTROL_SIZE))
                    .gap(px(8.))
                    .px(px(8.))
                    .text_size(ui(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors.text.opacity(0.8))
                    .children(icon.map(|icon| icon.size(IconSize::Small)))
                    .child(div().min_w_0().truncate().child(title)),
            )
            .child(self.new_chat_menu())
            .when(self.chat.is_some(), |this| {
                this.child(
                    HeaderButton::new(
                        "chat-panel-open-in-tab",
                        IconName::Maximize,
                        t!("Open in Tab"),
                    )
                    .on_click(|_, window, cx| {
                        window.dispatch_action(MoveChatToTab.boxed_clone(), cx)
                    }),
                )
            })
            .child(
                HeaderButton::new("chat-panel-close", IconName::Close, t!("Close"))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(PanelEvent::Close))),
            )
    }
}

impl EventEmitter<PanelEvent> for ChatPanel {}

impl Focusable for ChatPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ChatPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("chat-panel")
            .key_context("ChatPanel")
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(cx.theme().colors().editor_background)
            .child(self.render_header(cx))
            .child(div().flex_1().min_h_0().children(self.chat().cloned()))
    }
}

impl Panel for ChatPanel {
    fn persistent_name() -> &'static str {
        "Chat Panel"
    }

    fn activation_focus_handle(&self, cx: &App) -> FocusHandle {
        match self.chat() {
            Some(view) => view.focus_handle(cx),
            None => self.focus_handle.clone(),
        }
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

    fn enabled(&self, cx: &App) -> bool {
        agent_chat_enabled(cx)
    }

    fn set_active(&mut self, active: bool, _: &mut Window, cx: &mut Context<Self>) {
        let empty = self.chat.is_none() && !self.restoring;
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
            let panel = cx.new(|cx| ChatPanel::new(weak_workspace, None, cx));
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

    fn open_tab(setup: &Setup, cx: &mut VisualTestContext) -> Entity<ChatView> {
        let session = setup.store.update(cx, |store, cx| {
            store.create_session(AgentKind::Claude, setup.directory.path().to_path_buf(), cx)
        });
        setup.workspace.update_in(cx, |workspace, window, cx| {
            let weak_workspace = cx.entity().downgrade();
            let (store, project) = (setup.store.clone(), setup.project.clone());
            let view =
                cx.new(|cx| ChatView::new(session, store, project, weak_workspace, window, cx));
            workspace.add_item_to_active_pane(Box::new(view.clone()), None, true, window, cx);
            view
        })
    }

    fn tabs(setup: &Setup, cx: &mut VisualTestContext) -> Vec<Entity<ChatView>> {
        setup.workspace.read_with(cx, |workspace, cx| {
            workspace.items_of_type::<ChatView>(cx).collect()
        })
    }

    fn docked(setup: &Setup, cx: &mut VisualTestContext) -> Option<Entity<ChatView>> {
        setup.panel.read_with(cx, |panel, _| panel.chat().cloned())
    }

    #[gpui::test]
    async fn moving_chats_between_tabs_and_the_panel(cx: &mut TestAppContext) {
        let (setup, cx) = setup(cx).await;
        let first = open_tab(&setup, cx);
        setup.workspace.update_in(cx, |workspace, window, cx| {
            move_chat_to_panel(workspace, first.clone(), window, cx)
        });
        cx.run_until_parked();
        assert!(tabs(&setup, cx).is_empty());
        assert_eq!(docked(&setup, cx), Some(first.clone()));
        assert!(first.read_with(cx, |view, _| view.is_docked()));
        let found = setup.workspace.read_with(cx, |workspace, cx| {
            chat_view_for(workspace, first.read(cx).session(), cx)
        });
        assert_eq!(found, Some(first.clone()));

        let second = open_tab(&setup, cx);
        setup.workspace.update_in(cx, |workspace, window, cx| {
            move_chat_to_panel(workspace, second.clone(), window, cx)
        });
        cx.run_until_parked();
        assert_eq!(tabs(&setup, cx), std::slice::from_ref(&first));
        assert_eq!(docked(&setup, cx), Some(second.clone()));
        assert!(!first.read_with(cx, |view, _| view.is_docked()));

        setup.workspace.update_in(cx, |workspace, window, cx| {
            move_chat_to_tab(workspace, window, cx)
        });
        cx.run_until_parked();
        assert_eq!(docked(&setup, cx), None);
        assert_eq!(tabs(&setup, cx).len(), 2);
        assert!(!second.read_with(cx, |view, _| view.is_docked()));
    }

    #[gpui::test]
    async fn the_chat_moves_itself_to_the_panel(cx: &mut TestAppContext) {
        let (setup, cx) = setup(cx).await;
        let chat = open_tab(&setup, cx);
        chat.update_in(cx, |chat, window, cx| {
            chat.move_to_panel(&MoveChatToPanel, window, cx)
        });
        cx.run_until_parked();
        assert!(tabs(&setup, cx).is_empty());
        assert_eq!(docked(&setup, cx), Some(chat));
    }

    #[gpui::test]
    async fn deleting_the_docked_chat_empties_the_panel(cx: &mut TestAppContext) {
        let (setup, cx) = setup(cx).await;
        let chat = open_tab(&setup, cx);
        setup.workspace.update_in(cx, |workspace, window, cx| {
            move_chat_to_panel(workspace, chat.clone(), window, cx)
        });
        cx.run_until_parked();
        let id = chat.read_with(cx, |chat, cx| chat.session().read(cx).metadata().id.clone());
        setup
            .store
            .update(cx, |store, cx| store.delete_session(&id, cx));
        cx.run_until_parked();
        assert_eq!(docked(&setup, cx), None);
        assert!(tabs(&setup, cx).is_empty());
    }

    #[gpui::test]
    async fn closing_agent_chats_empties_the_panel(cx: &mut TestAppContext) {
        let (setup, cx) = setup(cx).await;
        let chat = open_tab(&setup, cx);
        setup.workspace.update_in(cx, |workspace, window, cx| {
            move_chat_to_panel(workspace, chat, window, cx)
        });
        cx.run_until_parked();
        cx.update(|_, cx| cx.defer(crate::close_agent_chats));
        cx.run_until_parked();
        assert_eq!(docked(&setup, cx), None);
        assert!(tabs(&setup, cx).is_empty());
    }

    fn new_session(setup: &Setup, cx: &mut VisualTestContext) -> Entity<AgentSession> {
        setup.store.update(cx, |store, cx| {
            store.create_session(AgentKind::Claude, setup.directory.path().to_path_buf(), cx)
        })
    }

    fn panel_open(setup: &Setup, cx: &mut VisualTestContext) -> bool {
        setup
            .workspace
            .read_with(cx, |workspace, cx| panel_is_visible(workspace, cx))
    }

    #[gpui::test]
    async fn picking_a_chat_while_the_panel_shows_one_swaps_it_in(cx: &mut TestAppContext) {
        let (setup, cx) = setup(cx).await;
        let first = open_tab(&setup, cx);
        setup.workspace.update_in(cx, |workspace, window, cx| {
            move_chat_to_panel(workspace, first.clone(), window, cx)
        });
        cx.run_until_parked();
        assert!(panel_open(&setup, cx));

        let unopened = new_session(&setup, cx);
        setup.workspace.update_in(cx, |workspace, window, cx| {
            show_chat_in_panel(workspace, unopened.clone(), setup.store.clone(), window, cx)
        });
        cx.run_until_parked();
        let docked_session =
            docked(&setup, cx).map(|view| view.read_with(cx, |view, _| view.session().clone()));
        assert_eq!(docked_session, Some(unopened));
        assert!(tabs(&setup, cx).is_empty());
        assert!(!first.read_with(cx, |view, _| view.is_docked()));

        let tab = open_tab(&setup, cx);
        let tab_session = tab.read_with(cx, |view, _| view.session().clone());
        setup.workspace.update_in(cx, |workspace, window, cx| {
            show_chat_in_panel(workspace, tab_session, setup.store.clone(), window, cx)
        });
        cx.run_until_parked();
        assert_eq!(docked(&setup, cx), Some(tab));
        assert!(tabs(&setup, cx).is_empty());
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
        setup.workspace.update_in(cx, |workspace, window, cx| {
            move_chat_to_panel(workspace, chat, window, cx)
        });
        cx.run_until_parked();
        assert!(panel_open(&setup, cx));
        setup.workspace.update_in(cx, |workspace, window, cx| {
            workspace.close_panel::<ChatPanel>(window, cx)
        });
        cx.run_until_parked();
        assert!(!panel_open(&setup, cx));
        toggle_panel(&setup, cx);
        cx.run_until_parked();
        assert!(panel_open(&setup, cx));

        setup.workspace.update_in(cx, |workspace, window, cx| {
            move_chat_to_tab(workspace, window, cx)
        });
        cx.run_until_parked();
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
        setup.workspace.update_in(cx, |workspace, window, cx| {
            move_chat_to_panel(workspace, chat, window, cx)
        });
        cx.run_until_parked();
        assert!(opens_in_panel(cx));

        setup.workspace.update_in(cx, |workspace, window, cx| {
            workspace.close_panel::<ChatPanel>(window, cx)
        });
        assert!(opens_in_panel(cx));

        setup.workspace.update_in(cx, |workspace, window, cx| {
            move_chat_to_tab(workspace, window, cx)
        });
        cx.run_until_parked();
        assert!(!opens_in_panel(cx));
    }
}
