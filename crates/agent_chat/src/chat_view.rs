mod attachments;
mod background_tasks;
mod completion;
mod composer;
mod dictation;
mod diff_view;
mod image_viewer;
mod links;
mod mermaid;
mod outline;
mod persistence;
mod queue_panel;
mod search;
mod side_pane;
mod todo_panel;

use crate::{
    AgentChatSettings, MoveChatToPanel, MoveChatToTab, RenameChat, Send, Stop, SubmitAnswer,
    chat_style::{
        Chip, accent, flavour_seed, flavour_word, format_elapsed, gradient_spinner, hairline, ink,
        mix, page, text_faint, ui, wash, working_animation,
    },
    model_picker::{ModelPicker, Selection, agent_icon},
    session::{AgentSession, AgentStore, Entry, PendingQuestion, Prompt, ToolEntry, ToolStatus},
    slash_commands::{CommandToken, app_command_for_text},
    usage_rings::{ContextCard, UsageCard, context_chip, usage_chip},
};
use agent_harness::{ToolCall, UserInputAnswer, UserInputQuestion, view};
use i18n::{t, tc};
use settings::{AgentChatSendKey, Settings as _};
use gpui::Font;
use theme_settings::ThemeSettings;
use collections::{HashMap, HashSet};
use editor::{Editor, EditorEvent};
use gpui::{
    Anchor, Animation, AnimationExt as _, AnyElement, App, Bounds, ClipboardItem, ContentMask,
    Context, Entity, EventEmitter, FillOptions, FillRule, FocusHandle, Focusable, FontWeight,
    Hsla, IntoElement, ParentElement, KeyContext, PathBuilder, PathStyle, Pixels, Point, Render,
    ScrollHandle, SharedString, StyleRefinement, Styled, ObjectFit, Subscription, Svg,
    ease_out_quint, Task, TextAlign, TextRun, TextStyleRefinement, Transformation,
    UnderlineStyle, WeakEntity, Window, canvas, div, img, percentage, point, px, radians, size,
    svg,
};
use markdown::{
    CodeBlockRenderer, HeadingLevelStyles, Markdown, MarkdownElement, MarkdownFont, MarkdownOptions,
    MarkdownStyle, TABLE_GROUP, TableActionsFn, parser::CodeBlockKind,
};
use mermaid_render::{Diagram, DiagramTheme, MermaidDiagrams};
use project::Project;
use gpui::{FollowMode, ListAlignment, ListState, list};
#[cfg(test)]
use gpui::ListOffset;
use std::{
    cell::Cell,
    ops::Range,
    rc::Rc,
    {sync::Arc, time::Duration},
};
use theme::ActiveTheme as _;
use util::ResultExt as _;
use ui::{
    Icon, IconName, IconSize, Label, PopoverMenu, PopoverMenuHandle,
    Tooltip, prelude::*,
};
use std::time::Instant;
use gpui::ExternalPaths;
use workspace::{
    DraggedSelection, Workspace,
    item::{Item, ItemEvent, TabContentParams},
};

const CONTENT_WIDTH: f32 = 736.;
const COMPOSER_WIDTH: f32 = CONTENT_WIDTH + 32.;
const SIDE_GUTTER: f32 = 48.;
const LIST_OVERDRAW: f32 = 1200.;
const SCROLL_BUTTON_THRESHOLD: f32 = 320.;
const AT_BOTTOM: f32 = 2.;
const COMPOSER_RADIUS: f32 = 26.;
const DOCKED_COMPOSER_RADIUS: f32 = 22.;
const SEND_BUTTON_SIZE: f32 = 28.;
const USER_COLLAPSED_LINES: usize = 5;
const USER_COLLAPSE_CHARS: usize = 400;
const USER_LINE_HEIGHT: f32 = 22.;
const USER_TOGGLE_GAP: f32 = 8.;
const ATTACHMENT_THUMB_WIDTH: f32 = 112.;
const ATTACHMENT_THUMB_HEIGHT: f32 = 80.;
const GENERATED_IMAGE_MAX_WIDTH: f32 = 512.;
const GENERATED_IMAGE_MAX_HEIGHT: f32 = 420.;
const SPACE_SM: f32 = 8.;
const SPACE_MD: f32 = 12.;
const SPACE_LG: f32 = 16.;
const FIRST_ROW_BREATHING_ROOM: f32 = 10.;
const TRANSCRIPT_BOTTOM_PAD: f32 = 32.;
const REVEALED_HIGHLIGHT_PAD: f32 = 10.;
const MD_BLOCK_GAP: f32 = 12.;
const TREE_ROW_HEIGHT: f32 = 32.;
const TREE_GUTTER: f32 = 48.;
const TREE_TRUNK_X: f32 = 12.5;
const TREE_BEND_RADIUS: f32 = 6.;
const TREE_BRANCH_END_X: f32 = 28.;
const TREE_ICON_LEFT: f32 = 32.;
const TREE_ICON_SIZE: f32 = 16.;
const TREE_TEXT_GAP: f32 = 8.;
const CHIP_HEIGHT: f32 = 38.;
const CHIP_CARD_HEIGHT: f32 = 30.;
const CHIP_HEADER_HEIGHT: f32 = CHIP_CARD_HEIGHT - 2.;
const CHIPS_TOP_PAD: f32 = 2.;
const TOOL_GROUP_HEADER_HEIGHT: f32 = 26.;
const GROUP_CHEVRON_WIDTH: f32 = 22.;
const GROUP_HEADER_GAP: f32 = 6.;
const TOOL_TEXT_SIZE: f32 = 12.;
const THOUGHT_BODY_SIZE: f32 = 15.;
const THOUGHT_CODE_SIZE: f32 = 14.;
const TOOL_LINE_HEIGHT: f32 = 18.;
const TOOL_SHIMMER_HALF_WIDTH: f32 = 0.36;
const TOOL_SHIMMER_STRIP_WIDTH: f32 = 2.;
const DETAIL_SEPARATOR: f32 = 1.;
const BLOB_AFFORDANCE_HEIGHT: f32 = 24.;
const CALL_WRAP_COLUMNS: usize = 80;
const OUTPUT_MAX_LINES: usize = 24;
const OUTPUT_LINE_HEIGHT: f32 = 18.;
const CODE_HEADER_HEIGHT: f32 = 28.;
const CODE_ACTION_SIZE: f32 = 22.;
const DIAGRAM_MAX_HEIGHT: f32 = 480.;
const SUBAGENT_SPINNER_CELL: f32 = 2.;
const SUBAGENT_SPINNER_DIM: f32 = 0.1;
const SUBAGENT_SPINNER_PERIOD: Duration = Duration::from_millis(750);
const COPIED_FEEDBACK: Duration = Duration::from_millis(1200);
const DOWNLOADED_FEEDBACK: Duration = Duration::from_secs(3);
const REVEALED_HIGHLIGHT: Duration = Duration::from_secs(2);
const AUTO_ADVANCE: Duration = Duration::from_millis(220);
const USAGE_POLL_INTERVAL: Duration = Duration::from_secs(5 * 60);
const SEARCH_REFRESH_DELAY: Duration = Duration::from_millis(250);
const GROUP_SHIMMER: Duration = Duration::from_millis(3400);
const FADE_IN: Duration = Duration::from_millis(260);
const GLIDE: Duration = Duration::from_millis(280);
const GLIDE_FRAME: Duration = Duration::from_millis(16);

#[derive(Clone)]
struct SubagentRef {
    id: String,
    title: SharedString,
}

pub struct ChatView {
    session: Entity<AgentSession>,
    subagent: Option<SubagentRef>,
    host: Option<WeakEntity<ChatView>>,
    docked: bool,
    side_pane: Option<side_pane::SidePane>,
    side_pane_openings: usize,
    _side_pane_load: Task<()>,
    workspace: WeakEntity<Workspace>,
    focus_handle: FocusHandle,
    store: Entity<AgentStore>,
    project: Entity<Project>,
    composer: Entity<Editor>,
    list: ListState,
    transcript: Transcript,
    hovered_row: Option<Row>,
    frame_message_style: Option<MarkdownStyle>,
    scanned_entries: usize,
    scanned_regroups: usize,
    scanned_tail: Option<(gpui::EntityId, usize)>,
    latest_todo: Option<usize>,
    tab_state: Option<TabState>,
    search_refresh_pending: bool,
    _search_refresh: Task<()>,
    has_rendered: bool,
    update_check_due: bool,
    window_active: bool,
    images: Entity<image_viewer::ChatImageCache>,
    show_scroll_button: bool,
    expanded_rows: HashSet<String>,
    group_overrides: HashMap<String, bool>,
    expanded_users: HashSet<usize>,
    expanded_questions: HashSet<String>,
    user_markdown: HashMap<usize, Entity<Markdown>>,
    selected_options: HashMap<String, Vec<String>>,
    question_pages: HashMap<String, usize>,
    answer_editors: HashMap<String, (Entity<Editor>, Subscription)>,
    question_focus: FocusHandle,
    command_menu: Option<completion::CompletionMenu>,
    mention_results: Vec<completion::MentionItem>,
    _mention_search: Task<()>,
    composer_skills: Vec<agent_harness::SkillRef>,
    attachments: Vec<std::path::PathBuf>,
    lightbox: Option<image_viewer::Lightbox>,
    generated_image_layouts: std::cell::RefCell<HashMap<std::path::PathBuf, Option<(Pixels, f32)>>>,
    diff_cache: std::cell::RefCell<HashMap<String, Arc<diff_view::FileDiffView>>>,
    diff_tasks: std::cell::RefCell<Vec<Task<()>>>,
    full_output: HashSet<String>,
    dictation: dictation::DictationState,
    dictation_model_ready: std::cell::Cell<Option<bool>>,
    _dictation_levels: Task<()>,
    _dictation_events: Task<()>,
    _dictation_download: Task<()>,
    _glide: Task<()>,
    container_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    root_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    painted_rows: Rc<std::cell::RefCell<HashMap<Row, Bounds<Pixels>>>>,
    this: WeakEntity<ChatView>,
    rename_editor: Option<Entity<Editor>>,
    _rename_subscription: Option<Subscription>,
    background_ticking: bool,
    _background_tick: Task<()>,
    search: search::SearchState,
    outline_hover: Option<usize>,
    expanded_work: HashSet<usize>,
    todo_panel: todo_panel::TodoPanelState,
    known_paths: HashMap<std::path::PathBuf, links::PathState>,
    next_path_recheck: Option<Instant>,
    link_menu: Option<links::LinkMenu>,
    diagrams: Entity<MermaidDiagrams>,
    diagram_theme: Option<(Arc<theme::Theme>, DiagramTheme)>,
    diagram_images: HashMap<u64, Arc<gpui::Image>>,
    diagram_sources: HashSet<String>,
    diagram_sources_shown: HashSet<String>,
    editing_queued: Option<queue_panel::QueuedEdit>,
    lightbox_focus: FocusHandle,
    attachment_error: Option<SharedString>,
    staging_count: usize,
    dismissed_command: Option<CommandToken>,
    command_scroll: ScrollHandle,
    model_picker: PopoverMenuHandle<ModelPicker>,
    copied: Option<SharedString>,
    revealed_turn: Option<usize>,
    flavour_seed: u64,
    _copied_reset: Task<()>,
    _revealed_reset: Task<()>,
    _auto_advance: Task<()>,
    _poll_usage: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl ChatView {
    pub fn new(
        session: Entity<AgentSession>,
        store: Entity<AgentStore>,
        project: Entity<Project>,
        workspace: WeakEntity<Workspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::build(session, None, store, project, workspace, window, cx)
    }

    fn build(
        session: Entity<AgentSession>,
        subagent: Option<SubagentRef>,
        store: Entity<AgentStore>,
        project: Entity<Project>,
        workspace: WeakEntity<Workspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let flavour = flavour_seed(&session.read(cx).metadata().id);
        let composer = cx.new(|cx| {
            let mut editor = Editor::auto_height(1, 10, window, cx);
            editor.set_placeholder_text(&t!("Ask the agent. @ for files, / for commands"), window, cx);
            editor.set_bar_cursor_width(px(1.), cx);
            editor.set_text_style_refinement(TextStyleRefinement {
                font_size: Some(ui(14.).into()),
                line_height: Some(ui(22.75).into()),
                ..Default::default()
            });
            editor
        });
        let markdown = store.read(cx).languages().language_for_name("Markdown");
        cx.spawn({
            let composer = composer.clone();
            async move |_, cx| {
                let Some(markdown) = markdown.await.log_err() else {
                    return;
                };
                composer.update(cx, |editor, cx| {
                    if let Some(buffer) = editor.buffer().read(cx).as_singleton() {
                        buffer.update(cx, |buffer, cx| buffer.set_language(Some(markdown), cx));
                    }
                });
            }
        })
        .detach();
        let list = ListState::new(0, ListAlignment::Top, px(LIST_OVERDRAW));
        list.set_follow_mode(FollowMode::Tail);
        let diagrams = cx.new(|_| MermaidDiagrams::default());
        let images = image_viewer::ChatImageCache::new(cx);
        let subscriptions = vec![
            cx.observe_in(&session, window, |this, session, window, cx| {
                if session.read(cx).is_deleted() {
                    cx.emit(ItemEvent::CloseItem);
                    return;
                }
                this.sync_transcript(cx);
                this.schedule_search_refresh(cx);
                if this.subagent.is_none() {
                    this.sync_answer_editors(window, cx);
                }
                this.sync_tab(cx);
                cx.notify();
            }),
            cx.subscribe(&composer, |this, _, event: &EditorEvent, cx| match event {
                EditorEvent::BufferEdited => {
                    this.update_command_menu(cx);
                    cx.notify();
                }
                EditorEvent::SelectionsChanged { .. } => this.update_command_menu(cx),
                _ => {}
            }),
            cx.on_release(|this, cx| {
                this.clear_search_highlights(cx);
                this.release_diagram_images(None, cx);
            }),
            cx.observe_window_activation(window, |this, window, cx| {
                this.window_active = window.is_window_active();
                if this.window_active && this.has_rendered && this.subagent.is_none() {
                    let kind = this.session.read(cx).kind();
                    this.store
                        .update(cx, |store, cx| store.refresh_plan_usage(kind, false, cx));
                }
                cx.notify();
            }),
            cx.observe(&store, |this, _, cx| {
                if this.subagent.is_none() {
                    this.pin_default_settings(cx);
                }
                cx.notify();
            }),
            cx.observe(&diagrams, |this, _, cx| {
                this.track_diagram_images(cx);
                cx.notify();
            }),
        ];
        let mut view = Self {
            session,
            subagent,
            host: None,
            docked: false,
            side_pane: None,
            side_pane_openings: 0,
            _side_pane_load: Task::ready(()),
            workspace,
            focus_handle: cx.focus_handle(),
            store,
            project,
            composer,
            list,
            transcript: Transcript::default(),
            hovered_row: None,
            frame_message_style: None,
            scanned_entries: 0,
            scanned_regroups: 0,
            scanned_tail: None,
            latest_todo: None,
            tab_state: None,
            search_refresh_pending: false,
            _search_refresh: Task::ready(()),
            has_rendered: false,
            update_check_due: false,
            window_active: window.is_window_active(),
            images,
            show_scroll_button: false,
            expanded_rows: HashSet::default(),
            group_overrides: HashMap::default(),
            expanded_users: HashSet::default(),
            expanded_questions: HashSet::default(),
            user_markdown: HashMap::default(),
            selected_options: HashMap::default(),
            question_pages: HashMap::default(),
            answer_editors: HashMap::default(),
            question_focus: cx.focus_handle(),
            command_menu: None,
            mention_results: Vec::new(),
            _mention_search: Task::ready(()),
            composer_skills: Vec::new(),
            attachments: Vec::new(),
            lightbox: None,
            generated_image_layouts: Default::default(),
            diff_cache: Default::default(),
            diff_tasks: Default::default(),
            full_output: HashSet::default(),
            dictation: dictation::DictationState::Idle,
            dictation_model_ready: Default::default(),
            _dictation_levels: Task::ready(()),
            _dictation_events: Task::ready(()),
            _dictation_download: Task::ready(()),
            _glide: Task::ready(()),
            container_bounds: Default::default(),
            root_bounds: Default::default(),
            painted_rows: Default::default(),
            this: cx.entity().downgrade(),
            rename_editor: None,
            _rename_subscription: None,
            background_ticking: false,
            _background_tick: Task::ready(()),
            search: Default::default(),
            outline_hover: None,
            expanded_work: HashSet::default(),
            todo_panel: Default::default(),
            known_paths: HashMap::default(),
            next_path_recheck: None,
            link_menu: None,
            diagrams,
            diagram_theme: None,
            diagram_images: HashMap::default(),
            diagram_sources: HashSet::default(),
            diagram_sources_shown: HashSet::default(),
            editing_queued: None,
            lightbox_focus: cx.focus_handle(),
            attachment_error: None,
            staging_count: 0,
            dismissed_command: None,
            command_scroll: ScrollHandle::new(),
            model_picker: PopoverMenuHandle::default(),
            copied: None,
            revealed_turn: None,
            flavour_seed: flavour,
            _copied_reset: Task::ready(()),
            _revealed_reset: Task::ready(()),
            _auto_advance: Task::ready(()),
            _poll_usage: Task::ready(()),
            _subscriptions: subscriptions,
        };
        view.pin_default_settings(cx);
        view.sync_answer_editors(window, cx);
        view.sync_transcript(cx);
        view.tab_state = Some(view.current_tab_state(cx));
        view
    }

    fn start_lookups(&mut self, cx: &mut Context<Self>) {
        let kind = self.session.read(cx).kind();
        let check_update = std::mem::take(&mut self.update_check_due);
        let store = self.store.clone();
        cx.defer(move |cx| {
            store.update(cx, |store, cx| {
                store.ensure_models(kind, cx);
                store.refresh_plan_usage(kind, false, cx);
                if check_update {
                    store.update_agent_if_due(kind, cx);
                }
            })
        });
        self._poll_usage = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(USAGE_POLL_INTERVAL).await;
                let polled = this.update(cx, |this, cx| {
                    if this.window_active {
                        this.store
                            .update(cx, |store, cx| store.refresh_plan_usage(kind, false, cx));
                    }
                });
                if polled.is_err() {
                    return;
                }
            }
        });
    }

    fn current_tab_state(&self, cx: &App) -> TabState {
        TabState {
            title: self.title(cx),
            working: self.is_working(cx),
            needs_input: self.subagent.is_none()
                && !self.session.read(cx).pending_questions().is_empty(),
        }
    }

    fn sync_tab(&mut self, cx: &mut Context<Self>) {
        let state = self.current_tab_state(cx);
        if self.tab_state.as_ref() != Some(&state) {
            self.tab_state = Some(state);
            cx.emit(ItemEvent::UpdateTab);
        }
    }

    fn schedule_search_refresh(&mut self, cx: &mut Context<Self>) {
        if self.search.is_empty() || self.search_refresh_pending {
            return;
        }
        self.search_refresh_pending = true;
        self._search_refresh = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SEARCH_REFRESH_DELAY).await;
            this.update(cx, |this, cx| {
                this.search_refresh_pending = false;
                if !this.search.is_empty() {
                    cx.emit(workspace::searchable::SearchEvent::MatchesInvalidated);
                }
            })
            .ok();
        });
    }

    fn sync_answer_editors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let session = self.session.read(cx);
        self.expanded_questions.retain(|id| {
            session
                .pending_questions()
                .iter()
                .flat_map(|pending| &pending.questions)
                .any(|question| &question.id == id)
        });
        let focus_was_in_question = self.question_focus.is_focused(window)
            || self
                .answer_editors
                .values()
                .any(|(editor, _)| editor.focus_handle(cx).is_focused(window));
        let has_question = !session.pending_questions().is_empty();
        let free_text: Vec<(String, bool, bool)> = session
            .pending_questions()
            .iter()
            .filter(|pending| !pending.is_permission())
            .flat_map(|pending| &pending.questions)
            .filter(|question| question.options.is_empty() || question.accepts_typed_answer)
            .map(|question| {
                (
                    question.id.clone(),
                    question.options.is_empty(),
                    question.multi_select,
                )
            })
            .collect();
        self.answer_editors.retain(|question_id, _| {
            free_text.iter().any(|(id, _, _)| id == question_id)
        });
        if has_question && self.composer.focus_handle(cx).is_focused(window) {
            window.focus(&self.question_focus, cx);
        } else if !has_question && focus_was_in_question {
            window.focus(&self.composer.focus_handle(cx), cx);
        }
        for (question_id, standalone, multi_select) in free_text {
            if self.answer_editors.contains_key(&question_id) {
                continue;
            }
            let editor = cx.new(|cx| {
                let mut editor = Editor::auto_height(1, 6, window, cx);
                let (placeholder, font_size, line_height) = if standalone {
                    (t!("Type your answer…"), 14., 22.75)
                } else {
                    (t!("Something else"), 13.5, 20.)
                };
                editor.set_placeholder_text(&placeholder, window, cx);
                editor.set_text_style_refinement(TextStyleRefinement {
                    font_size: Some(ui(font_size).into()),
                    line_height: Some(ui(line_height).into()),
                    ..Default::default()
                });
                editor
            });
            let subscription = cx.subscribe(&editor, {
                let question_id = question_id.clone();
                move |this, editor, event: &EditorEvent, cx| {
                    if !matches!(event, EditorEvent::BufferEdited) {
                        return;
                    }
                    if !multi_select && !editor.read(cx).text(cx).trim().is_empty() {
                        this.selected_options.remove(&question_id);
                        this._auto_advance = Task::ready(());
                    }
                    cx.notify();
                }
            });
            self.answer_editors
                .insert(question_id, (editor, subscription));
        }
    }

    fn answer_labels(&self, question: &UserInputQuestion, cx: &App) -> Vec<String> {
        let mut labels = self
            .selected_options
            .get(&question.id)
            .cloned()
            .unwrap_or_default();
        if (labels.is_empty() || question.multi_select)
            && let Some((editor, _)) = self.answer_editors.get(&question.id)
        {
            let text = editor.read(cx).text(cx).trim().to_string();
            if !text.is_empty() {
                labels.push(text);
            }
        }
        labels
    }

    fn pin_default_settings(&mut self, cx: &mut Context<Self>) {
        let session = self.session.read(cx);
        let Some(models) = self.store.read(cx).models_discovered(session.kind()) else {
            return;
        };
        if session.settings().model.is_some() {
            if let Some(reasoning) = crate::session::reasoning_for_model(session.settings(), models)
            {
                self.session.update(cx, |session, cx| {
                    session.pin_settings(|settings| settings.reasoning = reasoning, cx)
                });
            }
            return;
        }
        let Some(model) = view::default_model(models).cloned() else {
            return;
        };
        self.session.update(cx, |session, cx| {
            session.pin_settings(
                |settings| {
                    settings.reasoning =
                        view::clamp_reasoning(settings.reasoning, &model.reasoning_levels);
                    settings.model = Some(model.id.clone());
                },
                cx,
            )
        });
    }

    pub fn session(&self) -> &Entity<AgentSession> {
        &self.session
    }

    pub fn is_subagent(&self) -> bool {
        self.subagent.is_some()
    }

    pub(crate) fn is_docked(&self) -> bool {
        self.docked
    }

    pub(crate) fn set_docked(
        &mut self,
        docked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.docked = docked;
        if docked {
            self.close_side_pane(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn move_to_panel(
        &mut self,
        _: &MoveChatToPanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.subagent.is_some() || self.docked {
            cx.propagate();
            return;
        }
        let Some(view) = self.this.upgrade() else {
            return;
        };
        let workspace = self.workspace.clone();
        window.defer(cx, move |window, cx| {
            workspace
                .update(cx, |workspace, cx| {
                    crate::chat_panel::move_chat_to_panel(workspace, view, window, cx)
                })
                .log_err();
        });
    }

    pub(crate) fn move_to_tab(
        &mut self,
        _: &MoveChatToTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.docked {
            cx.propagate();
            return;
        }
        let Some(view) = self.this.upgrade() else {
            return;
        };
        let workspace = self.workspace.clone();
        window.defer(cx, move |window, cx| {
            workspace
                .update(cx, |workspace, cx| {
                    crate::chat_panel::move_chat_to_tab(workspace, view, window, cx)
                })
                .log_err();
        });
    }

    fn entries<'a>(&self, cx: &'a App) -> &'a [Entry] {
        let session = self.session.read(cx);
        match &self.subagent {
            Some(subagent) => session
                .subagent(&subagent.id)
                .map_or(&[], |subagent| subagent.entries()),
            None => session.entries(),
        }
    }

    fn is_working(&self, cx: &App) -> bool {
        let session = self.session.read(cx);
        match &self.subagent {
            Some(subagent) => session.subagent_running(&subagent.id),
            None => session.is_working(),
        }
    }

    fn working_since(&self, cx: &App) -> Option<Instant> {
        let session = self.session.read(cx);
        match &self.subagent {
            Some(subagent) => session
                .subagent_running(&subagent.id)
                .then(|| session.subagent(&subagent.id).map(|subagent| subagent.started_at()))
                .flatten(),
            None => session.working_since(),
        }
    }

    fn open_subagent(
        &mut self,
        id: String,
        call: &ToolCall,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let subagent = SubagentRef {
            id,
            title: subagent_title(call),
        };
        match self.host.as_ref().and_then(|host| host.upgrade()) {
            Some(host) => host.update(cx, |host, cx| {
                host.open_subagent_in_side_pane(subagent, window, cx)
            }),
            None => self.open_subagent_in_side_pane(subagent, window, cx),
        }
    }

    fn open_subagent_in_tab(
        &mut self,
        subagent: SubagentRef,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session = self.session.clone();
        let store = self.store.clone();
        let project = self.project.clone();
        let workspace = self.workspace.clone();
        window.defer(cx, move |window, cx| {
            workspace
                .update(cx, |workspace, cx| {
                    let existing = workspace.items_of_type::<ChatView>(cx).find(|view| {
                        let view = view.read(cx);
                        view.session == session
                            && view.subagent.as_ref().map(|open| &open.id) == Some(&subagent.id)
                    });
                    if let Some(view) = existing {
                        workspace.activate_item(&view, true, true, window, cx);
                        return;
                    }
                    let weak_workspace = cx.entity().downgrade();
                    let view = cx.new(|cx| {
                        ChatView::build(
                            session,
                            Some(subagent),
                            store,
                            project,
                            weak_workspace,
                            window,
                            cx,
                        )
                    });
                    workspace.add_item_to_active_pane(Box::new(view), None, true, window, cx);
                })
                .log_err();
        });
    }

    fn rename_chat(&mut self, _: &RenameChat, window: &mut Window, cx: &mut Context<Self>) {
        if self.subagent.is_some() || self.rename_editor.is_some() {
            return;
        }
        let title = self.title(cx);
        let editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_text(title, window, cx);
            editor.select_all(&Default::default(), window, cx);
            editor
        });
        let subscription = cx.subscribe_in(&editor, window, {
            let editor = editor.clone();
            move |_, _, event: &EditorEvent, window, cx| {
                if matches!(event, EditorEvent::Blurred) {
                    // Opening from a double-click briefly moves focus, so wait for it to settle.
                    let editor = editor.clone();
                    cx.defer_in(window, move |this, window, cx| {
                        let current = this.rename_editor.as_ref() == Some(&editor);
                        if current && !editor.focus_handle(cx).is_focused(window) {
                            this.finish_renaming(true, window, cx);
                        }
                    });
                }
            }
        });
        window.focus(&editor.focus_handle(cx), cx);
        self.rename_editor = Some(editor);
        self._rename_subscription = Some(subscription);
        cx.emit(ItemEvent::UpdateTab);
        cx.notify();
    }

    fn finish_renaming(&mut self, save: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.rename_editor.take() else {
            return;
        };
        self._rename_subscription = None;
        if save {
            let text = editor.read(cx).text(cx);
            let id = self.session.read(cx).metadata().id.clone();
            self.store
                .update(cx, |store, cx| store.rename_session(&id, text, cx));
        }
        if editor.focus_handle(cx).is_focused(window) {
            window.focus(&self.composer.focus_handle(cx), cx);
        }
        cx.emit(ItemEvent::UpdateTab);
        cx.notify();
    }

    pub(crate) fn title(&self, cx: &App) -> SharedString {
        if let Some(subagent) = &self.subagent {
            return subagent.title.clone();
        }
        crate::session::display_title(&self.session.read(cx).metadata().title)
    }

    fn send(&mut self, _: &Send, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.composer.read(cx).text(cx);
        if self.session.read(cx).is_deleted() || self.staging_count > 0 {
            return;
        }
        if text.trim().is_empty() && self.attachments.is_empty() {
            if self.editing_queued.is_some() {
                self.cancel_queued_edit(window, cx);
            } else {
                self.steer_newest_queued(cx);
            }
            return;
        }
        self.composer
            .update(cx, |editor, cx| editor.clear(window, cx));
        if self.attachments.is_empty()
            && self.editing_queued.is_none()
            && let Some(command) = app_command_for_text(&text, &self.command_items(cx))
        {
            self.run_app_command(command, window, cx);
            return;
        }
        if self.attachments.is_empty()
            && self.editing_queued.is_none()
            && self.redirect_hidden_command(&text, window, cx)
        {
            return;
        }
        let skills = std::mem::take(&mut self.composer_skills)
            .into_iter()
            .filter(|skill| text.contains(&format!("${}", skill.name)))
            .collect();
        let prompt = Prompt {
            text,
            attachments: std::mem::take(&mut self.attachments),
            skills,
        };
        self.attachment_error = None;
        if self.editing_queued.is_some() {
            self.save_queued_edit(prompt, window, cx);
            return;
        }
        self.session
            .update(cx, |session, cx| session.send_message(prompt, cx));
        self.glide_to_bottom(cx);
    }

    fn glide_to_bottom(&mut self, cx: &mut Context<Self>) {
        let start = self.scroll_top();
        self._glide = cx.spawn(async move |this, cx| {
            let frames = (GLIDE.as_millis() / GLIDE_FRAME.as_millis()).max(1) as u32;
            for frame in 1..=frames {
                cx.background_executor().timer(GLIDE_FRAME).await;
                let progress = ease_out_quint()(frame as f32 / frames as f32);
                let still_open = this.update(cx, |this, cx| {
                    let target = this.list.max_offset_for_scrollbar().y;
                    let y = start + (target - start) * progress;
                    this.list.set_offset_from_scrollbar(point(px(0.), -y));
                    cx.notify();
                });
                if still_open.is_err() {
                    return;
                }
            }
            this.update(cx, |this, cx| {
                this.list.set_follow_mode(FollowMode::Tail);
                cx.notify();
            })
            .ok();
        });
    }

    fn steer_newest_queued(&mut self, cx: &mut Context<Self>) {
        let newest = self.session.read(cx).queue().last().map(|queued| queued.id);
        if let Some(id) = newest {
            self.session
                .update(cx, |session, cx| session.steer_queued(id, cx));
        }
    }

    fn stop(&mut self, _: &Stop, _: &mut Window, cx: &mut Context<Self>) {
        self.session.update(cx, |session, cx| session.stop(cx));
    }

    // A tail-following list reports its anchor past the end, so clamp to the real maximum.
    fn scroll_top(&self) -> Pixels {
        (-self.list.scroll_px_offset_for_scrollbar().y).min(self.list.max_offset_for_scrollbar().y)
    }

    fn update_scroll_state(&mut self) {
        let distance = if self.list.is_following_tail() {
            0.
        } else {
            f32::from(self.list.max_offset_for_scrollbar().y - self.scroll_top()).max(0.)
        };
        if distance > SCROLL_BUTTON_THRESHOLD {
            self.show_scroll_button = true;
        } else if distance <= AT_BOTTOM {
            self.show_scroll_button = false;
        }
    }

    fn copy(&mut self, key: SharedString, text: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.show_done(key, COPIED_FEEDBACK, cx);
    }

    /// Scrolls to a saved message, looking it up by its text if the chat has changed since.
    pub(crate) fn reveal_message(
        &mut self,
        entry: usize,
        text: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        self.sync_transcript(cx);
        let count = self.entries(cx).len();
        let Some(target) = std::iter::once(entry)
            .chain(0..count)
            .find(|index| self.saved_text_at(*index, cx).as_deref() == Some(text))
        else {
            return false;
        };
        let Some(turn) = self.transcript.turn_for_entry(target) else {
            return false;
        };
        self.list.pause_following_tail();
        self.list.scroll_to(gpui::ListOffset {
            item_ix: self.transcript.turn_rows(turn).start,
            offset_in_item: px(0.),
        });
        if matches!(self.entries(cx).get(target), Some(Entry::User { .. })) {
            self.expanded_users.insert(target);
        }
        self.revealed_turn = Some(turn);
        self._revealed_reset = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(REVEALED_HIGHLIGHT).await;
            this.update(cx, |this, cx| {
                this.revealed_turn = None;
                cx.notify();
            })
            .ok();
        });
        cx.notify();
        true
    }

    fn saved_text_at(&self, entry: usize, cx: &App) -> Option<String> {
        let entries = self.entries(cx);
        match entries.get(entry)? {
            Entry::User { text, .. } => Some(text.to_string()),
            Entry::Assistant { .. } => {
                let turn = self
                    .transcript
                    .turns
                    .get(self.transcript.turn_for_entry(entry)?)?;
                (turn.first_reply == Some(entry))
                    .then(|| CopySource::Messages(turn.items.clone()).text(self, cx))
            }
            _ => None,
        }
    }

    fn show_done(&mut self, key: SharedString, duration: Duration, cx: &mut Context<Self>) {
        self.copied = Some(key.clone());
        self._copied_reset = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(duration).await;
            this.update(cx, |this, cx| {
                if this.copied.as_ref() == Some(&key) {
                    this.copied = None;
                    cx.notify();
                }
            })
            .ok();
        });
        cx.notify();
    }

    fn message_style(window: &Window, cx: &App) -> MarkdownStyle {
        let colors = cx.theme().colors();
        let accent = accent(cx);
        let mut style = MarkdownStyle::themed(MarkdownFont::Editor, window, cx);
        style.base_text_style.font_size = ui(14.).into();
        style.base_text_style.line_height = ui(22.).into();
        style.base_text_style.color = colors.text;
        style.paragraph_spacing = px(MD_BLOCK_GAP);
        style.soft_break_as_hard_break = true;
        style.list_spacing = px(MD_BLOCK_GAP);
        style.table_cell_padding = point(px(12.), px(12.));
        style.container_style.margin.bottom = Some(px(-MD_BLOCK_GAP).into());
        style.heading.margin.top = Some(px(0.).into());
        style.heading.margin.bottom = Some(px(MD_BLOCK_GAP).into());
        let is_light = cx.theme().appearance.is_light();
        style.inline_code = TextStyleRefinement {
            color: Some(accent),
            background_color: Some(accent.opacity(if is_light { 0.10 } else { 0.12 })),
            ..code_text_style(cx)
        };
        style.link = TextStyleRefinement {
            color: Some(colors.text),
            underline: Some(UnderlineStyle {
                thickness: px(1.),
                color: Some(colors.text_muted),
                wavy: false,
            }),
            ..Default::default()
        };
        let mut code_block = StyleRefinement::default();
        code_block.padding.left = Some(px(12.).into());
        code_block.padding.right = Some(px(12.).into());
        code_block.padding.top = Some(px(10.).into());
        code_block.padding.bottom = Some(px(10.).into());
        code_block.text = TextStyleRefinement {
            font_size: Some(px(12.5).into()),
            line_height: Some(px(18.).into()),
            ..code_text_style(cx)
        };
        style.code_block = code_block;
        let heading = |size: f32, line_height: f32| {
            Some(TextStyleRefinement {
                font_size: Some(ui(size).into()),
                line_height: Some(ui(line_height).into()),
                font_weight: Some(FontWeight::SEMIBOLD),
                color: Some(colors.text),
                ..Default::default()
            })
        };
        style.heading_level_styles = Some(HeadingLevelStyles {
            h1: heading(19., 27.),
            h2: heading(16., 24.),
            h3: heading(15., 22.),
            h4: heading(14., 22.),
            h5: heading(14., 22.),
            h6: heading(14., 22.),
        });
        style.rule_color = colors.border;
        style.block_quote_border_color = accent.opacity(0.6);
        style.block_quote_border_width = px(2.);
        style.block_quote = TextStyleRefinement {
            color: Some(colors.text_muted),
            ..Default::default()
        };
        style.selection_background_color = accent.opacity(if is_light { 0.24 } else { 0.35 });
        style
    }

    fn thought_style(window: &Window, cx: &App) -> MarkdownStyle {
        let faint = text_faint(cx);
        let mut style = Self::message_style(window, cx);
        style.base_text_style.font_size = ui(THOUGHT_BODY_SIZE).into();
        style.base_text_style.line_height = px(OUTPUT_LINE_HEIGHT).into();
        // Markdown body text inherits its size from the container, not from base_text_style.
        style.container_style.text.font_size = Some(ui(THOUGHT_BODY_SIZE).into());
        style.base_text_style.color = faint;
        style.paragraph_spacing = px(OUTPUT_LINE_HEIGHT);
        style.list_spacing = px(OUTPUT_LINE_HEIGHT);
        style.container_style.margin.bottom = Some(px(-OUTPUT_LINE_HEIGHT).into());
        style.heading.margin.bottom = Some(px(OUTPUT_LINE_HEIGHT).into());
        style.inline_code = TextStyleRefinement {
            color: Some(faint),
            ..code_text_style(cx)
        };
        style.link = TextStyleRefinement {
            color: Some(faint),
            underline: Some(UnderlineStyle {
                thickness: px(1.),
                color: Some(faint),
                wavy: false,
            }),
            ..Default::default()
        };
        let mut code_block = StyleRefinement::default();
        code_block.margin.bottom = Some(px(OUTPUT_LINE_HEIGHT).into());
        code_block.text = TextStyleRefinement {
            font_size: Some(ui(THOUGHT_CODE_SIZE).into()),
            line_height: Some(px(OUTPUT_LINE_HEIGHT).into()),
            color: Some(faint),
            ..code_text_style(cx)
        };
        style.code_block = code_block;
        let heading = || {
            Some(TextStyleRefinement {
                font_size: Some(ui(THOUGHT_BODY_SIZE).into()),
                line_height: Some(px(OUTPUT_LINE_HEIGHT).into()),
                font_weight: Some(FontWeight::SEMIBOLD),
                color: Some(faint),
                ..Default::default()
            })
        };
        style.heading_level_styles = Some(HeadingLevelStyles {
            h1: heading(),
            h2: heading(),
            h3: heading(),
            h4: heading(),
            h5: heading(),
            h6: heading(),
        });
        style.block_quote = TextStyleRefinement {
            color: Some(faint),
            ..Default::default()
        };
        style
    }

    fn table_actions(&self, message_key: usize, cx: &Context<Self>) -> TableActionsFn {
        let view: WeakEntity<Self> = cx.entity().downgrade();
        let copied = self.copied.clone();
        Arc::new(move |table, range, _, cx| {
            let key: SharedString = format!("table-{message_key}-{}", range.start).into();
            let is_copied = copied.as_ref() == Some(&key);
            let muted = cx.theme().colors().text_muted;
            let table = table.to_string();
            let view = view.clone();
            h_flex()
                .id(ElementId::Name(key.clone()))
                .absolute()
                .top(px(4.))
                .right(px(4.))
                .h(px(CODE_ACTION_SIZE))
                .px(px(6.))
                .gap(px(4.))
                .rounded(px(5.))
                .cursor_pointer()
                .text_size(px(10.5))
                .text_color(muted)
                .bg(cx.theme().colors().editor_background)
                .border_1()
                .border_color(cx.theme().colors().border)
                .when(!is_copied, |this| {
                    this.invisible()
                        .group_hover(TABLE_GROUP, |style| style.visible())
                })
                .hover(|style| style.bg(ink(0.08, cx)))
                .tooltip(Tooltip::text(t!("Copy table as Markdown")))
                .child(
                    icon(
                        if is_copied {
                            IconName::AgentCheck
                        } else {
                            IconName::AgentCopy
                        },
                        12.,
                    )
                    .text_color(muted),
                )
                .when(is_copied, |this| this.child(t!("Copied")))
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    let key = key.clone();
                    let table = table.clone();
                    view.update(cx, |this, cx| this.copy(key, table, cx)).ok();
                })
                .into_any_element()
        })
    }

    fn code_block_renderer(&self, message_key: usize, cx: &Context<Self>) -> CodeBlockRenderer {
        let view: WeakEntity<Self> = cx.entity().downgrade();
        let view_for_hide = view.clone();
        let copied = self.copied.clone();
        CodeBlockRenderer::Custom {
            render: Arc::new(move |kind, parsed, range, metadata, _, cx| {
                let colors = cx.theme().colors();
                let language: SharedString = match kind {
                    CodeBlockKind::FencedLang(language) => language.clone(),
                    CodeBlockKind::FencedSrc(path) => path
                        .path
                        .rsplit(['/', '\\'])
                        .next()
                        .map(|name| SharedString::from(name.to_string()))
                        .unwrap_or_default(),
                    _ => SharedString::default(),
                };
                let code = parsed
                    .source()
                    .get(metadata.content_range)
                    .unwrap_or_default()
                    .to_string();
                let key: SharedString = format!("code-{message_key}-{}", range.start).into();
                let is_copied = copied.as_ref() == Some(&key);
                let view = view.clone();
                let is_mermaid = language.eq_ignore_ascii_case("mermaid");
                let diagram = if is_mermaid {
                    view.upgrade().map(|chat| {
                        let shown_source =
                            chat.read(cx).diagram_sources_shown.contains(code.trim_end());
                        let diagram = chat.read(cx).diagrams.read(cx).get(&code).cloned();
                        (shown_source, diagram)
                    })
                } else {
                    None
                };
                let toggle_key = key.clone();
                let toggle_code = code.trim_end().to_string();
                let diagram_view = view.clone();
                let muted = colors.text_muted;
                let ready_diagram = diagram.as_ref().and_then(|(shown_source, diagram)| {
                    matches!(diagram, Some(Diagram::Ready { .. })).then_some(*shown_source)
                });
                let failure = diagram.as_ref().and_then(|(_, diagram)| match diagram {
                    Some(Diagram::Failed(error)) => {
                        Some(t!("Couldn't draw this diagram: {error}", error = error))
                    }
                    _ => None,
                });
                let warning = cx.theme().status().warning;
                div()
                    .w_full()
                    .min_w_0()
                    .mb(px(MD_BLOCK_GAP))
                    .flex()
                    .flex_col()
                    .rounded(px(10.))
                    .border_1()
                    .border_color(colors.border)
                    .bg(ink(0.035, cx))
                    .overflow_hidden()
                    .child(
                        h_flex()
                            .h(px(CODE_HEADER_HEIGHT))
                            .flex_none()
                            .pl(px(12.))
                            .pr(px(5.))
                            .justify_between()
                            .border_b_1()
                            .border_color(colors.border)
                            .bg(ink(0.02, cx))
                            .child(
                                div()
                                    .min_w_0()
                                    .text_size(px(11.))
                                    .text_color(muted)
                                    .child(language),
                            )
                            .child(
                                h_flex()
                                    .flex_none()
                                    .gap(px(2.))
                                    .when_some(failure, |this, failure| {
                                        this.child(
                                            div()
                                                .id(ElementId::Name(
                                                    format!("{toggle_key}-failure").into(),
                                                ))
                                                .size(px(CODE_ACTION_SIZE))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .tooltip(Tooltip::text(failure))
                                                .child(
                                                    icon(IconName::AgentDangerTriangle, 13.)
                                                        .text_color(warning),
                                                ),
                                        )
                                    })
                                    .when_some(ready_diagram, |this, shown_source| {
                                        let view = diagram_view.clone();
                                        this.child(
                                            div()
                                                .id(ElementId::Name(
                                                    format!("{toggle_key}-toggle").into(),
                                                ))
                                                .size(px(CODE_ACTION_SIZE))
                                                .rounded(px(6.))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .cursor_pointer()
                                                .hover(|style| style.bg(ink(0.08, cx)))
                                                .tooltip(Tooltip::text(if shown_source {
                                                    t!("Show diagram")
                                                } else {
                                                    t!("Show source")
                                                }))
                                                .child(
                                                    icon(
                                                        if shown_source {
                                                            IconName::Eye
                                                        } else {
                                                            IconName::FileCode
                                                        },
                                                        13.,
                                                    )
                                                    .text_color(muted),
                                                )
                                                .on_click(move |_, _, cx| {
                                                    cx.stop_propagation();
                                                    let code = toggle_code.clone();
                                                    view.update(cx, |this, cx| {
                                                        if !this.diagram_sources_shown.remove(&code)
                                                        {
                                                            this.diagram_sources_shown.insert(code);
                                                        }
                                                        cx.notify();
                                                    })
                                                    .ok();
                                                }),
                                        )
                                    })
                                    .child(
                                        h_flex()
                                            .id(ElementId::Name(key.clone()))
                                            .h(px(CODE_ACTION_SIZE))
                                            .px(px(6.))
                                            .gap(px(4.))
                                            .rounded(px(5.))
                                            .cursor_pointer()
                                            .text_size(px(10.5))
                                            .text_color(muted)
                                            .hover(|style| style.bg(ink(0.08, cx)))
                                            .tooltip(Tooltip::text(t!("Copy code")))
                                            .child(
                                                icon(
                                                    if is_copied {
                                                        IconName::AgentCheck
                                                    } else {
                                                        IconName::AgentCopy
                                                    },
                                                    12.,
                                                )
                                                .text_color(muted),
                                            )
                                            .when(is_copied, |this| this.child(t!("Copied")))
                                            .on_click(move |_, _, cx| {
                                                cx.stop_propagation();
                                                let key = key.clone();
                                                let code = code.clone();
                                                view.update(cx, |this, cx| this.copy(key, code, cx))
                                                    .ok();
                                            }),
                                    ),
                            ),
                    )
                    .when_some(
                        diagram.and_then(|(shown_source, diagram)| match diagram {
                            Some(diagram @ Diagram::Ready { .. }) if !shown_source => Some(diagram),
                            _ => None,
                        }),
                        |this, diagram| this.child(render_diagram(diagram, diagram_view.clone())),
                    )
            }),
            transform: None,
            hide_body: Some(Arc::new({
                let view = view_for_hide;
                move |kind, code, cx| {
                    let is_mermaid = matches!(kind, CodeBlockKind::FencedLang(language) if language.eq_ignore_ascii_case("mermaid"));
                    is_mermaid
                        && view.upgrade().is_some_and(|chat| {
                            let chat = chat.read(cx);
                            !chat.diagram_sources_shown.contains(code.trim_end())
                                && chat.diagrams.read(cx).is_ready(code)
                        })
                }
            })),
        }
    }

    fn row_flags(&self, cx: &App) -> RowFlags {
        RowFlags {
            working: self.is_working(cx),
            compact: AgentChatSettings::get_global(cx).compact_transcript,
            standalone: self.subagent.is_none(),
        }
    }

    fn sync_turns(&mut self, cx: &App) {
        let flags = self.row_flags(cx);
        let entries = self.entries(cx);
        let edits = self.transcript.sync(entries, flags, &self.expanded_work);
        self.apply_row_edits(edits);
    }

    fn rebuild_rows(&mut self, from_turn: usize, cx: &App) {
        let entries = self.entries(cx);
        let edits = self
            .transcript
            .rebuild_rows(from_turn, entries, &self.expanded_work);
        self.apply_row_edits(edits);
    }

    fn apply_row_edits(&self, edits: RowEdits) {
        if let Some((range, count)) = edits.splice {
            self.list.splice(range, count);
        }
        // A list following its tail never uses the scroll anchor a remeasure leaves behind.
        if self.list.is_following_tail() {
            return;
        }
        for row in edits.remeasure {
            self.list.remeasure_items(row..row + 1);
        }
    }

    fn sync_transcript(&mut self, cx: &mut Context<Self>) {
        self.sync_turns(cx);
        self.scan_new_entries(cx);
    }

    fn scan_new_entries(&mut self, cx: &mut Context<Self>) {
        let entries = self.entries(cx);
        if self.scanned_regroups != self.transcript.regroups || entries.len() < self.scanned_entries
        {
            self.scanned_regroups = self.transcript.regroups;
            self.scanned_entries = 0;
            self.scanned_tail = None;
            self.latest_todo = None;
        }
        let cwd = &self.session.read(cx).metadata().cwd;
        let wants_links = self.has_rendered;
        let wants_diagrams = self.diagram_theme.is_some();
        let mut paths = Vec::new();
        let mut diagrams = Vec::new();
        let mut tail = None;
        let first = self.scanned_entries.saturating_sub(1);
        for (index, entry) in entries.iter().enumerate().skip(first) {
            tail = None;
            match entry {
                Entry::Tool(tool) if matches!(tool.call, ToolCall::Todo { .. }) => {
                    self.latest_todo = Some(index);
                }
                Entry::Assistant { markdown, .. } => {
                    let source = markdown.read(cx).source();
                    tail = Some((markdown.entity_id(), source.len()));
                    if self.scanned_tail == tail {
                        continue;
                    }
                    if wants_links {
                        links::path_candidates(source, cwd, &mut paths);
                    }
                    if wants_diagrams && source.contains("```mermaid") {
                        diagrams.extend(mermaid::mermaid_sources(source));
                    }
                }
                _ => {}
            }
        }
        self.scanned_tail = tail;
        self.scanned_entries = entries.len();
        self.check_paths(paths, cx);
        self.request_diagrams(diagrams, None, cx);
    }

    fn message_style_for_frame(&mut self, window: &Window, cx: &App) -> MarkdownStyle {
        self.frame_message_style
            .get_or_insert_with(|| Self::message_style(window, cx))
            .clone()
    }

    fn set_row_hovered(&mut self, row: Row, hovered: bool, cx: &mut Context<Self>) {
        let turn_before = self.hovered_row.map(|row| row.turn);
        if hovered {
            self.hovered_row = Some(row);
        } else if self.hovered_row == Some(row) {
            self.hovered_row = None;
        }
        if self.hovered_row.map(|row| row.turn) != turn_before {
            cx.notify();
        }
    }

    fn render_row(
        &mut self,
        row_index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(row) = self.transcript.rows.get(row_index).copied() else {
            return div().into_any_element();
        };
        let Some(turn) = self.transcript.turns.get(row.turn).cloned() else {
            return div().into_any_element();
        };
        let is_first_turn = row.turn == 0;
        let is_last_turn = row.turn + 1 == self.transcript.turns.len();
        let previous = row_index
            .checked_sub(1)
            .and_then(|previous| self.transcript.rows.get(previous))
            .filter(|previous| previous.turn == row.turn)
            .copied();
        let is_last_in_turn = self
            .transcript
            .rows
            .get(row_index + 1)
            .is_none_or(|next| next.turn != row.turn);
        let top_pad = match previous {
            Some(_) => 0.,
            None if is_first_turn => self.first_row_inset(),
            None => SPACE_LG,
        };
        let bottom_pad = if is_last_turn && is_last_in_turn {
            TRANSCRIPT_BOTTOM_PAD
        } else {
            0.
        };
        let gap = match (
            previous.and_then(|row| row.kind.spacing()),
            row.kind.spacing(),
        ) {
            (Some(previous), Some(current)) => top_gap(previous, current),
            (None, Some(_)) if is_first_turn && turn.user.is_none() => 0.,
            (None, Some(_)) => SPACE_LG,
            (_, None) => 0.,
        };
        let content = match row.kind {
            RowKind::User(index) => {
                let user_text = self.user_markdown(index, cx);
                let user_style = Self::user_style(self.message_style_for_frame(window, cx));
                self.render_user_message(index, user_text, user_style, cx)
            }
            RowKind::Worked => self.render_worked_header(row.turn, &turn, cx),
            RowKind::ToolGroup { start, end } => {
                let last_entry = self.entries(cx).len().saturating_sub(1);
                let is_live = self.is_working(cx) && (start..end).contains(&last_entry);
                self.render_tool_group(start..end, is_live, window, cx)
            }
            RowKind::Message(index) => {
                let style = self.message_style_for_frame(window, cx);
                self.render_reply(index, style, cx)
            }
            RowKind::Notice(index) => self.render_notice(index, cx),
            RowKind::Image(index) => self.render_generated_image(index, cx),
            RowKind::Actions => self.render_turn_actions(row.turn, &turn, cx),
            RowKind::Working => self.render_working_trailer(cx),
            RowKind::Retry(index) => self.render_retry(index, cx),
        };
        let content = match row.kind.hover_id(row.turn) {
            Some(id) => {
                let hovered_row = row.identity();
                let wrapper = if row.kind.spacing().is_some() {
                    div()
                } else {
                    v_flex()
                };
                wrapper
                    .id(id)
                    .w_full()
                    .pt(px(gap))
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        this.set_row_hovered(hovered_row, *hovered, cx)
                    }))
                    .child(content)
                    .into_any_element()
            }
            None => content,
        };
        let painted_rows = self.painted_rows.clone();
        let painted_row = row.identity();
        let row_element = v_flex()
            .relative()
            .w_full()
            .when(self.revealed_turn == Some(row.turn), |this| {
                let top = if previous.is_none() {
                    top_pad - REVEALED_HIGHLIGHT_PAD
                } else {
                    0.
                };
                let bottom = if is_last_in_turn {
                    bottom_pad - REVEALED_HIGHLIGHT_PAD
                } else {
                    0.
                };
                this.child(
                    div()
                        .absolute()
                        .top(px(top))
                        .bottom(px(bottom))
                        .left(px(-REVEALED_HIGHLIGHT_PAD))
                        .right(px(-REVEALED_HIGHLIGHT_PAD))
                        .when(previous.is_none(), |this| this.rounded_t(px(12.)))
                        .when(is_last_in_turn, |this| this.rounded_b(px(12.)))
                        .bg(accent(cx).opacity(0.08)),
                )
            })
            .child(
                canvas(
                    move |bounds, _, _| {
                        painted_rows.borrow_mut().insert(painted_row, bounds);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .pt(px(top_pad))
            .pb(px(bottom_pad))
            .child(content);
        column(row_element, self.gutter()).into_any_element()
    }

    fn hover_strip(
        &self,
        reveal: StripReveal,
        at: i64,
        copy: Option<(SharedString, CopySource)>,
        save: Option<SaveTarget>,
        editable: bool,
        align_end: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let timestamp = chrono::DateTime::from_timestamp(at, 0)
            .filter(|_| at > 0)
            .map(|time| {
                time.with_timezone(&chrono::Local)
                    .format(&t!("%b %-d, %-I:%M %p"))
                    .to_string()
            });
        let muted = cx.theme().colors().text_muted;
        h_flex()
            .h(px(SPACE_SM + SPACE_MD * 2.))
            .pt(px(SPACE_SM))
            .w_full()
            .when(align_end, |this| this.justify_end())
            .gap(px(SPACE_SM))
            .child(
                reveal
                    .apply(h_flex().gap(px(SPACE_SM)))
                    .when_some(timestamp, |this, timestamp| {
                        this.child(
                            div()
                                .text_size(ui(12.))
                                .text_color(muted.opacity(0.55))
                                .child(timestamp),
                        )
                    })
                    .when(editable, |this| {
                        this.child(
                            div()
                                .id("edit-last-message")
                                .size(px(SPACE_MD * 2.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(6.))
                                .cursor_pointer()
                                .hover(|style| style.bg(ink(0.08, cx)))
                                .tooltip(Tooltip::text(t!("Edit message")))
                                .child(icon(IconName::AgentPen, 14.).text_color(muted))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    cx.stop_propagation();
                                    this.edit_last_message(window, cx)
                                })),
                        )
                    })
                    .when_some(copy, |this, (copy_key, copy_source)| {
                        let is_copied = self.copied.as_ref() == Some(&copy_key);
                        this.child(
                            div()
                                .id(ElementId::Name(copy_key.clone()))
                                .size(px(SPACE_MD * 2.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(6.))
                                .cursor_pointer()
                                .hover(|style| style.bg(ink(0.08, cx)))
                                .tooltip(Tooltip::text(t!("Copy message")))
                                .child(
                                    icon(
                                        if is_copied {
                                            IconName::AgentCheck
                                        } else {
                                            IconName::AgentCopy
                                        },
                                        14.,
                                    )
                                    .text_color(muted),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    let text = copy_source.text(this, cx);
                                    this.copy(copy_key.clone(), text, cx)
                                })),
                        )
                    }),
            )
            .when_some(save, |this, target| {
                let chat_id = self.session.read(cx).metadata().id.clone();
                let saved = self
                    .store
                    .read(cx)
                    .saved_text(&chat_id, target.entry)
                    .is_some_and(|saved| saved == target.source.text(self, cx));
                this.child(
                    reveal
                        .apply(
                            div()
                                .id(("save-message", target.entry))
                                .size(px(SPACE_MD * 2.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(6.))
                                .cursor_pointer(),
                        )
                        .hover(|style| style.bg(ink(0.08, cx)))
                        .tooltip(Tooltip::text(if saved {
                            tc!("chat", "Remove from saved")
                        } else {
                            tc!("chat", "Save message")
                        }))
                        .child(
                            icon(
                                if saved {
                                    IconName::AgentBookmarkFilled
                                } else {
                                    IconName::AgentBookmark
                                },
                                14.,
                            )
                            .text_color(if saved {
                                accent(cx)
                            } else {
                                muted
                            }),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            let text = target.source.text(this, cx);
                            this.store.update(cx, |store, cx| {
                                store.toggle_saved_message(
                                    &chat_id,
                                    target.entry,
                                    text,
                                    target.from_agent,
                                    cx,
                                )
                            });
                        })),
                )
            })
    }

    fn user_markdown(&mut self, index: usize, cx: &mut Context<Self>) -> Option<Entity<Markdown>> {
        let Some(Entry::User { text, .. }) = self.entries(cx).get(index) else {
            return None;
        };
        let text = text.clone();
        if let Some(markdown) = self.user_markdown.get(&index)
            && markdown.read(cx).source() == text.as_ref()
        {
            return Some(markdown.clone());
        }
        let markdown = cx.new(|cx| {
            Markdown::new_with_options(
                text,
                None,
                None,
                MarkdownOptions {
                    parse_links_only: true,
                    parse_block_quotes: true,
                    ..Default::default()
                },
                cx,
            )
        });
        self.highlight_new_message(index, &markdown, cx);
        self.user_markdown.insert(index, markdown.clone());
        Some(markdown)
    }

    fn user_style(mut style: MarkdownStyle) -> MarkdownStyle {
        style.base_text_style.line_height = ui(USER_LINE_HEIGHT).into();
        style.container_style.text.font_size = Some(ui(14.).into());
        style.container_style.text.line_height = Some(ui(USER_LINE_HEIGHT).into());
        style.container_style.margin.bottom = None;
        style.paragraph_spacing = px(0.);
        style
    }

    fn render_user_message(
        &self,
        index: usize,
        markdown: Option<Entity<Markdown>>,
        style: MarkdownStyle,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(Entry::User {
            text,
            at,
            attachments,
            ..
        }) = self.entries(cx).get(index)
        else {
            return div().into_any_element();
        };
        let text = text.clone();
        let attachments = attachments.clone();
        let editable =
            self.subagent.is_none() && self.session.read(cx).editable_message() == Some(index);
        let line_count = text.lines().count();
        let collapsible =
            line_count > USER_COLLAPSED_LINES || text.chars().count() > USER_COLLAPSE_CHARS;
        let expanded = self.expanded_users.contains(&index);
        let collapsed = collapsible && !expanded;
        let group: SharedString = format!("agent-user-{index}").into();
        let toggle_group: SharedString = format!("agent-user-toggle-{index}").into();
        let colors = cx.theme().colors();
        let bubble_background = if cx.theme().appearance.is_light() {
            wash(0.04, cx)
        } else {
            wash(0.08, cx)
        };
        let user_text = markdown.map(|markdown| MarkdownElement::new(markdown, style));
        let copy = (!text.trim().is_empty()).then(|| {
            (
                SharedString::from(format!("copy-user-{index}")),
                CopySource::Text(text.clone()),
            )
        });
        let save = (self.subagent.is_none() && !text.trim().is_empty()).then(|| SaveTarget {
            entry: index,
            source: CopySource::Text(text.clone()),
            from_agent: false,
        });
        v_flex()
            .group(group.clone())
            .w_full()
            .items_end()
            .when(!attachments.is_empty(), |this| {
                this.child(
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .flex_none()
                        .flex_wrap()
                        .justify_end()
                        .items_start()
                        .gap(px(8.))
                        .px(px(4.))
                        .pt(px(4.))
                        .pb(px(6.))
                        .children(attachments.into_iter().enumerate().map(|(position, path)| {
                            div()
                                .id(SharedString::from(format!(
                                    "agent-sent-image-{index}-{position}"
                                )))
                                .flex_none()
                                .w(px(ATTACHMENT_THUMB_WIDTH))
                                .h(px(ATTACHMENT_THUMB_HEIGHT))
                                .rounded(px(8.))
                                .overflow_hidden()
                                .border_1()
                                .border_color(hairline(0.11, cx))
                                .bg(ink(0.035, cx))
                                .cursor_pointer()
                                .child(
                                    img(path.clone())
                                        .image_cache(&self.images)
                                        .w(px(ATTACHMENT_THUMB_WIDTH - 2.))
                                        .h(px(ATTACHMENT_THUMB_HEIGHT - 2.))
                                        .rounded(px(7.))
                                        .object_fit(ObjectFit::Cover),
                                )
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.open_image(path.clone(), window, cx)
                                }))
                        })),
                )
            })
            .child(
                v_flex()
                    .when(text.trim().is_empty(), |this| this.hidden())
                    .min_w_0()
                    .max_w(px(CONTENT_WIDTH * 0.8))
                    .px(px(16.))
                    .py(px(10.))
                    .rounded(px(16.))
                    .bg(bubble_background)
                    .text_size(ui(14.))
                    .line_height(ui(USER_LINE_HEIGHT))
                    .text_color(colors.text)
                    .map(|this| {
                        if collapsed {
                            this.child(
                                div()
                                    .max_h(ui(USER_LINE_HEIGHT * USER_COLLAPSED_LINES as f32))
                                    .overflow_hidden()
                                    .children(user_text),
                            )
                            .child(div().h(ui(USER_LINE_HEIGHT)).child("..."))
                        } else {
                            this.children(user_text)
                        }
                    })
                    .when(collapsible, |this| {
                        this.child(
                            div().mt(px(USER_TOGGLE_GAP)).flex().items_start().child(
                                h_flex()
                                    .id(("agent-user-expand", index))
                                    .group(toggle_group.clone())
                                    .gap(px(5.))
                                    .text_size(ui(14.))
                                    .line_height(ui(USER_LINE_HEIGHT))
                                    .text_color(colors.text_muted)
                                    .cursor_pointer()
                                    .hover(|style| style.text_color(colors.text))
                                    .child(if expanded {
                                        t!("Show less")
                                    } else {
                                        t!("Show more")
                                    })
                                    .child(
                                        icon(
                                            if expanded {
                                                IconName::AgentArrowUp
                                            } else {
                                                IconName::AgentArrowDown
                                            },
                                            12.,
                                        )
                                        .text_color(colors.text_muted)
                                        .group_hover(toggle_group, |style| {
                                            style.text_color(colors.text)
                                        }),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if !this.expanded_users.remove(&index) {
                                            this.expanded_users.insert(index);
                                        }
                                        cx.notify();
                                    })),
                            ),
                        )
                    }),
            )
            .child(self.hover_strip(
                StripReveal::OnGroupHover(group),
                *at,
                copy,
                save,
                editable,
                true,
                cx,
            ))
            .into_any_element()
    }

    fn render_retry(&self, index: usize, cx: &Context<Self>) -> AnyElement {
        h_flex()
            .id(("agent-retry", index))
            .gap(px(SPACE_SM))
            .pt(px(SPACE_LG))
            .text_size(ui(12.))
            .text_color(cx.theme().status().error)
            .cursor_pointer()
            .child(t!("Not delivered. Click to retry."))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.session
                    .update(cx, |session, cx| session.retry_undelivered(index, cx))
            }))
            .into_any_element()
    }

    fn group_header(
        &self,
        id: ElementId,
        open: bool,
        title: AnyElement,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let colors = cx.theme().colors();
        let muted = colors.text_muted;
        let text = colors.text;
        let rotation = if open {
            0.
        } else {
            -std::f32::consts::FRAC_PI_2
        };
        h_flex()
            .id(id)
            .relative()
            .gap(px(GROUP_HEADER_GAP))
            .pr(px(4.))
            .h(px(TOOL_GROUP_HEADER_HEIGHT))
            .cursor_pointer()
            .text_size(px(TOOL_TEXT_SIZE))
            .line_height(px(TOOL_LINE_HEIGHT))
            .text_color(muted)
            .hover(|style| style.text_color(text))
            .child(
                div()
                    .w(px(GROUP_CHEVRON_WIDTH))
                    .h(px(TOOL_LINE_HEIGHT))
                    .flex_none()
                    .relative()
                    .child(
                        icon(IconName::AgentArrowDown, 14.)
                            .absolute()
                            .left(px(TREE_TRUNK_X - 7.))
                            .top(px(2.))
                            .with_transformation(Transformation::rotate(radians(rotation)))
                            .text_color(muted),
                    ),
            )
            .child(
                div()
                    .min_w_0()
                    .h(px(TOOL_LINE_HEIGHT))
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .child(title),
            )
    }

    fn render_worked_header(
        &self,
        turn_index: usize,
        turn: &Turn,
        cx: &Context<Self>,
    ) -> AnyElement {
        let entries = self.entries(cx);
        let started = turn.user.and_then(|index| match entries.get(index) {
            Some(Entry::User { at, .. }) => Some(*at),
            _ => None,
        });
        let finished = turn.last_reply.and_then(|index| match entries.get(index) {
            Some(Entry::Assistant { at, .. }) => Some(*at),
            _ => None,
        });
        let label = match (started, finished) {
            (Some(started), Some(finished)) if finished > started => t!(
                "Worked for {duration}",
                duration = format_elapsed((finished - started) as u64)
            )
            .to_string(),
            _ => t!("Worked").to_string(),
        };
        let open = self.expanded_work.contains(&turn_index);
        self.group_header(
            ("agent-worked", turn_index).into(),
            open,
            SharedString::from(label).into_any_element(),
            cx,
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            if !this.expanded_work.remove(&turn_index) {
                this.expanded_work.insert(turn_index);
            }
            this.rebuild_rows(turn_index, cx);
            cx.notify();
        }))
        .into_any_element()
    }

    fn render_reply(&self, index: usize, style: MarkdownStyle, cx: &Context<Self>) -> AnyElement {
        let Some(Entry::Assistant { markdown, .. }) = self.entries(cx).get(index) else {
            return div().into_any_element();
        };
        let view = cx.entity().downgrade();
        let link_view = view.clone();
        let cwd = self.session.read(cx).metadata().cwd.clone();
        let menu_markdown = markdown.clone();
        div()
            .w_full()
            .on_mouse_down(
                gpui::MouseButton::Right,
                cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                    if let Some(url) = menu_markdown.read(cx).context_menu_link().cloned() {
                        this.show_link_menu(url, event.position, window, cx);
                        cx.stop_propagation();
                    }
                }),
            )
            .with_animation(
                ("agent-reply-fade", index),
                Animation::new(FADE_IN).with_easing(ease_out_quint()),
                |this, delta| this.opacity(delta),
            )
            .child(
                MarkdownElement::new(markdown.clone(), style)
                    .code_block_renderer(self.code_block_renderer(index, cx))
                    .table_actions(self.table_actions(index, cx))
                    .on_url_click(move |url, window, cx| {
                        link_view
                            .update(cx, |this, cx| this.open_link(url, window, cx))
                            .ok();
                    })
                    .on_code_span_link(move |text, cx| {
                        view.upgrade()?.read(cx).code_span_url(text, &cwd)
                    }),
            )
            .into_any_element()
    }

    fn render_notice(&self, index: usize, cx: &Context<Self>) -> AnyElement {
        let Some(Entry::Notice { text, is_error }) = self.entries(cx).get(index) else {
            return div().into_any_element();
        };
        let text = crate::session::translated_notice(text);
        if *is_error {
            self.render_error(index, text, cx)
        } else {
            div()
                .text_size(ui(12.))
                .text_color(cx.theme().colors().text_muted)
                .child(text)
                .into_any_element()
        }
    }

    fn render_generated_image(&self, index: usize, cx: &Context<Self>) -> AnyElement {
        let Some(Entry::Image { path }) = self.entries(cx).get(index) else {
            return div().into_any_element();
        };
        let path = path.clone();
        let layout = self.generated_image_layout(&path);
        let group = SharedString::from(format!("agent-generated-image-{index}"));
        let download = {
            let image = image_viewer::ChatImage::File(path.clone());
            let downloaded = self.copied.as_ref() == Some(&image.download_key());
            div()
                .id(("agent-generated-image-download", index))
                .absolute()
                .top(px(8.))
                .right(px(8.))
                .size(px(28.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(gpui::hsla(0., 0., 0., 0.55))
                .cursor_pointer()
                .hover(|style| style.bg(gpui::hsla(0., 0., 0., 0.75)))
                .when(!downloaded, |this| this.visible_on_hover(group.clone()))
                .tooltip(Tooltip::text(if downloaded {
                    t!("Saved to Downloads")
                } else {
                    t!("Download")
                }))
                .child(
                    ui::Icon::new(if downloaded {
                        IconName::Check
                    } else {
                        IconName::Download
                    })
                    .size(ui::IconSize::Small)
                    .color(Color::Custom(gpui::white())),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.download_image(image.clone(), window, cx)
                }))
        };
        div()
            .id(("agent-generated-image", index))
            .group(group)
            .relative()
            .max_w(px(GENERATED_IMAGE_MAX_WIDTH))
            .max_h(px(GENERATED_IMAGE_MAX_HEIGHT))
            .when_some(layout, |this, (width, aspect_ratio)| {
                this.w_full().max_w(width).aspect_ratio(aspect_ratio)
            })
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(12.))
            .overflow_hidden()
            .bg(ink(0.045, cx))
            .cursor_pointer()
            .child(
                img(path.clone())
                    .image_cache(&self.images)
                    .map(|image| match layout {
                        Some(_) => image.size_full(),
                        None => image
                            .max_w(px(GENERATED_IMAGE_MAX_WIDTH))
                            .max_h(px(GENERATED_IMAGE_MAX_HEIGHT)),
                    })
                    .rounded(px(12.))
                    .object_fit(ObjectFit::Contain),
            )
            .child(download)
            .on_click(
                cx.listener(move |this, _, window, cx| this.open_image(path.clone(), window, cx)),
            )
            .into_any_element()
    }

    fn render_turn_actions(
        &self,
        turn_index: usize,
        turn: &Turn,
        cx: &Context<Self>,
    ) -> AnyElement {
        let last_at = turn
            .last_reply
            .and_then(|index| match self.entries(cx).get(index) {
                Some(Entry::Assistant { at, .. }) => Some(*at),
                _ => None,
            })
            .unwrap_or_default();
        let save = turn
            .first_reply
            .filter(|_| self.subagent.is_none())
            .map(|entry| SaveTarget {
                entry,
                source: CopySource::Messages(turn.items.clone()),
                from_agent: true,
            });
        let copy = turn.first_reply.map(|_| {
            (
                SharedString::from(format!("copy-turn-{turn_index}")),
                CopySource::Messages(turn.items.clone()),
            )
        });
        let hovered = self.hovered_row.is_some_and(|row| row.turn == turn_index);
        self.hover_strip(
            StripReveal::When(hovered),
            last_at,
            copy,
            save,
            false,
            false,
            cx,
        )
        .into_any_element()
    }

    fn render_working_trailer(&self, cx: &Context<Self>) -> AnyElement {
        let elapsed = self
            .working_since(cx)
            .map(|since| since.elapsed().as_secs())
            .unwrap_or(0);
        let label = if self.subagent.is_none() && self.session.read(cx).is_compacting() {
            t!("Compacting conversation…").to_string()
        } else {
            format!(
                "{}…",
                i18n::translate(flavour_word(self.flavour_seed, elapsed))
            )
        };
        let colors = cx.theme().colors();
        h_flex()
            .gap(px(SPACE_SM))
            .pt(px(SPACE_LG))
            .text_size(ui(11.))
            .child(gradient_spinner(
                format!("agent-working-{}", cx.entity_id()).into(),
                14.,
            ))
            .child(
                div()
                    .text_size(ui(12.))
                    .text_color(colors.text_muted)
                    .child(label),
            )
            .child(
                div()
                    .relative()
                    .top(px(1.))
                    .text_color(text_faint(cx))
                    .child(format_elapsed(elapsed)),
            )
            .into_any_element()
    }

    fn render_error(&self, index: usize, text: SharedString, cx: &Context<Self>) -> AnyElement {
        let danger = cx.theme().status().error;
        let danger_muted = mix(danger, cx.theme().colors().text, 0.28).opacity(0.8);
        let copy_key: SharedString = format!("copy-error-{index}").into();
        let is_copied = self.copied.as_ref() == Some(&copy_key);
        div()
            .py(px(4.))
            .w_full()
            .child(
                v_flex()
                    .w_full()
                    .overflow_hidden()
                    .gap(px(6.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(danger.opacity(0.16))
                    .bg(danger.opacity(0.05))
                    .px(px(10.))
                    .py(px(8.))
                    .text_size(px(12.))
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .child(
                                div()
                                    .flex_none()
                                    .size(px(20.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(6.))
                                    .bg(danger.opacity(0.12))
                                    .child(
                                        icon(IconName::AgentDangerTriangle, 12.)
                                            .text_color(danger_muted),
                                    ),
                            )
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(danger_muted)
                                    .child(t!("Error")),
                            )
                            .child(div().flex_1())
                            .child(
                                div()
                                    .id(ElementId::Name(copy_key.clone()))
                                    .flex_none()
                                    .size(px(20.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(6.))
                                    .cursor_pointer()
                                    .hover(|style| style.bg(danger.opacity(0.12)))
                                    .tooltip(Tooltip::text(t!("Copy message")))
                                    .child(
                                        icon(
                                            if is_copied {
                                                IconName::AgentCheck
                                            } else {
                                                IconName::AgentCopy
                                            },
                                            12.,
                                        )
                                        .text_color(danger_muted),
                                    )
                                    .on_click(cx.listener({
                                        let text = text.to_string();
                                        move |this, _, _, cx| {
                                            cx.stop_propagation();
                                            this.copy(copy_key.clone(), text.clone(), cx)
                                        }
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .w_full()
                            .text_color(cx.theme().colors().text.opacity(0.8))
                            .child(text),
                    ),
            )
            .into_any_element()
    }

    fn group_summary(&self, items: Range<usize>, cx: &App) -> String {
        let mut tools = Vec::new();
        let mut thoughts = 0;
        for entry in self.entries(cx).get(items).unwrap_or_default() {
            match entry {
                Entry::Tool(tool) => tools.push((&tool.call, tool.status == ToolStatus::Failed)),
                Entry::Thinking(_) => thoughts += 1,
                _ => {}
            }
        }
        let mut segments: Vec<String> = Vec::new();
        match thoughts {
            0 => {}
            1 => segments.push(t!("thought process").to_string()),
            count => segments.push(t!("thought {count} times", count = count).to_string()),
        }
        if !tools.is_empty() {
            segments.push(view::tool_group_summary(&tools));
        }
        let mut summary = segments.join(" · ");
        if let Some(first) = summary.get_mut(..1) {
            first.make_ascii_uppercase();
        }
        summary
    }

    fn render_tool_group(
        &self,
        items: Range<usize>,
        is_live: bool,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let entries = self.entries(cx);
        let group = entries.get(items.clone()).unwrap_or_default();
        let collapses = group.iter().any(|entry| match entry {
            Entry::Tool(tool) => !tool.call.is_subagent_spawn(),
            _ => true,
        });
        if !collapses {
            return v_flex()
                .w_full()
                .pt(px(CHIPS_TOP_PAD))
                .children(items.map(|index| self.render_subagent_chip(index, cx)))
                .into_any_element();
        }
        let group_key = match group.first() {
            Some(Entry::Tool(tool)) => format!("tools-{}", tool.id),
            _ => format!("tools-at-{}", items.start),
        };
        let open = self
            .group_overrides
            .get(&group_key)
            .copied()
            .unwrap_or(is_live);
        let summary: SharedString = self.group_summary(items.clone(), cx).into();
        let colors = cx.theme().colors();
        let title = if is_live {
            shimmer_title(
                summary,
                ElementId::Name(format!("{group_key}-shimmer").into()),
                colors.text_muted,
                colors.text,
            )
        } else {
            summary.into_any_element()
        };
        let header = self
            .group_header(
                ElementId::Name(format!("{group_key}-header").into()),
                open,
                title,
                cx,
            )
            .on_click(cx.listener({
                move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.group_overrides.insert(group_key.clone(), !open);
                    cx.notify();
                }
            }));

        if let [Entry::Thinking(markdown)] = group {
            return v_flex()
                .w_full()
                .child(header)
                .when(open, |this| {
                    this.child(
                        div()
                            .pl(px(GROUP_CHEVRON_WIDTH + GROUP_HEADER_GAP))
                            .pb(px(6.))
                            .child(
                                MarkdownElement::new(
                                    markdown.clone(),
                                    Self::thought_style(window, cx),
                                )
                                .code_block_renderer(plain_code_block_renderer()),
                            ),
                    )
                })
                .into_any_element();
        }
        let last = items.end.saturating_sub(1);
        v_flex()
            .w_full()
            .child(header)
            .when(open, |this| {
                this.child(v_flex().pt(px(CHIPS_TOP_PAD)).children(
                    items.map(|index| self.render_tree_row(index, index < last, window, cx)),
                ))
            })
            .into_any_element()
    }

    fn render_tree_row(
        &self,
        index: usize,
        continues: bool,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(entry) = self.entries(cx).get(index) else {
            return div().into_any_element();
        };
        let colors = cx.theme().colors();
        let danger = cx.theme().status().error;
        let (key, icon_name, label, detail, failed): (String, IconName, SharedString, String, bool) =
            match entry {
                Entry::Tool(tool) => {
                    let (label, detail) = view::tool_chip_content(&tool.call);
                    (
                        format!("tool-{}", tool.id),
                        tool_icon(&tool.call),
                        label,
                        detail,
                        tool.status == ToolStatus::Failed,
                    )
                }
                Entry::Thinking(_) => (
                    format!("thought-{index}"),
                    IconName::AgentChatRoundLine,
                    t!("Thought process"),
                    String::new(),
                    false,
                ),
                _ => return div().into_any_element(),
            };
        let stats = match entry {
            Entry::Tool(tool) => tool
                .diff
                .as_ref()
                .map(|diff| self.diff_view(&tool.id, diff, cx))
                .map(|view| (view.additions, view.deletions)),
            _ => None,
        };
        let expanded = self.expanded_rows.contains(&key);
        let body = expanded.then(|| match entry {
            Entry::Tool(tool) => self.render_tool_detail(tool, cx),
            Entry::Thinking(markdown) => v_flex()
                .w_full()
                .min_w_0()
                .child(detail_separator())
                .child(
                    div().py(px(6.)).child(
                        MarkdownElement::new(markdown.clone(), Self::thought_style(window, cx))
                            .code_block_renderer(plain_code_block_renderer()),
                    ),
                )
                .into_any_element(),
            _ => div().into_any_element(),
        });
        let tint = if failed { danger } else { colors.text_muted };
        let text = colors.text;
        let hover_group: SharedString = format!("{key}-header").into();
        let hover_text = !failed;
        let header_row = h_flex()
            .group(hover_group.clone())
            .h(px(CHIP_CARD_HEIGHT))
            .w_full()
            .min_w_0()
            .gap(px(8.))
            .text_size(px(TOOL_TEXT_SIZE))
            .line_height(px(TOOL_LINE_HEIGHT))
            .child(
                div()
                    .id(ElementId::Name(format!("{key}-label").into()))
                    .flex_none()
                    .h(px(TOOL_LINE_HEIGHT))
                    .flex()
                    .items_center()
                    .text_color(tint)
                    .when(hover_text, |this| {
                        this.group_hover(hover_group.clone(), |style| style.text_color(text))
                    })
                    .child(label),
            )
            .child(
                div()
                    .id(ElementId::Name(format!("{key}-detail").into()))
                    .min_w_0()
                    .h(px(TOOL_LINE_HEIGHT))
                    .flex()
                    .items_center()
                    .when(detail.is_empty(), |this| this.hidden())
                    .truncate()
                    .text_color(tint)
                    .when(hover_text, |this| {
                        this.group_hover(hover_group.clone(), |style| style.text_color(text))
                    })
                    .child(div().min_w_0().truncate().child(detail)),
            )
            .when_some(stats, |this, (additions, deletions)| {
                let status = cx.theme().status();
                this.child(
                    h_flex()
                        .flex_none()
                        .gap(px(4.))
                        .font(code_font(cx))
                        .text_size(px(11.))
                        .when(additions > 0, |this| {
                            this.child(
                                div()
                                    .text_color(status.created)
                                    .child(format!("+{additions}")),
                            )
                        })
                        .when(deletions > 0, |this| {
                            this.child(
                                div()
                                    .text_color(status.deleted)
                                    .child(format!("−{deletions}")),
                            )
                        }),
                )
            })
            .child(
                div()
                    .size(px(18.))
                    .flex_none()
                    .opacity(0.)
                    .group_hover(hover_group.clone(), |style| style.opacity(1.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        icon(
                            if expanded {
                                IconName::AgentArrowDown
                            } else {
                                IconName::AgentArrowRight
                            },
                            12.,
                        )
                        .text_color(text_faint(cx))
                        .group_hover(hover_group, move |style| {
                            style.text_color(if failed { danger } else { text })
                        }),
                    ),
            );
        let card = v_flex()
            .ml(px(TREE_TEXT_GAP))
            .my(px((TREE_ROW_HEIGHT - CHIP_CARD_HEIGHT) / 2.))
            .min_w_0()
            .flex_1()
            .overflow_hidden()
            .child(
                div()
                    .id(ElementId::Name(key.clone().into()))
                    .h(px(CHIP_CARD_HEIGHT))
                    .flex_none()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .child(header_row)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        if !this.expanded_rows.remove(&key) {
                            this.expanded_rows.insert(key.clone());
                        }
                        cx.notify();
                    })),
            )
            .children(body);
        div()
            .w_full()
            .flex_none()
            .flex()
            .flex_row()
            .with_animation(
                ("agent-tree-row-fade", index),
                Animation::new(FADE_IN).with_easing(ease_out_quint()),
                |this, delta| this.opacity(delta),
            )
            .child(activity_rail(
                icon_name,
                tint,
                continues,
                hairline(0.12, cx),
            ))
            .child(card)
            .into_any_element()
    }

    fn render_subagent_chip(&self, index: usize, cx: &Context<Self>) -> AnyElement {
        let Some(Entry::Tool(tool)) = self.entries(cx).get(index) else {
            return div().into_any_element();
        };
        let colors = cx.theme().colors();
        let danger = cx.theme().status().error;
        let (label, detail) = view::tool_chip_content(&tool.call);
        let running = self.session.read(cx).subagent_running(&tool.id);
        let detail = running
            .then(|| self.subagent_activity(&tool.id, cx))
            .flatten()
            .unwrap_or(detail);
        let failed = tool.status == ToolStatus::Failed;
        let tint = if failed { danger } else { colors.text_muted };
        let model = tool.call.subagent_model().map(|model| model.to_string());
        let id = tool.id.clone();
        let call = tool.call.clone();
        div()
            .h(px(CHIP_HEIGHT))
            .w_full()
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .child(
                div()
                    .id(SharedString::from(format!("subagent-{}", tool.id)))
                    .h(px(CHIP_CARD_HEIGHT))
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .rounded(px(9.))
                    .border_1()
                    .border_color(hairline(0.07, cx))
                    .bg(ink(0.03, cx))
                    .cursor_pointer()
                    .hover(|style| style.bg(ink(0.05, cx)))
                    .tooltip(Tooltip::text(t!("Open subagent")))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_subagent(id.clone(), &call, window, cx)
                    }))
                    .child(
                        h_flex()
                            .h(px(CHIP_HEADER_HEIGHT))
                            .w_full()
                            .min_w_0()
                            .gap(px(8.))
                            .px(px(8.))
                            .text_size(px(TOOL_TEXT_SIZE))
                            .line_height(px(TOOL_LINE_HEIGHT))
                            .child(
                                div()
                                    .size(px(18.))
                                    .flex_none()
                                    .rounded(px(5.))
                                    .bg(ink(0.08, cx))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        icon(tool_icon(&tool.call), 12.)
                                            .text_color(colors.text_muted),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .h(px(TOOL_LINE_HEIGHT))
                                    .flex()
                                    .items_center()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(tint)
                                    .child(label),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .h(px(TOOL_LINE_HEIGHT))
                                    .flex()
                                    .items_center()
                                    .truncate()
                                    .text_color(if failed {
                                        danger
                                    } else {
                                        colors.text.opacity(0.85)
                                    })
                                    .child(div().min_w_0().truncate().child(detail)),
                            )
                            .when_some(model, |this, model| {
                                this.child(
                                    div()
                                        .flex_none()
                                        .h(px(18.))
                                        .flex()
                                        .items_center()
                                        .text_size(px(11.))
                                        .text_color(text_faint(cx))
                                        .child(model),
                                )
                            })
                            .when(running, |this| {
                                this.child(div().flex_none().child(subagent_spinner(
                                    format!("subagent-spinner-{}", tool.id).into(),
                                    accent(cx),
                                    cx.theme().appearance.is_light(),
                                )))
                            })
                            .child(
                                div()
                                    .size(px(18.))
                                    .flex_none()
                                    .rounded(px(5.))
                                    .bg(ink(0.06, cx))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        icon(IconName::AgentArrowUpRight, 11.)
                                            .text_color(colors.text_muted.opacity(0.8)),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn subagent_activity(&self, id: &str, cx: &App) -> Option<String> {
        let subagent = self.session.read(cx).subagent(id)?;
        subagent.entries().iter().rev().find_map(|entry| match entry {
            Entry::Tool(tool) => {
                let (label, detail) = view::tool_chip_content(&tool.call);
                Some(if detail.is_empty() {
                    label.to_string()
                } else {
                    format!("{label} {detail}")
                })
            }
            Entry::Assistant { markdown, .. } => {
                let text = view::single_line(markdown.read(cx).source());
                (!text.is_empty()).then_some(text)
            }
            _ => None,
        })
    }

    fn render_tool_detail(&self, tool: &ToolEntry, cx: &Context<Self>) -> AnyElement {
        let faint = text_faint(cx);
        let show_all = self.full_output.contains(&tool.id);
        let max_lines = if show_all { usize::MAX } else { OUTPUT_MAX_LINES };
        let call_text = view::tool_call_text(&tool.call);
        let invocation = clip_lines(&call_text, CALL_WRAP_COLUMNS, max_lines);
        let output = clip_lines(tool.output.as_deref().unwrap_or(""), usize::MAX, max_lines);
        let output_size = tool.output.as_ref().map_or(0, |output| output.len());
        let diff = tool
            .diff
            .as_ref()
            .map(|diff| self.diff_view(&tool.id, diff, cx));
        let invocation_hidden = invocation.hidden;
        let output_hidden = output.hidden;
        v_flex()
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .when(!invocation.lines.is_empty(), |this| {
                this.child(detail_separator())
                    .child(detail_lines(
                        invocation.lines,
                        invocation_hidden,
                        faint,
                        code_font(cx),
                    ))
                    .when(invocation_hidden > 0, |this| {
                        this.child(
                            self.render_show_full(
                                format!("agent-full-input-{}", tool.id),
                                t!(
                                    "Show full input ({size})",
                                    size = format_bytes(call_text.len())
                                )
                                .to_string(),
                                &tool.id,
                                cx,
                            ),
                        )
                    })
            })
            .map(|this| match diff {
                Some(diff) => this
                    .child(detail_separator())
                    .child(self.render_diff(&diff, cx)),
                None if !output.lines.is_empty() => this
                    .child(detail_separator())
                    .child(detail_lines(
                        output.lines,
                        output_hidden,
                        faint,
                        code_font(cx),
                    ))
                    .when(output_hidden > 0, |this| {
                        this.child(
                            self.render_show_full(
                                format!("agent-full-output-{}", tool.id),
                                t!(
                                    "Show full output ({size})",
                                    size = format_bytes(output_size)
                                )
                                .to_string(),
                                &tool.id,
                                cx,
                            ),
                        )
                    }),
                None => this,
            })
            .into_any_element()
    }

    fn render_show_full(
        &self,
        id: String,
        label: String,
        tool_id: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let tool_id = tool_id.to_string();
        let muted = cx.theme().colors().text_muted;
        div()
            .id(SharedString::from(id))
            .h(px(BLOB_AFFORDANCE_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .text_size(px(TOOL_TEXT_SIZE))
            .text_color(text_faint(cx))
            .cursor_pointer()
            .hover(|style| style.text_color(muted))
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.full_output.insert(tool_id.clone());
                cx.notify();
            }))
    }

    fn render_new_chat(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let project_name: SharedString = self
            .session
            .read(cx)
            .metadata()
            .cwd
            .file_name()
            .map(|name| name.to_string_lossy().into_owned().into())
            .unwrap_or_default();
        v_flex()
            .size_full()
            .justify_center()
            .items_center()
            .pt(px(16.))
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(COMPOSER_WIDTH))
                    .px(px(16.))
                    .gap(px(8.))
                    .child(
                        h_flex().h(px(20.)).px(px(10.)).justify_end().child(
                            Self::footer_label_shell(IconName::AgentFolder, cx)
                                .child(div().min_w_0().truncate().child(project_name)),
                        ),
                    )
                    .child(self.render_pill(COMPOSER_RADIUS, window, cx))
                    .child(
                        h_flex()
                            .h(px(24.))
                            .px(px(10.))
                            .child(
                                h_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .children(self.render_agent_update(cx)),
                            )
                            .children(self.render_move_to_panel(cx)),
                    ),
            )
            .into_any_element()
    }
}

pub(crate) fn code_font(cx: &App) -> Font {
    ThemeSettings::get_global(cx).buffer_font.clone()
}

fn code_text_style(cx: &App) -> TextStyleRefinement {
    let font = code_font(cx);
    TextStyleRefinement {
        font_family: Some(font.family),
        font_features: Some(font.features),
        font_fallbacks: font.fallbacks,
        font_weight: Some(font.weight),
        font_style: Some(font.style),
        ..Default::default()
    }
}

fn render_diagram(diagram: Diagram, view: WeakEntity<ChatView>) -> AnyElement {
    let Diagram::Ready {
        image,
        width,
        height,
    } = diagram
    else {
        return gpui::Empty.into_any_element();
    };
    div()
        .id(SharedString::from(format!("agent-diagram-{}", image.id())))
        .w_full()
        .max_w(px(width))
        .mx_auto()
        .max_h(px(DIAGRAM_MAX_HEIGHT))
        .aspect_ratio(width / height.max(1.))
        .cursor_pointer()
        .child(
            img(image.clone())
                .size_full()
                .object_fit(ObjectFit::Contain),
        )
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            let image = image.clone();
            view.update(cx, |this, cx| this.open_image_data(image, window, cx))
                .ok();
        })
        .into_any_element()
}

fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024. * 1024.))
    }
}

fn subagent_title(call: &ToolCall) -> SharedString {
    let (name, input) = match call {
        ToolCall::Unknown { name, input } => (name.as_str(), input.as_ref()),
        ToolCall::Mcp { tool, input, .. } => (tool.as_str(), input.as_ref()),
        _ => return t!("Subagent"),
    };
    let candidates = [
        Some(name),
        input.and_then(|input| input.get("description")?.as_str()),
        input.and_then(|input| input.get("prompt")?.as_str()),
    ];
    candidates
        .into_iter()
        .flatten()
        .map(|text| view::single_line(text.strip_prefix("Agent").unwrap_or(text)))
        .map(|text| text.trim_start_matches(':').trim().to_string())
        .find(|text| !text.is_empty())
        .map_or_else(|| t!("Subagent"), Into::into)
}

fn icon(name: IconName, size: f32) -> Svg {
    svg().path(name.path()).size(px(size)).flex_none()
}

enum CopySource {
    Text(SharedString),
    Messages(Range<usize>),
}

enum StripReveal {
    OnGroupHover(SharedString),
    When(bool),
}

impl StripReveal {
    fn apply<E: gpui::InteractiveElement + Styled>(&self, element: E) -> E {
        match self {
            StripReveal::OnGroupHover(group) => element.visible_on_hover(group.clone()),
            StripReveal::When(true) => element,
            StripReveal::When(false) => element.invisible(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct TabState {
    title: SharedString,
    working: bool,
    needs_input: bool,
}

struct SaveTarget {
    entry: usize,
    source: CopySource,
    from_agent: bool,
}

impl CopySource {
    fn text(&self, chat: &ChatView, cx: &App) -> String {
        match self {
            CopySource::Text(text) => text.to_string(),
            CopySource::Messages(entries) => chat
                .entries(cx)
                .get(entries.clone())
                .unwrap_or_default()
                .iter()
                .filter_map(|entry| match entry {
                    Entry::Assistant { markdown, .. } => {
                        let source: &str = markdown.read(cx).source();
                        Some(source)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n\n"),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum TranscriptRow {
    ToolGroup,
    Other,
}

fn top_gap(previous: TranscriptRow, current: TranscriptRow) -> f32 {
    if previous == TranscriptRow::ToolGroup || current == TranscriptRow::ToolGroup {
        SPACE_MD
    } else {
        SPACE_SM
    }
}

fn shimmer_amount(x: f32, phase: f32) -> f32 {
    let primary_center = -2.5 + phase.clamp(0., 1.) * 6.;
    (-2..=2)
        .map(|copy| primary_center + copy as f32 * 3.)
        .map(|center| (1. - (x - center).abs() / TOOL_SHIMMER_HALF_WIDTH).clamp(0., 1.))
        .fold(0., f32::max)
}

fn shimmer_overlay(text: SharedString, phase: f32, base: Hsla, peak: Hsla) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            let font = window.text_style().font();
            let run = |color: Hsla| TextRun {
                len: text.len(),
                font: font.clone(),
                color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let text_system = window.text_system();
            let probe =
                text_system.shape_line(text.clone(), px(TOOL_TEXT_SIZE), &[run(peak)], None);
            let text_width = f32::from(probe.width()).min(f32::from(bounds.size.width));
            let strip_count = (text_width / TOOL_SHIMMER_STRIP_WIDTH).ceil() as usize;
            let mut strips = Vec::with_capacity(strip_count);
            for strip in 0..strip_count {
                let left = strip as f32 * TOOL_SHIMMER_STRIP_WIDTH;
                let right = ((strip + 1) as f32 * TOOL_SHIMMER_STRIP_WIDTH).min(text_width);
                let x = (left + right) * 0.5 / text_width.max(1.);
                let amount = shimmer_amount(x, phase);
                if amount <= 0.001 {
                    continue;
                }
                let line = text_system.shape_line(
                    text.clone(),
                    px(TOOL_TEXT_SIZE),
                    &[run(mix(base, peak, amount))],
                    None,
                );
                strips.push((left, right, line));
            }
            strips
        },
        move |bounds, strips, window, cx| {
            for (left, right, line) in strips {
                let mask = ContentMask {
                    bounds: Bounds {
                        origin: point(bounds.origin.x + px(left), bounds.origin.y),
                        size: size(px(right - left), bounds.size.height),
                    },
                };
                window.with_content_mask(Some(mask), |window| {
                    line.paint(
                        bounds.origin,
                        px(TOOL_LINE_HEIGHT),
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    )
                    .log_err();
                });
            }
        },
    )
    .absolute()
    .inset_0()
}

fn shimmer_title(text: SharedString, id: ElementId, base: Hsla, peak: Hsla) -> AnyElement {
    div()
        .relative()
        .h_full()
        .min_w_0()
        .flex_1()
        .overflow_hidden()
        .child(text.clone())
        .with_animation(
            id,
            working_animation(GROUP_SHIMMER),
            move |title, phase| title.child(shimmer_overlay(text.clone(), phase, base, peak)),
        )
        .into_any_element()
}

fn activity_ribbon(path: &mut PathBuilder, points: &[Point<Pixels>]) {
    let (Some(first), Some(last)) = (points.first(), points.last()) else {
        return;
    };
    let mut left = Vec::with_capacity(points.len());
    let mut right = Vec::with_capacity(points.len());
    for (position, current) in points.iter().enumerate() {
        let previous = points.get(position.wrapping_sub(1)).unwrap_or(first);
        let next = points.get(position + 1).unwrap_or(last);
        let dx = f32::from(next.x - previous.x);
        let dy = f32::from(next.y - previous.y);
        let length = dx.hypot(dy).max(0.0001);
        let normal = point(px(-dy / length * 0.5), px(dx / length * 0.5));
        left.push(*current + normal);
        right.push(*current - normal);
    }
    let Some(start) = left.first() else {
        return;
    };
    path.move_to(*start);
    for corner in left.iter().skip(1).chain(right.iter().rev()) {
        path.line_to(*corner);
    }
    path.close();
}

fn activity_branch_points() -> Vec<Point<f32>> {
    let mut points: Vec<Point<f32>> = (0..=24)
        .map(|step| {
            let t = step as f32 / 24.;
            point(
                TREE_BEND_RADIUS * t * t,
                TREE_BEND_RADIUS * (2. * t - t * t),
            )
        })
        .collect();
    points.push(point(TREE_BRANCH_END_X - TREE_TRUNK_X, TREE_BEND_RADIUS));
    points
}

fn activity_rail(
    icon_name: IconName,
    tint: Hsla,
    continues: bool,
    color: Hsla,
) -> impl IntoElement {
    div()
        .relative()
        .w(px(TREE_GUTTER))
        .flex_none()
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let x = bounds.origin.x + px(TREE_TRUNK_X);
                    let bend_y = bounds.origin.y + px(TREE_ROW_HEIGHT / 2. - TREE_BEND_RADIUS);
                    let bottom = if continues { bounds.bottom() } else { bend_y };
                    let mut tree = PathBuilder::fill().with_style(PathStyle::Fill(
                        FillOptions::default().with_fill_rule(FillRule::NonZero),
                    ));
                    activity_ribbon(&mut tree, &[point(x, bounds.origin.y), point(x, bottom)]);
                    let branch: Vec<Point<Pixels>> = activity_branch_points()
                        .into_iter()
                        .map(|offset| point(x + px(offset.x), bend_y + px(offset.y)))
                        .collect();
                    activity_ribbon(&mut tree, &branch);
                    if let Ok(path) = tree.build() {
                        window.paint_path(path, color);
                    }
                },
            )
            .absolute()
            .inset_0(),
        )
        .child(
            icon(icon_name, TREE_ICON_SIZE)
                .absolute()
                .left(px(TREE_ICON_LEFT))
                .top(px(TREE_ROW_HEIGHT / 2. - TREE_ICON_SIZE / 2.))
                .text_color(tint),
        )
}

fn detail_separator() -> impl IntoElement {
    div().h(px(DETAIL_SEPARATOR)).flex_none()
}

fn detail_lines(
    lines: Vec<String>,
    hidden: usize,
    color: Hsla,
    font: Font,
) -> impl IntoElement {
    v_flex()
        .w_full()
        .min_w_0()
        .overflow_hidden()
        .py(px(6.))
        .font(font)
        .text_size(px(TOOL_TEXT_SIZE))
        .children(lines.into_iter().map(move |line| {
            div()
                .h(px(OUTPUT_LINE_HEIGHT))
                .w_full()
                .min_w_0()
                .flex()
                .items_center()
                .text_color(color)
                .child(div().w_full().min_w_0().truncate().child(line))
        }))
        .when(hidden > 0, |this| {
            this.child(
                div()
                    .h(px(OUTPUT_LINE_HEIGHT))
                    .flex()
                    .items_center()
                    .text_size(px(TOOL_TEXT_SIZE))
                    .text_color(color)
                    .child(t!("… {count} more lines", count = hidden)),
            )
        })
}

#[derive(Debug, PartialEq)]
struct ClippedLines {
    lines: Vec<String>,
    hidden: usize,
}

fn wrapped_line_count(line: &str, columns: usize) -> usize {
    line.chars().count().div_ceil(columns).max(1)
}

fn clip_lines(text: &str, columns: usize, max_lines: usize) -> ClippedLines {
    let mut lines = Vec::new();
    let mut hidden = 0;
    for line in text.trim_end().lines() {
        if lines.len() >= max_lines {
            hidden += wrapped_line_count(line, columns);
            continue;
        }
        let mut rest = line;
        loop {
            let split = rest
                .char_indices()
                .nth(columns)
                .map_or(rest.len(), |(index, _)| index);
            let (chunk, tail) = rest.split_at(split);
            lines.push(chunk.to_string());
            rest = tail;
            if rest.is_empty() {
                break;
            }
            if lines.len() >= max_lines {
                hidden += wrapped_line_count(rest, columns);
                break;
            }
        }
    }
    ClippedLines { lines, hidden }
}

fn plain_code_block_renderer() -> CodeBlockRenderer {
    CodeBlockRenderer::Custom {
        render: Arc::new(|_, _, _, _, _, _| div().w_full()),
        transform: None,
        hide_body: None,
    }
}

fn spinner_opacity(t: f32) -> f32 {
    let t = t.rem_euclid(1.);
    if t < 0.45 {
        1. + (SUBAGENT_SPINNER_DIM - 1.) * (t / 0.45)
    } else if t < 0.92 {
        SUBAGENT_SPINNER_DIM
    } else {
        SUBAGENT_SPINNER_DIM + (1. - SUBAGENT_SPINNER_DIM) * ((t - 0.92) / 0.08)
    }
}

fn subagent_spinner(id: SharedString, accent: Hsla, is_light: bool) -> impl IntoElement {
    const RING: [[usize; 2]; 3] = [[0, 1], [5, 2], [4, 3]];
    let mut light = accent;
    let mut deep = accent;
    if is_light {
        light.l = (light.l + 0.11).min(0.76);
        light.s *= 0.78;
        deep.l = (deep.l - 0.09).max(0.22);
    } else {
        light.l = (light.l + 0.14).min(0.9);
        light.s *= 0.72;
        deep.l = (deep.l - 0.08).max(0.22);
    }
    let tints = [light, accent, deep];
    let cell = SUBAGENT_SPINNER_CELL;
    v_flex()
        .flex_none()
        .gap(px(cell / 2.))
        .children(tints.into_iter().enumerate().map(move |(row, tint)| {
            let id = id.clone();
            h_flex()
                .gap(px(cell / 2.))
                .children((0..2).map(move |column| {
                    let phase = RING[row][column] as f32 / 6.;
                    div()
                        .size(px(cell))
                        .rounded(px(cell / 2.))
                        .bg(tint)
                        .with_animation(
                            ElementId::Name(format!("{id}-{row}-{column}").into()),
                            working_animation(SUBAGENT_SPINNER_PERIOD),
                            move |cell, delta| cell.opacity(spinner_opacity(delta + phase)),
                        )
                }))
        }))
}

fn tool_icon(call: &ToolCall) -> IconName {
    match call {
        ToolCall::Exec { .. } => IconName::AgentTerminal,
        ToolCall::ReadFile { .. } | ToolCall::ApplyPatch { .. } => IconName::AgentDocument,
        ToolCall::WriteFile { .. } => IconName::AgentDocumentAdd,
        ToolCall::EditFile { .. } => IconName::AgentPen,
        ToolCall::Search { .. } => IconName::AgentMagnifer,
        ToolCall::Glob { .. } => IconName::AgentFolderWithFiles,
        ToolCall::WebFetch { .. } | ToolCall::WebSearch { .. } => IconName::AgentGlobal,
        ToolCall::Todo { .. } => IconName::AgentChecklist,
        call if call.is_subagent_spawn() => IconName::AgentBot,
        ToolCall::Unknown { name, .. } if name == "Wait for agents" => IconName::AgentBot,
        ToolCall::Mcp { .. } | ToolCall::Unknown { .. } => IconName::AgentWidget,
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Turn {
    user: Option<usize>,
    items: Range<usize>,
    first_reply: Option<usize>,
    last_reply: Option<usize>,
}

impl Turn {
    fn first_entry(&self) -> usize {
        self.user.unwrap_or(self.items.start)
    }
}

fn extend_turns(turns: &mut Vec<Turn>, entries: &[Entry], from: usize) {
    for (index, entry) in entries.iter().enumerate().skip(from) {
        if matches!(entry, Entry::User { .. }) {
            turns.push(Turn {
                user: Some(index),
                items: index + 1..index + 1,
                first_reply: None,
                last_reply: None,
            });
            continue;
        }
        if turns.is_empty() {
            turns.push(Turn {
                user: None,
                items: index..index,
                first_reply: None,
                last_reply: None,
            });
        }
        if let Some(turn) = turns.last_mut() {
            turn.items.end = index + 1;
            if matches!(entry, Entry::Assistant { .. }) {
                turn.first_reply.get_or_insert(index);
                turn.last_reply = Some(index);
            }
        }
    }
}

fn group_turns(entries: &[Entry]) -> Vec<Turn> {
    let mut turns = Vec::new();
    extend_turns(&mut turns, entries, 0);
    turns
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum RowKind {
    User(usize),
    Worked,
    ToolGroup { start: usize, end: usize },
    Message(usize),
    Notice(usize),
    Image(usize),
    Actions,
    Working,
    Retry(usize),
}

impl RowKind {
    fn spacing(self) -> Option<TranscriptRow> {
        match self {
            RowKind::Worked | RowKind::ToolGroup { .. } => Some(TranscriptRow::ToolGroup),
            RowKind::Message(_) | RowKind::Notice(_) | RowKind::Image(_) => {
                Some(TranscriptRow::Other)
            }
            RowKind::User(_) | RowKind::Actions | RowKind::Working | RowKind::Retry(_) => None,
        }
    }

    fn hover_id(self, turn: usize) -> Option<ElementId> {
        match self {
            RowKind::Worked => Some(("agent-row-worked", turn).into()),
            RowKind::ToolGroup { start, .. } => Some(("agent-row-tools", start).into()),
            RowKind::Message(entry) => Some(("agent-row-message", entry).into()),
            RowKind::Notice(entry) => Some(("agent-row-notice", entry).into()),
            RowKind::Image(entry) => Some(("agent-row-image", entry).into()),
            RowKind::Actions => Some(("agent-row-actions", turn).into()),
            RowKind::Working => Some(("agent-row-working", turn).into()),
            RowKind::User(_) | RowKind::Retry(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Row {
    turn: usize,
    kind: RowKind,
}

impl Row {
    fn identity(self) -> Row {
        match self.kind {
            RowKind::ToolGroup { start, .. } => Row {
                turn: self.turn,
                kind: RowKind::ToolGroup { start, end: start },
            },
            _ => self,
        }
    }

    fn first_entry(self, turn: &Turn) -> usize {
        match self.kind {
            RowKind::User(entry)
            | RowKind::Message(entry)
            | RowKind::Notice(entry)
            | RowKind::Image(entry) => entry,
            RowKind::ToolGroup { start, .. } => start,
            RowKind::Worked => turn.items.start,
            RowKind::Actions | RowKind::Working | RowKind::Retry(_) => turn.items.end,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RowFlags {
    working: bool,
    compact: bool,
    standalone: bool,
}

fn push_turn_rows(
    rows: &mut Vec<Row>,
    turn_index: usize,
    turn: &Turn,
    entries: &[Entry],
    live: bool,
    expanded: bool,
    flags: RowFlags,
) {
    let mut push = |kind| {
        rows.push(Row {
            turn: turn_index,
            kind,
        })
    };
    if let Some(user) = turn.user {
        push(RowKind::User(user));
    }
    let hidden_until = turn
        .last_reply
        .filter(|reply| flags.compact && !live && *reply > turn.items.start);
    if hidden_until.is_some() {
        push(RowKind::Worked);
    }
    let shown = match hidden_until {
        Some(reply) if !expanded => reply..turn.items.end,
        _ => turn.items.clone(),
    };
    let mut group: Option<(usize, bool)> = None;
    for index in shown.start..shown.end.min(entries.len()) {
        let Some(entry) = entries.get(index) else {
            break;
        };
        let spawns_subagent = match entry {
            Entry::Tool(tool) => Some(tool.call.is_subagent_spawn()),
            Entry::Thinking(_) => Some(false),
            _ => None,
        };
        if let Some((start, group_spawns_subagent)) = group
            && spawns_subagent != Some(group_spawns_subagent)
        {
            push(RowKind::ToolGroup { start, end: index });
            group = None;
        }
        match entry {
            Entry::Tool(_) | Entry::Thinking(_) => {
                if group.is_none() {
                    group = spawns_subagent.map(|spawns_subagent| (index, spawns_subagent));
                }
            }
            Entry::Assistant { .. } => push(RowKind::Message(index)),
            Entry::Notice { .. } => push(RowKind::Notice(index)),
            Entry::Image { .. } => push(RowKind::Image(index)),
            Entry::User { .. } => {}
        }
    }
    if let Some((start, _)) = group {
        push(RowKind::ToolGroup {
            start,
            end: shown.end.min(entries.len()),
        });
    }
    if live {
        push(RowKind::Working);
    } else if turn.last_reply.is_some() {
        push(RowKind::Actions);
    }
    if let Some(user) = turn.user
        && flags.standalone
        && matches!(
            entries.get(user),
            Some(Entry::User {
                undelivered: true,
                ..
            })
        )
    {
        push(RowKind::Retry(user));
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum EntryStamp {
    User { at: i64, length: usize },
    Assistant(gpui::EntityId),
    Thinking(gpui::EntityId),
    Tool { id: u64, spawns_subagent: bool },
    Notice { length: usize, is_error: bool },
    Image(u64),
}

impl EntryStamp {
    fn of(entry: &Entry) -> Self {
        match entry {
            Entry::User { text, at, .. } => EntryStamp::User {
                at: *at,
                length: text.len(),
            },
            Entry::Assistant { markdown, .. } => EntryStamp::Assistant(markdown.entity_id()),
            Entry::Thinking(markdown) => EntryStamp::Thinking(markdown.entity_id()),
            Entry::Tool(tool) => EntryStamp::Tool {
                id: gpui::hash(&tool.id),
                spawns_subagent: tool.call.is_subagent_spawn(),
            },
            Entry::Notice { text, is_error } => EntryStamp::Notice {
                length: text.len(),
                is_error: *is_error,
            },
            Entry::Image { path } => EntryStamp::Image(gpui::hash(path)),
        }
    }
}

#[derive(Debug, Default, PartialEq)]
struct RowEdits {
    splice: Option<(Range<usize>, usize)>,
    remeasure: Vec<usize>,
}

#[derive(Default)]
struct Transcript {
    turns: Vec<Turn>,
    rows: Vec<Row>,
    turn_rows: Vec<usize>,
    regroups: usize,
    entry_count: usize,
    last_entry: Option<EntryStamp>,
    last_turn_start: Option<EntryStamp>,
    flags: Option<RowFlags>,
    last_user_undelivered: bool,
}

impl Transcript {
    fn sync(
        &mut self,
        entries: &[Entry],
        flags: RowFlags,
        expanded_work: &HashSet<usize>,
    ) -> RowEdits {
        let previous_last_turn = self.turns.len().saturating_sub(1);
        let mut stale_from = self.sync_turns(entries);
        let last_user_undelivered = self.turns.last().is_some_and(|turn| {
            turn.user.is_some_and(|user| {
                matches!(
                    entries.get(user),
                    Some(Entry::User {
                        undelivered: true,
                        ..
                    })
                )
            })
        });
        let stale_flags = match self.flags {
            Some(known) if known.compact != flags.compact => Some(0),
            Some(known) if known.standalone != flags.standalone => Some(0),
            Some(known)
                if known.working != flags.working
                    || last_user_undelivered != self.last_user_undelivered =>
            {
                Some(previous_last_turn)
            }
            Some(_) => None,
            None => Some(0),
        };
        self.flags = Some(flags);
        self.last_user_undelivered = last_user_undelivered;
        if let Some(turn) = stale_flags {
            stale_from = Some(stale_from.map_or(turn, |stale| stale.min(turn)));
        }
        match stale_from {
            Some(turn) => self.rebuild_rows(turn, entries, expanded_work),
            None => RowEdits::default(),
        }
    }

    fn sync_turns(&mut self, entries: &[Entry]) -> Option<usize> {
        let stamp = |index: Option<usize>| {
            index
                .and_then(|index| entries.get(index))
                .map(EntryStamp::of)
        };
        let last_turn_start = self.turns.last().map(Turn::first_entry);
        let only_appended = self.entry_count <= entries.len()
            && stamp(self.entry_count.checked_sub(1)) == self.last_entry
            && stamp(last_turn_start) == self.last_turn_start;
        let stale_from = if !only_appended {
            self.turns = group_turns(entries);
            self.regroups += 1;
            Some(0)
        } else if self.entry_count < entries.len() {
            let last_turn = self.turns.len().saturating_sub(1);
            extend_turns(&mut self.turns, entries, self.entry_count);
            Some(last_turn)
        } else {
            None
        };
        self.entry_count = entries.len();
        self.last_entry = stamp(entries.len().checked_sub(1));
        self.last_turn_start = stamp(self.turns.last().map(Turn::first_entry));
        stale_from
    }

    fn rebuild_rows(
        &mut self,
        from_turn: usize,
        entries: &[Entry],
        expanded_work: &HashSet<usize>,
    ) -> RowEdits {
        let Some(flags) = self.flags else {
            return RowEdits::default();
        };
        let from_turn = from_turn.min(self.turn_rows.len()).min(self.turns.len());
        let start = self
            .turn_rows
            .get(from_turn)
            .copied()
            .unwrap_or(self.rows.len());
        let old = self.rows.split_off(start);
        self.turn_rows.truncate(from_turn);
        let last_turn = self.turns.len().saturating_sub(1);
        for (turn_index, turn) in self.turns.iter().enumerate().skip(from_turn) {
            self.turn_rows.push(self.rows.len());
            push_turn_rows(
                &mut self.rows,
                turn_index,
                turn,
                entries,
                flags.working && turn_index == last_turn,
                expanded_work.contains(&turn_index),
                flags,
            );
        }
        let new = self.rows.get(start..).unwrap_or_default();
        let same_row = |(old, new): &(&Row, &Row)| old.identity() == new.identity();
        let prefix = old.iter().zip(new).take_while(same_row).count();
        let suffix = old
            .iter()
            .skip(prefix)
            .rev()
            .zip(new.iter().skip(prefix).rev())
            .take_while(same_row)
            .count();
        let removed = old.len() - prefix - suffix;
        let inserted = new.len() - prefix - suffix;
        let mut edits = RowEdits::default();
        let kept_before = old.iter().zip(new).take(prefix);
        let kept_after = old.iter().rev().zip(new.iter().rev()).take(suffix);
        for (offset, (before, after)) in kept_before.enumerate() {
            if before != after {
                edits.remeasure.push(start + offset);
            }
        }
        for (offset, (before, after)) in kept_after.enumerate() {
            if before != after {
                edits.remeasure.push(start + new.len() - 1 - offset);
            }
        }
        if removed > 0 || inserted > 0 {
            let first_changed = start + prefix;
            edits.splice = Some((first_changed..first_changed + removed, inserted));
            edits.remeasure.extend(first_changed.checked_sub(1));
            if suffix > 0 {
                edits.remeasure.push(first_changed + inserted);
            }
        }
        edits
    }

    fn turn_for_entry(&self, entry: usize) -> Option<usize> {
        let turn = self
            .turns
            .partition_point(|turn| turn.first_entry() <= entry)
            .checked_sub(1)?;
        (entry < self.turns.get(turn)?.items.end).then_some(turn)
    }

    fn turn_rows(&self, turn: usize) -> Range<usize> {
        let start = self.turn_rows.get(turn).copied().unwrap_or(self.rows.len());
        let end = self
            .turn_rows
            .get(turn + 1)
            .copied()
            .unwrap_or(self.rows.len());
        start..end
    }

    fn row_for_entry(&self, entry: usize) -> Option<usize> {
        let turn_index = self.turn_for_entry(entry)?;
        let turn = self.turns.get(turn_index)?;
        let rows = self.turn_rows(turn_index);
        let before = self
            .rows
            .get(rows.clone())?
            .partition_point(|row| row.first_entry(turn) <= entry);
        Some(rows.start + before.saturating_sub(1))
    }

    fn entry_for_row(&self, row: usize) -> Option<usize> {
        let row = self.rows.get(row)?;
        Some(row.first_entry(self.turns.get(row.turn)?))
    }
}

fn column(content: impl IntoElement, gutter: f32) -> impl IntoElement {
    h_flex()
        .w_full()
        .justify_center()
        .px(px(gutter))
        .child(div().w_full().max_w(px(CONTENT_WIDTH)).min_w_0().child(content))
}

impl ChatView {
    fn key_context(&self) -> KeyContext {
        let mut context = KeyContext::default();
        context.add("AgentChat");
        context
    }

    fn gutter(&self) -> f32 {
        if self.docked { SPACE_LG } else { SIDE_GUTTER }
    }

    fn first_row_inset(&self) -> f32 {
        if self.subagent.is_some() {
            SPACE_LG
        } else {
            SPACE_LG + FIRST_ROW_BREATHING_ROOM
        }
    }

    fn begin_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .hovered_row
            .is_some_and(|row| !self.painted_rows.borrow().contains_key(&row))
        {
            self.hovered_row = None;
        }
        self.painted_rows.borrow_mut().clear();
        self.frame_message_style = None;
        self.ensure_background_tick(cx);
        if !self.has_rendered {
            self.has_rendered = true;
            self.scanned_entries = 0;
            self.scanned_tail = None;
            if self.subagent.is_none() {
                self.start_lookups(cx);
            }
        }
        self.sync_transcript(cx);
        self.update_scroll_state();
        self.refresh_diagrams(window, cx);
        self.observe_todo(cx);
        self.images
            .update(cx, |images, cx| images.start_frame(window, cx));
    }

    fn render_transcript(&self, cx: &Context<Self>) -> impl IntoElement {
        let container = self.container_bounds.clone();
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .child(
                canvas(
                    move |bounds, _, _| container.set(Some(bounds)),
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(
                list(
                    self.list.clone(),
                    cx.processor(|this, row_index, window, cx| {
                        this.render_row(row_index, window, cx)
                    }),
                )
                .size_full(),
            )
            .when(self.show_scroll_button, |this| {
                this.child(self.render_scroll_button(cx))
            })
            .children(self.render_outline(cx))
    }

    fn render_subagent(&self, cx: &Context<Self>) -> impl IntoElement {
        let root = v_flex()
            .key_context("AgentChat")
            .track_focus(&self.focus_handle)
            .relative()
            .size_full()
            .when(self.side_pane.is_none(), |root| {
                root.bg(cx.theme().colors().editor_background)
            })
            .child(self.track_root_bounds())
            .children(self.render_lightbox(cx))
            .children(self.render_link_menu());
        if self.entries(cx).is_empty() {
            let message = if self.is_working(cx) {
                t!("Waiting for the subagent…")
            } else {
                t!("This subagent has no recorded activity.")
            };
            return root.child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(ui(13.))
                    .text_color(cx.theme().colors().text_muted)
                    .child(message),
            );
        }
        root.child(self.render_with_side_pane(self.render_transcript(cx).into_any_element(), cx))
    }
}

impl Render for ChatView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.begin_frame(window, cx);
        if self.subagent.is_none()
            && window.is_window_active()
            && self.session.read(cx).metadata().unseen
        {
            let session = self.session.clone();
            cx.defer(move |cx| session.update(cx, |session, cx| session.mark_seen(cx)));
        }
        if self.subagent.is_some() {
            return self.render_subagent(cx).into_any_element();
        }
        let session = self.session.read(cx);
        let is_new_chat = session.entries().is_empty() && !session.is_working();
        let pending = session
            .pending_questions()
            .first()
            .map(|pending| pending.id().to_string());
        let drop_highlight = cx.theme().colors().drop_target_background;
        let root = v_flex()
            .key_context(self.key_context())
            .relative()
            .size_full()
            .when(self.side_pane.is_none(), |root| {
                root.bg(cx.theme().colors().editor_background)
            })
            .child(self.track_root_bounds())
            .drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(drop_highlight))
            .drag_over::<DraggedSelection>(move |style, _, _, _| style.bg(drop_highlight))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                this.drop_external_paths(paths, window, cx)
            }))
            .on_drop(cx.listener(|this, selection: &DraggedSelection, window, cx| {
                let paths = this.selection_paths(selection, cx);
                this.drop_paths(paths, window, cx)
            }))
            .children(self.render_lightbox(cx))
            .children(self.render_link_menu())
            .on_action(cx.listener(Self::send))
            .on_action(cx.listener(Self::stop))
            .on_action(cx.listener(Self::submit_answer))
            .on_action(cx.listener(Self::select_previous_command))
            .on_action(cx.listener(Self::select_next_command))
            .on_action(cx.listener(Self::accept_command))
            .on_action(cx.listener(Self::dismiss_commands))
            .on_action(cx.listener(Self::toggle_dictation))
            .on_action(cx.listener(Self::rename_chat))
            .on_action(cx.listener(Self::move_to_panel))
            .on_action(cx.listener(Self::move_to_tab))
            .on_key_up(cx.listener(|this, event: &gpui::KeyUpEvent, _, cx| {
                this.dictation_key_up(event, cx)
            }));
        if is_new_chat {
            let new_chat = self.render_new_chat(window, cx).into_any_element();
            return root
                .child(self.render_with_side_pane(new_chat, cx))
                .into_any_element();
        }
        let composer_area = match pending.and_then(|id| {
            self.session
                .read(cx)
                .pending_questions()
                .iter()
                .find(|pending| pending.id() == id)
                .map(|pending| self.render_question_panel(pending, cx))
        }) {
            Some(panel) => panel,
            None => self.render_pill(DOCKED_COMPOSER_RADIUS, window, cx),
        };
        let chat = v_flex()
            .size_full()
            .child(self.render_transcript(cx))
            .child(
                h_flex().w_full().justify_center().child(
                    v_flex()
                        .w_full()
                        .max_w(px(COMPOSER_WIDTH))
                        .px(px(16.))
                        .pb(px(8.))
                        .child(self.render_status_strip(cx))
                        .children(self.render_todo_panel(cx))
                        .children(self.render_queue_panel(cx))
                        .child(composer_area)
                        .child(div().pt(px(8.)).child(self.render_footer(cx))),
                ),
            );
        root.child(self.render_with_side_pane(chat.into_any_element(), cx))
            .into_any_element()
    }
}

impl Focusable for ChatView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        if self.subagent.is_some() {
            return self.focus_handle.clone();
        }
        if !self.session.read(cx).pending_questions().is_empty() {
            return self.question_focus.clone();
        }
        self.composer.focus_handle(cx)
    }
}

impl EventEmitter<ItemEvent> for ChatView {}

impl Item for ChatView {
    type Event = ItemEvent;

    fn tab_content(&self, params: TabContentParams, _: &Window, cx: &App) -> AnyElement {
        let session = self.session.read(cx);
        let needs_input = self.subagent.is_none() && !session.pending_questions().is_empty();
        let is_working = self.is_working(cx);
        let renaming = self.rename_editor.is_some();
        let this = self.this.clone();
        h_flex()
            .gap_1()
            // A background tab's chat isn't rendered, so the tab itself receives the rename.
            .when(!params.selected, |this| {
                this.track_focus(&Focusable::focus_handle(self, cx))
            })
            .on_action({
                let this = this.clone();
                move |action: &RenameChat, window, cx| {
                    this.update(cx, |this, cx| this.rename_chat(action, window, cx))
                        .ok();
                }
            })
            .on_action({
                let this = this.clone();
                move |action: &MoveChatToPanel, window, cx| {
                    this.update(cx, |this, cx| this.move_to_panel(action, window, cx))
                        .ok();
                }
            })
            .on_action(move |action: &MoveChatToTab, window, cx| {
                this.update(cx, |this, cx| this.move_to_tab(action, window, cx))
                    .ok();
            })
            .child(
                div()
                    .relative()
                    .child(
                        Label::new(self.title(cx))
                            .single_line()
                            .color(params.text_color())
                            .when(renaming, |this| this.alpha(0.)),
                    )
                    .when_some(self.rename_editor.clone(), |this, editor| {
                        let confirm = self.this.clone();
                        let cancel = self.this.clone();
                        this.child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full()
                                .child(editor)
                                .on_action(move |_: &menu::Confirm, window, cx| {
                                    confirm
                                        .update(cx, |this, cx| {
                                            this.finish_renaming(true, window, cx)
                                        })
                                        .ok();
                                })
                                .on_action(move |_: &menu::Cancel, window, cx| {
                                    cancel
                                        .update(cx, |this, cx| {
                                            this.finish_renaming(false, window, cx)
                                        })
                                        .ok();
                                }),
                        )
                    }),
            )
            .when(needs_input, |this| {
                this.child(
                    Icon::new(IconName::AgentChatRoundLine)
                        .size(IconSize::XSmall)
                        .color(Color::Accent),
                )
            })
            .when(!needs_input && is_working, |this| {
                this.child(
                    svg()
                        .path(IconName::LoadCircle.path())
                        .flex_none()
                        .size(IconSize::XSmall.rems())
                        .text_color(Color::Muted.color(cx))
                        .with_animation(
                            "agent-chat-tab-working",
                            working_animation(Duration::from_secs(2)),
                            |icon, delta| {
                                icon.with_transformation(Transformation::rotate(percentage(delta)))
                            },
                        ),
                )
            })
            .into_any_element()
    }

    fn tab_content_text(&self, _detail: usize, cx: &App) -> SharedString {
        self.title(cx)
    }

    fn tab_extra_context_menu_actions(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Vec<(SharedString, Box<dyn gpui::Action>)> {
        if self.subagent.is_some() {
            return Vec::new();
        }
        let move_action: (SharedString, Box<dyn gpui::Action>) = if self.docked {
            (t!("Open in Tab"), Box::new(MoveChatToTab))
        } else {
            (t!("Move to Side Panel"), Box::new(MoveChatToPanel))
        };
        vec![(t!("Rename"), Box::new(RenameChat)), move_action]
    }

    fn tab_icon(&self, _: &Window, cx: &App) -> Option<Icon> {
        if self.subagent.is_some() {
            return Some(Icon::new(IconName::AgentBot));
        }
        Some(agent_icon(self.session.read(cx).kind()))
    }

    fn tab_tooltip_text(&self, cx: &App) -> Option<SharedString> {
        let kind = self.session.read(cx).kind();
        let kind = if self.subagent.is_some() {
            t!("Subagent")
        } else {
            kind.label().into()
        };
        Some(format!("{kind} · {}", self.title(cx)).into())
    }

    fn to_item_events(event: &Self::Event, f: &mut dyn FnMut(ItemEvent)) {
        f(*event)
    }

    fn show_toolbar(&self) -> bool {
        true
    }

    fn handle_drop(
        &self,
        active_pane: &workspace::Pane,
        dropped: &dyn std::any::Any,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        if self.subagent.is_some() {
            return false;
        }
        let paths = self.dropped_paths(dropped, cx);
        let has_image = paths
            .iter()
            .any(|path| attachments::is_attachable_image(path));
        // Drops on a pane edge split the pane, unless they carry an image to attach.
        if paths.is_empty() || (active_pane.drag_split_direction().is_some() && !has_image) {
            return false;
        }
        // The pane calls this while it is already updating the chat.
        let this = self.this.clone();
        window.defer(cx, move |window, cx| {
            this.update(cx, |chat, cx| chat.drop_paths(paths, window, cx))
                .log_err();
        });
        true
    }

    fn claims_drop(&self, dragged: &dyn std::any::Any, cx: &App) -> bool {
        self.subagent.is_none()
            && self
                .dropped_paths(dragged, cx)
                .iter()
                .any(|path| attachments::is_attachable_image(path))
    }

    fn as_searchable(
        &self,
        handle: &Entity<Self>,
        _: &App,
    ) -> Option<Box<dyn workspace::searchable::SearchableItemHandle>> {
        Some(Box::new(handle.clone()))
    }
}

impl EventEmitter<workspace::searchable::SearchEvent> for ChatView {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AcceptCommand, AgentKind, DismissCommands, SelectNextCommand, SelectPreviousCommand,
        session::tests::{new_store, run_until},
    };
    use gpui::{KeyBinding, TestAppContext, VisualTestContext};
    use project::FakeFs;

    fn bind_composer_keys(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let composer = Some("AgentComposer > Editor");
            let menu = Some("AgentComposer && command_menu > Editor");
            cx.bind_keys([
                KeyBinding::new("enter", Send, composer),
                KeyBinding::new("up", SelectPreviousCommand, menu),
                KeyBinding::new("down", SelectNextCommand, menu),
                KeyBinding::new("enter", AcceptCommand, menu),
                KeyBinding::new("escape", DismissCommands, menu),
            ]);
        });
    }

    fn composer_text(view: &Entity<ChatView>, cx: &mut VisualTestContext) -> String {
        view.read_with(cx, |view, cx| view.composer.read(cx).text(cx))
    }

    #[test]
    fn clip_lines_wraps_only_the_visible_lines_and_counts_the_rest() {
        let text = format!("{}\nshort\n\n{}\n\n  \n", "a".repeat(25), "b".repeat(21));
        assert_eq!(
            clip_lines(&text, 10, 4),
            ClippedLines {
                lines: vec!["a".repeat(10), "a".repeat(10), "a".repeat(5), "short".into()],
                hidden: 4,
            }
        );
        assert_eq!(
            clip_lines(&text, 10, 2),
            ClippedLines {
                lines: vec!["a".repeat(10), "a".repeat(10)],
                hidden: 6,
            }
        );
        assert_eq!(clip_lines(&text, 10, usize::MAX).hidden, 0);
        assert_eq!(clip_lines("é".repeat(3).as_str(), 2, 1).lines, ["éé"]);
        assert_eq!(clip_lines("one\ntwo", usize::MAX, 1).hidden, 1);
        assert!(clip_lines(" \n\n", 10, 4).lines.is_empty());
    }

    fn matches(view: &Entity<ChatView>, cx: &mut VisualTestContext) -> Vec<String> {
        view.read_with(cx, |view, cx| {
            view.completion_names(cx)
        })
    }

    const SHOWN_IN_FULL: RowFlags = RowFlags {
        working: false,
        compact: false,
        standalone: true,
    };

    fn row(turn: usize, kind: RowKind) -> Row {
        Row { turn, kind }
    }

    fn identities(rows: &[Row]) -> Vec<Row> {
        rows.iter().map(|row| row.identity()).collect()
    }

    fn tool_call(id: &str, name: &str) -> agent_harness::AgentEvent {
        agent_harness::AgentEvent::ToolCall {
            id: id.into(),
            call: ToolCall::Unknown {
                name: name.into(),
                input: None,
            },
        }
    }

    fn text(text: &str) -> agent_harness::AgentEvent {
        agent_harness::AgentEvent::TextDelta { text: text.into() }
    }

    struct ShownRows {
        transcript: Transcript,
        list: Vec<Row>,
    }

    impl ShownRows {
        fn sync(
            &mut self,
            session: &Entity<AgentSession>,
            flags: RowFlags,
            expanded_work: &HashSet<usize>,
            cx: &mut TestAppContext,
        ) -> RowEdits {
            session.read_with(cx, |session, _| {
                let entries = session.entries();
                let edits = self.transcript.sync(entries, flags, expanded_work);
                self.apply(&edits);
                let mut rebuilt = Transcript::default();
                rebuilt.sync(entries, flags, expanded_work);
                assert_eq!(self.transcript.turns, group_turns(entries));
                assert_eq!(self.transcript.turns, rebuilt.turns);
                assert_eq!(self.transcript.rows, rebuilt.rows);
                assert_eq!(self.transcript.turn_rows, rebuilt.turn_rows);
                assert_eq!(identities(&self.list), identities(&rebuilt.rows));
                edits
            })
        }

        fn apply(&mut self, edits: &RowEdits) {
            if let Some((range, count)) = edits.splice.clone() {
                let inserted = self.transcript.rows[range.start..range.start + count].to_vec();
                self.list.splice(range, inserted);
            }
            for row in &edits.remeasure {
                assert!(
                    *row < self.list.len(),
                    "remeasured a row that does not exist"
                );
            }
        }
    }

    #[gpui::test]
    async fn rows_follow_the_chat_without_rebuilding_earlier_rows(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = new_store(directory.path(), cx);
        let session = store.update(cx, |store, cx| {
            store.create_session(AgentKind::Claude, directory.path().to_path_buf(), cx)
        });
        let working = RowFlags {
            working: true,
            ..SHOWN_IN_FULL
        };
        let mut expanded_work = HashSet::default();
        let mut shown = ShownRows {
            transcript: Transcript::default(),
            list: Vec::new(),
        };

        crate::session::tests::feed_user(&session, "first", cx);
        shown.sync(&session, SHOWN_IN_FULL, &expanded_work, cx);
        assert_eq!(shown.transcript.rows, [row(0, RowKind::User(0))]);

        crate::session::tests::feed_events(&session, vec![text("On it.")], cx);
        let edits = shown.sync(&session, working, &expanded_work, cx);
        assert_eq!(edits.splice, Some((1..1, 2)));
        assert_eq!(
            shown.transcript.rows,
            [
                row(0, RowKind::User(0)),
                row(0, RowKind::Message(1)),
                row(0, RowKind::Working),
            ]
        );

        crate::session::tests::feed_events(&session, vec![text(" Looking around.")], cx);
        let edits = shown.sync(&session, working, &expanded_work, cx);
        assert_eq!(edits, RowEdits::default(), "streamed text changes no rows");

        crate::session::tests::feed_events(&session, vec![tool_call("a", "Bash")], cx);
        let edits = shown.sync(&session, working, &expanded_work, cx);
        assert_eq!(edits.splice, Some((2..2, 1)));

        crate::session::tests::feed_events(&session, vec![tool_call("b", "Bash")], cx);
        let edits = shown.sync(&session, working, &expanded_work, cx);
        assert_eq!(
            edits,
            RowEdits {
                splice: None,
                remeasure: vec![2],
            },
            "a tool joining the last group only remeasures that group"
        );

        crate::session::tests::feed_events(
            &session,
            vec![tool_call("c", "Agent: explore"), text("Done.")],
            cx,
        );
        let edits = shown.sync(&session, working, &expanded_work, cx);
        assert_eq!(edits.splice, Some((3..3, 2)));
        assert_eq!(
            shown.transcript.rows,
            [
                row(0, RowKind::User(0)),
                row(0, RowKind::Message(1)),
                row(0, RowKind::ToolGroup { start: 2, end: 4 }),
                row(0, RowKind::ToolGroup { start: 4, end: 5 }),
                row(0, RowKind::Message(5)),
                row(0, RowKind::Working),
            ]
        );
        for (entry, row) in [(0, 0), (1, 1), (2, 2), (3, 2), (4, 3), (5, 4)] {
            assert_eq!(
                shown.transcript.row_for_entry(entry),
                Some(row),
                "entry {entry}"
            );
            assert_eq!(shown.transcript.turn_for_entry(entry), Some(0));
        }
        assert_eq!(shown.transcript.row_for_entry(6), None);

        let edits = shown.sync(&session, SHOWN_IN_FULL, &expanded_work, cx);
        assert_eq!(edits.splice, Some((5..6, 1)));
        assert_eq!(
            shown.transcript.rows.last(),
            Some(&row(0, RowKind::Actions))
        );

        let compact = RowFlags {
            compact: true,
            ..SHOWN_IN_FULL
        };
        shown.sync(&session, compact, &expanded_work, cx);
        assert_eq!(
            shown.transcript.rows,
            [
                row(0, RowKind::User(0)),
                row(0, RowKind::Worked),
                row(0, RowKind::Message(5)),
                row(0, RowKind::Actions),
            ]
        );
        assert_eq!(
            shown.transcript.row_for_entry(3),
            Some(1),
            "a folded entry maps to the row that folds it"
        );

        expanded_work.insert(0);
        let edits = session.read_with(cx, |session, _| {
            shown
                .transcript
                .rebuild_rows(0, session.entries(), &expanded_work)
        });
        shown.apply(&edits);
        assert_eq!(edits.splice, Some((2..2, 3)));
        assert_eq!(shown.transcript.rows.len(), 7);
        assert_eq!(identities(&shown.list), identities(&shown.transcript.rows));

        crate::session::tests::feed_user(&session, "second", cx);
        let edits = shown.sync(&session, compact, &expanded_work, cx);
        assert_eq!(edits.splice, Some((7..7, 1)));
        assert_eq!(
            shown.transcript.rows.last(),
            Some(&row(1, RowKind::User(6)))
        );
        assert_eq!(shown.transcript.turn_rows(1), 7..8);
        assert_eq!(shown.transcript.entry_for_row(7), Some(6));

        let other = store.update(cx, |store, cx| {
            store.create_session(AgentKind::Claude, directory.path().to_path_buf(), cx)
        });
        crate::session::tests::feed_events(
            &other,
            vec![text("Hello."), tool_call("z", "Bash")],
            cx,
        );
        let regroups = shown.transcript.regroups;
        shown.sync(&other, compact, &expanded_work, cx);
        assert_eq!(
            shown.transcript.regroups,
            regroups + 1,
            "replaced entries fall back to a full regroup"
        );
        assert_eq!(shown.transcript.turns.len(), 1);
        assert_eq!(shown.transcript.turns[0].user, None);
    }

    #[gpui::test]
    async fn new_output_leaves_the_reading_position_alone(cx: &mut TestAppContext) {
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
        crate::session::tests::feed_user(&session, "a long task", cx);
        for step in 0..40 {
            crate::session::tests::feed_events(
                &session,
                vec![
                    text(&format!("Step {step}.\n\nMore about it.")),
                    tool_call(&format!("tool-{step}"), "Bash"),
                ],
                cx,
            );
        }
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
        let draw = |cx: &mut VisualTestContext| {
            cx.run_until_parked();
            cx.update(|window, cx| {
                let _ = window.draw(cx);
            });
        };
        draw(cx);
        let rows_in_sync = |cx: &mut VisualTestContext| {
            chat.read_with(cx, |chat, _| {
                (chat.list.item_count(), chat.transcript.rows.len())
            })
        };
        assert_eq!(rows_in_sync(cx), (82, 82), "one long turn is many rows");
        let painted = chat.read_with(cx, |chat, _| chat.painted_rows.borrow().len());
        assert!(painted < 40, "only the rows on screen are drawn: {painted}");

        chat.update(cx, |chat, cx| {
            chat.list.scroll_to(ListOffset {
                item_ix: 12,
                offset_in_item: px(4.),
            });
            cx.notify();
        });
        draw(cx);
        let reading_position = |cx: &mut VisualTestContext| {
            chat.read_with(cx, |chat, _| {
                let top = chat.list.logical_scroll_top();
                (top.item_ix, top.offset_in_item)
            })
        };
        assert_eq!(reading_position(cx), (12, px(4.)));
        for step in 40..44 {
            crate::session::tests::feed_events(
                &session,
                vec![
                    text(&format!("Step {step}.")),
                    tool_call(&format!("tool-{step}"), "Bash"),
                    tool_call(&format!("tool-{step}-again"), "Bash"),
                ],
                cx,
            );
            draw(cx);
        }
        assert_eq!(rows_in_sync(cx), (90, 90));
        assert_eq!(reading_position(cx), (12, px(4.)));
    }

    #[gpui::test]
    async fn lookups_wait_until_the_chat_is_shown(cx: &mut TestAppContext) {
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
        let (multi_workspace, cx) = cx.add_window_view(|window, cx| {
            workspace::MultiWorkspace::test_new(project.clone(), window, cx)
        });
        let workspace = multi_workspace.read_with(cx, |multi, _| multi.workspace().clone());
        let chat = workspace.update_in(cx, |_, window, cx| {
            let weak = cx.entity().downgrade();
            cx.new(|cx| {
                ChatView::new(
                    session.clone(),
                    store.clone(),
                    project.clone(),
                    weak,
                    window,
                    cx,
                )
            })
        });
        let draw = |cx: &mut VisualTestContext| {
            cx.run_until_parked();
            cx.update(|window, cx| {
                let _ = window.draw(cx);
            });
        };
        draw(cx);
        assert!(!chat.read_with(cx, |chat, _| chat.has_rendered));

        workspace.update_in(cx, |workspace, window, cx| {
            workspace.add_item_to_active_pane(Box::new(chat.clone()), None, true, window, cx)
        });
        draw(cx);
        assert!(chat.read_with(cx, |chat, _| chat.has_rendered));
    }

    #[gpui::test]
    async fn the_tab_updates_only_when_what_it_shows_changes(cx: &mut TestAppContext) {
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
        let (view, cx) = cx.add_window_view(|window, cx| {
            ChatView::new(
                session.clone(),
                store.clone(),
                project,
                WeakEntity::new_invalid(),
                window,
                cx,
            )
        });
        let tab_updates = Rc::new(Cell::new(0));
        cx.update(|_, cx| {
            cx.subscribe(&view, {
                let tab_updates = tab_updates.clone();
                move |_, event: &ItemEvent, _| {
                    if matches!(event, ItemEvent::UpdateTab) {
                        tab_updates.set(tab_updates.get() + 1);
                    }
                }
            })
            .detach()
        });
        let notify = |cx: &mut VisualTestContext| {
            session.update(cx, |_, cx| cx.notify());
            cx.run_until_parked();
        };

        notify(cx);
        assert_eq!(tab_updates.get(), 0);

        crate::session::tests::feed_events(&session, vec![text("First words")], cx);
        notify(cx);
        assert_eq!(tab_updates.get(), 1, "the tab starts showing its spinner");

        for _ in 0..5 {
            crate::session::tests::feed_events(&session, vec![text(" and more")], cx);
            notify(cx);
        }
        assert_eq!(tab_updates.get(), 1, "streamed text leaves the tab alone");

        session.update(cx, |session, cx| {
            session.update_metadata(|metadata| metadata.title = "Renamed".into(), cx)
        });
        cx.run_until_parked();
        assert_eq!(tab_updates.get(), 2, "a new title reaches the tab");
    }

    #[gpui::test]
    async fn spawn_rows_open_the_subagent_in_the_side_pane(cx: &mut TestAppContext) {
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
        crate::session::tests::feed_subagent(&session, cx);
        let (multi_workspace, cx) = cx.add_window_view(|window, cx| {
            workspace::MultiWorkspace::test_new(project.clone(), window, cx)
        });
        let workspace = multi_workspace.read_with(cx, |multi, _| multi.workspace().clone());
        let chat = workspace.update_in(cx, |workspace, window, cx| {
            let weak = cx.entity().downgrade();
            let chat = cx.new(|cx| {
                ChatView::new(session.clone(), store.clone(), project.clone(), weak, window, cx)
            });
            workspace.add_item_to_active_pane(Box::new(chat.clone()), None, true, window, cx);
            chat
        });
        let open = |cx: &mut VisualTestContext| {
            chat.update_in(cx, |chat, window, cx| {
                let call = ToolCall::Unknown {
                    name: "Agent: scan the repo".into(),
                    input: None,
                };
                chat.open_subagent("spawn-1".into(), &call, window, cx)
            });
            cx.run_until_parked();
        };
        open(cx);
        open(cx);
        let tab_count = |cx: &mut VisualTestContext| {
            workspace.read_with(cx, |workspace, cx| {
                workspace.items_of_type::<ChatView>(cx).count()
            })
        };
        assert_eq!(
            tab_count(cx),
            1,
            "the subagent opens beside the chat, not in a tab"
        );
        chat.read_with(cx, |chat, cx| {
            let Some(side_pane::SidePaneContent::Subagent { view, .. }) = chat.side_pane_content()
            else {
                panic!("expected the subagent in the side pane");
            };
            let view = view.read(cx);
            assert_eq!(view.title(cx), "scan the repo");
            assert_eq!(view.entries(cx).len(), 3);
        });

        chat.update_in(cx, |chat, window, cx| {
            chat.move_side_pane_to_tab(window, cx)
        });
        cx.run_until_parked();
        open(cx);
        chat.update_in(cx, |chat, window, cx| {
            chat.move_side_pane_to_tab(window, cx)
        });
        cx.run_until_parked();
        assert_eq!(
            tab_count(cx),
            2,
            "moving to a tab twice reuses the subagent tab"
        );
        assert!(chat.read_with(cx, |chat, _| chat.side_pane_content().is_none()));
        let subagent_view = workspace
            .read_with(cx, |workspace, cx| {
                workspace
                    .items_of_type::<ChatView>(cx)
                    .find(|view| view.read(cx).is_subagent())
            })
            .expect("subagent tab");
        subagent_view.read_with(cx, |view, cx| {
            assert_eq!(view.title(cx), "scan the repo");
            assert_eq!(view.entries(cx).len(), 3);
        });
    }

    #[gpui::test]
    async fn pane_keeps_focus_while_the_agent_streams(cx: &mut TestAppContext) {
        use agent_harness::AgentEvent;
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
        let (multi_workspace, cx) = cx.add_window_view(|window, cx| {
            workspace::MultiWorkspace::test_new(project.clone(), window, cx)
        });
        let workspace = multi_workspace.read_with(cx, |multi, _| multi.workspace().clone());
        let chat = workspace.update_in(cx, |workspace, window, cx| {
            let weak = cx.entity().downgrade();
            let chat = cx.new(|cx| {
                ChatView::new(session.clone(), store.clone(), project.clone(), weak, window, cx)
            });
            workspace.add_item_to_active_pane(Box::new(chat.clone()), None, true, window, cx);
            chat
        });
        let pane = workspace.read_with(cx, |workspace, _| workspace.active_pane().clone());
        crate::session::tests::feed_events(
            &session,
            vec![AgentEvent::TextDelta { text: "First reply with some words.".into() }],
            cx,
        );
        let draw = |cx: &mut VisualTestContext| {
            cx.run_until_parked();
            cx.update(|window, cx| {
                let _ = window.draw(cx);
            });
        };
        draw(cx);
        let focus_targets: Vec<(&str, FocusHandle)> = cx.update(|_, cx| {
            let markdown = chat.read(cx).entries(cx).iter().find_map(|entry| match entry {
                Entry::Assistant { markdown, .. } => Some(markdown.focus_handle(cx)),
                _ => None,
            });
            [("composer", Some(chat.read(cx).composer.focus_handle(cx))), ("reply", markdown)]
                .into_iter()
                .filter_map(|(name, handle)| Some((name, handle?)))
                .collect()
        });
        for (name, target) in focus_targets {
            cx.update(|window, cx| window.focus(&target, cx));
            draw(cx);
            let mut lost = Vec::new();
            for step in 0..40 {
                let events = match step % 5 {
                    0 => vec![AgentEvent::ReasoningDelta { text: format!("thinking {step} ") }],
                    1 => vec![AgentEvent::ToolCall {
                        id: format!("tool-{step}"),
                        call: ToolCall::Unknown { name: "Bash".into(), input: None },
                    }],
                    2 => vec![AgentEvent::ToolResult {
                        id: format!("tool-{}", step - 1),
                        is_error: false,
                        output: Some("ok".into()),
                        diff: None,
                    }],
                    3 => vec![AgentEvent::TextDelta { text: format!("more text {step} ") }],
                    _ => vec![AgentEvent::AssistantMessageCompleted { assistant_message_id: String::new() }],
                };
                crate::session::tests::feed_events(&session, events, cx);
                draw(cx);
                let focused = cx.update(|window, cx| pane.read(cx).has_focus(window, cx));
                if !focused {
                    lost.push(step);
                }
            }
            assert!(lost.is_empty(), "pane lost focus ({name} focused) at steps {lost:?}");
        }
    }

    #[gpui::test]
    async fn generated_images_reserve_their_height_before_they_load(cx: &mut TestAppContext) {
        use agent_harness::AgentEvent;
        cx.update(|cx| {
            workspace::AppState::test(cx);
            editor::init(cx);
        });
        let directory = tempfile::tempdir().expect("temporary directory");
        let picture = directory.path().join("picture.png");
        image::RgbImage::new(1024, 768).save(&picture).expect("write png");
        let store = new_store(directory.path(), cx);
        store.update(cx, |store, _| store.skip_plan_usage());
        let project = project::Project::test(FakeFs::new(cx.executor()), [], cx).await;
        let session = store.update(cx, |store, cx| {
            store.create_session(AgentKind::Codex, directory.path().to_path_buf(), cx)
        });
        let (multi_workspace, cx) = cx.add_window_view(|window, cx| {
            workspace::MultiWorkspace::test_new(project.clone(), window, cx)
        });
        let workspace = multi_workspace.read_with(cx, |multi, _| multi.workspace().clone());
        let chat = workspace.update_in(cx, |workspace, window, cx| {
            let weak = cx.entity().downgrade();
            let chat = cx.new(|cx| {
                ChatView::new(session.clone(), store.clone(), project.clone(), weak, window, cx)
            });
            workspace.add_item_to_active_pane(Box::new(chat.clone()), None, true, window, cx);
            chat
        });
        cx.run_until_parked();
        crate::session::tests::feed_events(
            &session,
            vec![AgentEvent::GeneratedImage {
                id: "image".into(),
                path: picture.to_string_lossy().into_owned(),
                name: "picture.png".into(),
                mime_type: "image/png".into(),
            }],
            cx,
        );
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        let turn_height = chat.read_with(cx, |chat, _| {
            chat.list.bounds_for_item(0).map(|bounds| bounds.size.height)
        });
        assert!(turn_height.is_some_and(|height| height >= px(384.)), "{turn_height:?}");
    }

    #[gpui::test]
    async fn chats_rename_from_their_tab_whether_or_not_it_is_active(cx: &mut TestAppContext) {
        cx.update(|cx| {
            workspace::AppState::test(cx);
            editor::init(cx);
        });
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = new_store(directory.path(), cx);
        store.update(cx, |store, _| store.skip_plan_usage());
        let project = project::Project::test(FakeFs::new(cx.executor()), [], cx).await;
        let (multi_workspace, cx) = cx.add_window_view(|window, cx| {
            workspace::MultiWorkspace::test_new(project.clone(), window, cx)
        });
        let workspace = multi_workspace.read_with(cx, |multi, _| multi.workspace().clone());
        let open_chat = |cx: &mut VisualTestContext| {
            let session = store.update(cx, |store, cx| {
                store.create_session(AgentKind::Claude, directory.path().to_path_buf(), cx)
            });
            session.update(cx, |session, cx| {
                session.update_metadata(|metadata| metadata.title = "Old name".into(), cx)
            });
            workspace.update_in(cx, |workspace, window, cx| {
                let weak = cx.entity().downgrade();
                let chat = cx.new(|cx| {
                    ChatView::new(
                        session.clone(),
                        store.clone(),
                        project.clone(),
                        weak,
                        window,
                        cx,
                    )
                });
                workspace.add_item_to_active_pane(Box::new(chat.clone()), None, true, window, cx);
                chat
            })
        };
        let background = open_chat(cx);
        let active = open_chat(cx);
        let draw = |cx: &mut VisualTestContext| {
            cx.run_until_parked();
            cx.update(|window, cx| {
                let _ = window.draw(cx);
            });
        };
        let rename = |chat: &Entity<ChatView>,
                      text: &str,
                      finish: &dyn gpui::Action,
                      cx: &mut VisualTestContext| {
            draw(cx);
            cx.update(|window, cx| {
                let target = chat.read(cx).composer.focus_handle(cx);
                target.dispatch_action(&RenameChat, window, cx);
            });
            draw(cx);
            let editor = chat
                .read_with(cx, |chat, _| chat.rename_editor.clone())
                .expect("the tab should be renaming");
            cx.update(|window, cx| {
                editor.update(cx, |editor, cx| editor.set_text(text, window, cx));
                editor.focus_handle(cx).dispatch_action(finish, window, cx);
            });
            draw(cx);
            assert!(chat.read_with(cx, |chat, _| chat.rename_editor.is_none()));
        };
        let title = |chat: &Entity<ChatView>, cx: &mut VisualTestContext| {
            chat.read_with(cx, |chat, cx| chat.title(cx))
        };

        rename(&active, "Release notes", &menu::Confirm, cx);
        assert_eq!(title(&active, cx), "Release notes");

        rename(&background, "Ignored", &menu::Cancel, cx);
        assert_eq!(title(&background, cx), "Old name");
        rename(&background, "Background chat", &menu::Confirm, cx);
        assert_eq!(title(&background, cx), "Background chat");
    }

    #[gpui::test]
    async fn focus_left_on_a_message_scrolled_out_of_view_moves_to_the_composer(
        cx: &mut TestAppContext,
    ) {
        use agent_harness::AgentEvent;
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
        let (multi_workspace, cx) = cx.add_window_view(|window, cx| {
            workspace::MultiWorkspace::test_new(project.clone(), window, cx)
        });
        let workspace = multi_workspace.read_with(cx, |multi, _| multi.workspace().clone());
        let chat = workspace.update_in(cx, |workspace, window, cx| {
            let weak = cx.entity().downgrade();
            let chat = cx.new(|cx| {
                ChatView::new(session.clone(), store.clone(), project.clone(), weak, window, cx)
            });
            workspace.add_item_to_active_pane(Box::new(chat.clone()), None, true, window, cx);
            chat
        });
        let pane = workspace.read_with(cx, |workspace, _| workspace.active_pane().clone());
        for turn in 0..60 {
            crate::session::tests::feed_user(&session, &format!("question {turn}"), cx);
            crate::session::tests::feed_events(
                &session,
                vec![AgentEvent::TextDelta {
                    text: format!("Answer {turn}.\n\nSecond paragraph.\n\nThird paragraph."),
                }],
                cx,
            );
        }
        let draw = |cx: &mut VisualTestContext| {
            cx.run_until_parked();
            cx.update(|window, cx| {
                let _ = window.draw(cx);
            });
        };
        chat.update(cx, |chat, cx| {
            chat.list.scroll_to(ListOffset {
                item_ix: 0,
                offset_in_item: px(0.),
            });
            cx.notify();
        });
        draw(cx);
        let first_reply = chat.read_with(cx, |chat, cx| {
            chat.entries(cx).iter().find_map(|entry| match entry {
                Entry::Assistant { markdown, .. } => Some(markdown.focus_handle(cx)),
                _ => None,
            })
        });
        let first_reply = first_reply.expect("an assistant reply");
        let other_session = store.update(cx, |store, cx| {
            store.create_session(AgentKind::Claude, directory.path().to_path_buf(), cx)
        });
        let other_chat = workspace.update_in(cx, |workspace, window, cx| {
            let weak = cx.entity().downgrade();
            let other = cx.new(|cx| {
                ChatView::new(other_session, store.clone(), project.clone(), weak, window, cx)
            });
            workspace.split_item(
                workspace::SplitDirection::Right,
                Box::new(other.clone()),
                window,
                cx,
            );
            other
        });
        draw(cx);
        cx.update(|window, cx| window.focus(&other_chat.read(cx).composer.focus_handle(cx), cx));
        draw(cx);
        cx.update(|window, cx| window.focus(&first_reply, cx));
        draw(cx);
        assert!(cx.update(|window, cx| pane.read(cx).has_focus(window, cx)));

        chat.update(cx, |chat, cx| {
            chat.list.set_follow_mode(FollowMode::Tail);
            cx.notify();
        });
        let mut frames = String::new();
        for _ in 0..8 {
            draw(cx);
            frames.push(if cx.update(|window, cx| pane.read(cx).has_focus(window, cx)) {
                '+'
            } else {
                '-'
            });
        }
        assert_eq!(frames, "++++++++", "pane focus per frame");
        let composer_focused = cx.update(|window, cx| {
            chat.read(cx).composer.focus_handle(cx).is_focused(window)
        });
        assert!(composer_focused, "focus falls back to the composer");
    }

    #[gpui::test]
    async fn model_picker_closes_on_a_click_elsewhere(cx: &mut TestAppContext) {
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
        let (view, cx) = cx.add_window_view(|window, cx| {
            ChatView::new(session.clone(), store.clone(), project, WeakEntity::new_invalid(), window, cx)
        });
        cx.run_until_parked();
        view.update_in(cx, |view, window, cx| view.model_picker.show(window, cx));
        cx.run_until_parked();
        assert!(view.read_with(cx, |view, _| view.model_picker.is_deployed()));
        cx.simulate_click(point(px(4.), px(4.)), gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(!view.read_with(cx, |view, _| view.model_picker.is_deployed()));
    }

    #[gpui::test]
    async fn at_mentions_search_project_files(cx: &mut TestAppContext) {
        cx.update(|cx| {
            workspace::AppState::test(cx);
            editor::init(cx);
        });
        bind_composer_keys(cx);
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = new_store(directory.path(), cx);
        store.update(cx, |store, _| store.skip_plan_usage());
        let fs = FakeFs::new(cx.executor());
        fs.insert_tree(
            "/proj",
            serde_json::json!({ "src": { "main.rs": "", "lib.rs": "" }, "README.md": "" }),
        )
        .await;
        let project = project::Project::test(fs, [std::path::Path::new("/proj")], cx).await;
        let session = store.update(cx, |store, cx| {
            store.create_session(AgentKind::Claude, "/proj".into(), cx)
        });
        let (view, cx) = cx.add_window_view(|window, cx| {
            ChatView::new(session.clone(), store.clone(), project, WeakEntity::new_invalid(), window, cx)
        });
        view.update_in(cx, |view, window, cx| {
            window.focus(&view.composer.focus_handle(cx), cx)
        });
        cx.simulate_input("look at @");
        cx.run_until_parked();
        assert_eq!(matches(&view, cx), ["src", "README.md"]);
        cx.simulate_input("mai");
        cx.run_until_parked();
        assert_eq!(matches(&view, cx), ["main.rs"]);
        cx.simulate_keystrokes("enter");
        assert_eq!(composer_text(&view, cx), "look at @src/main.rs ");
        let shown = view.update(cx, |view, cx| {
            view.composer.update(cx, |editor, cx| editor.display_text(cx))
        });
        assert_eq!(shown, "look at ⋯ ");
        view.update_in(cx, |view, window, cx| {
            view.composer.update(cx, |editor, cx| {
                editor.backspace(&editor::actions::Backspace, window, cx);
                editor.backspace(&editor::actions::Backspace, window, cx);
            })
        });
        assert_eq!(composer_text(&view, cx), "look at ");
    }

    #[gpui::test]
    async fn saved_messages_are_found_even_after_the_chat_changes(cx: &mut TestAppContext) {
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
        crate::session::tests::feed_stopped_turn(
            &session,
            "first question",
            1,
            ToolCall::Glob {
                pattern: "*.rs".into(),
            },
            cx,
        );
        crate::session::tests::feed_events(
            &session,
            vec![agent_harness::AgentEvent::TextDelta {
                text: "First answer".into(),
            }],
            cx,
        );
        crate::session::tests::feed_stopped_turn(
            &session,
            "second question",
            1,
            ToolCall::Glob {
                pattern: "*.md".into(),
            },
            cx,
        );
        let (view, cx) = cx.add_window_view(|window, cx| {
            ChatView::new(
                session.clone(),
                store.clone(),
                project,
                WeakEntity::new_invalid(),
                window,
                cx,
            )
        });
        let reveal = |entry: usize, text: &str, cx: &mut VisualTestContext| {
            view.update(cx, |view, cx| {
                view.reveal_message(entry, text, cx)
                    .then_some(view.revealed_turn)
                    .flatten()
            })
        };
        let answer = session.read_with(cx, |session, _| {
            session
                .entries()
                .iter()
                .position(|entry| matches!(entry, Entry::Assistant { .. }))
        });
        assert_eq!(
            reveal(answer.expect("an answer"), "First answer", cx),
            Some(0)
        );
        assert_eq!(
            reveal(0, "second question", cx),
            Some(1),
            "a stale position falls back to the text"
        );
        assert_eq!(reveal(0, "never sent", cx), None);
    }

    #[gpui::test]
    async fn editing_a_stopped_message_puts_it_back_in_the_input(cx: &mut TestAppContext) {
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
        crate::session::tests::feed_stopped_turn(
            &session,
            "fix teh bug",
            1,
            ToolCall::Glob {
                pattern: "*.rs".into(),
            },
            cx,
        );
        let (view, cx) = cx.add_window_view(|window, cx| {
            ChatView::new(
                session.clone(),
                store.clone(),
                project,
                WeakEntity::new_invalid(),
                window,
                cx,
            )
        });
        view.update_in(cx, |view, window, cx| {
            view.composer
                .update(cx, |editor, cx| editor.set_text("and the docs", window, cx));
            view.edit_last_message(window, cx);
        });
        assert_eq!(composer_text(&view, cx), "fix teh bug\n\nand the docs");
        assert!(session.read_with(cx, |session, _| session.entries().is_empty()));
        assert_eq!(
            session.read_with(cx, |session, _| session.editable_message()),
            None
        );
    }

    #[gpui::test]
    async fn at_mentions_start_with_recent_files_and_rank_files_first(cx: &mut TestAppContext) {
        cx.update(|cx| {
            workspace::AppState::test(cx);
            editor::init(cx);
        });
        bind_composer_keys(cx);
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = new_store(directory.path(), cx);
        store.update(cx, |store, _| store.skip_plan_usage());
        let fs = FakeFs::new(cx.executor());
        fs.insert_tree(
            "/proj",
            serde_json::json!({
                "src": { "main.rs": "", "lib.rs": "" },
                "lib": {},
                "README.md": "",
            }),
        )
        .await;
        let project = project::Project::test(fs, [std::path::Path::new("/proj")], cx).await;
        let session = store.update(cx, |store, cx| {
            store.create_session(AgentKind::Claude, "/proj".into(), cx)
        });
        let (multi_workspace, cx) = cx.add_window_view(|window, cx| {
            workspace::MultiWorkspace::test_new(project.clone(), window, cx)
        });
        let workspace = multi_workspace.read_with(cx, |multi, _| multi.workspace().clone());
        for path in ["/proj/src/lib.rs", "/proj/src/main.rs"] {
            workspace
                .update_in(cx, |workspace, window, cx| {
                    workspace.open_abs_path(path.into(), Default::default(), window, cx)
                })
                .await
                .expect("open file");
        }
        let view = workspace.update_in(cx, |workspace, window, cx| {
            let weak = cx.entity().downgrade();
            let chat = cx.new(|cx| {
                ChatView::new(session.clone(), store.clone(), project.clone(), weak, window, cx)
            });
            workspace.add_item_to_active_pane(Box::new(chat.clone()), None, true, window, cx);
            chat
        });
        view.update_in(cx, |view, window, cx| {
            window.focus(&view.composer.focus_handle(cx), cx)
        });
        cx.simulate_input("@");
        cx.run_until_parked();
        assert_eq!(matches(&view, cx), ["main.rs", "lib.rs"]);
        cx.simulate_input("lib");
        cx.run_until_parked();
        assert_eq!(matches(&view, cx), ["lib.rs", "lib"]);
    }

    #[gpui::test]
    async fn file_links_open_read_only_in_the_side_pane(cx: &mut TestAppContext) {
        cx.update(|cx| {
            workspace::AppState::test(cx);
            editor::init(cx);
        });
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = new_store(directory.path(), cx);
        store.update(cx, |store, _| store.skip_plan_usage());
        let fs = FakeFs::new(cx.executor());
        fs.insert_tree(
            "/proj",
            serde_json::json!({ "src": { "main.rs": "fn main() {}\n" } }),
        )
        .await;
        let project = project::Project::test(fs, [std::path::Path::new("/proj")], cx).await;
        let session = store.update(cx, |store, cx| {
            store.create_session(AgentKind::Claude, "/proj".into(), cx)
        });
        let (view, cx) = cx.add_window_view(|window, cx| {
            ChatView::new(
                session.clone(),
                store.clone(),
                project,
                WeakEntity::new_invalid(),
                window,
                cx,
            )
        });
        let target = links::FileTarget {
            path: "/proj/src/main.rs".into(),
            line: Some(1),
        };
        view.update_in(cx, |view, window, cx| {
            view.open_file_in_side_pane(target, window, cx)
        });
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            let Some(side_pane::SidePaneContent::File { editor, .. }) = view.side_pane_content()
            else {
                panic!("expected the file in the side pane");
            };
            let editor = editor.read(cx);
            assert!(editor.read_only(cx));
            assert_eq!(editor.text(cx), "fn main() {}\n");
        });
        view.update_in(cx, |view, window, cx| view.close_side_pane(window, cx));
        assert!(view.read_with(cx, |view, _| view.side_pane_content().is_none()));
    }

    #[gpui::test]
    async fn dollar_menu_lists_claude_skills(cx: &mut TestAppContext) {
        cx.update(|cx| {
            workspace::AppState::test(cx);
            editor::init(cx);
        });
        bind_composer_keys(cx);
        let directory = tempfile::tempdir().expect("temporary directory");
        let skill = directory.path().join(".claude/skills/review");
        std::fs::create_dir_all(&skill).expect("skill directory");
        std::fs::write(skill.join("SKILL.md"), "body").expect("skill file");
        let store = new_store(directory.path(), cx);
        store.update(cx, |store, _| store.skip_plan_usage());
        let project = project::Project::test(FakeFs::new(cx.executor()), [], cx).await;
        let session = store.update(cx, |store, cx| {
            store.create_session(AgentKind::Claude, directory.path().to_path_buf(), cx)
        });
        let (view, cx) = cx.add_window_view(|window, cx| {
            ChatView::new(
                session.clone(),
                store.clone(),
                project,
                WeakEntity::new_invalid(),
                window,
                cx,
            )
        });
        view.update_in(cx, |view, window, cx| {
            window.focus(&view.composer.focus_handle(cx), cx)
        });

        cx.simulate_input("use $");
        run_until(cx, |cx| {
            store.read_with(cx, |store, _| {
                store
                    .commands(AgentKind::Claude, directory.path())
                    .is_some_and(|catalog| catalog.loaded)
            })
        });
        assert_eq!(matches(&view, cx), ["review"]);
        cx.simulate_keystrokes("enter");
        assert_eq!(composer_text(&view, cx), "use $review ");
        view.read_with(cx, |view, _| {
            assert_eq!(
                view.composer_skills,
                [agent_harness::SkillRef {
                    name: "review".into(),
                    path: "claude-skill:review".into(),
                }]
            )
        });
    }

    #[gpui::test]
    async fn slash_menu_inserts_agent_commands_and_runs_wu_commands(cx: &mut TestAppContext) {
        cx.update(|cx| {
            workspace::AppState::test(cx);
            editor::init(cx);
        });
        bind_composer_keys(cx);
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = new_store(directory.path(), cx);
        store.update(cx, |store, _| store.skip_plan_usage());
        let project = project::Project::test(FakeFs::new(cx.executor()), [], cx).await;
        let session = store.update(cx, |store, cx| {
            store.create_session(AgentKind::Claude, directory.path().to_path_buf(), cx)
        });
        let (view, cx) = cx.add_window_view(|window, cx| {
            ChatView::new(
                session.clone(),
                store.clone(),
                project,
                WeakEntity::new_invalid(),
                window,
                cx,
            )
        });
        view.update_in(cx, |view, window, cx| {
            window.focus(&view.composer.focus_handle(cx), cx)
        });

        cx.simulate_input("/re");
        run_until(cx, |cx| {
            store.read_with(cx, |store, _| {
                store
                    .commands(AgentKind::Claude, directory.path())
                    .is_some_and(|catalog| catalog.loaded)
            })
        });
        assert_eq!(matches(&view, cx), ["review", "resume"]);
        cx.simulate_keystrokes("down down enter");
        assert_eq!(composer_text(&view, cx), "/review ");
        let shown = view.update(cx, |view, cx| {
            view.composer.update(cx, |editor, cx| editor.display_text(cx))
        });
        assert_eq!(shown, "⋯ ");
        view.read_with(cx, |view, _| assert!(view.command_menu.is_none()));

        view.update_in(cx, |view, window, cx| {
            view.composer.update(cx, |editor, cx| editor.clear(window, cx))
        });
        cx.simulate_input("/");
        assert!(matches(&view, cx).contains(&"compact".to_string()));
        cx.simulate_keystrokes("escape");
        view.read_with(cx, |view, _| assert!(view.command_menu.is_none()));
        cx.simulate_input("sto");
        assert_eq!(matches(&view, cx), ["stop"]);
        cx.simulate_keystrokes("enter");
        assert_eq!(composer_text(&view, cx), "");

        cx.simulate_input("/");
        let names = matches(&view, cx);
        assert!(!names.contains(&"usage".to_string()));
        assert_eq!(names.iter().filter(|name| *name == "model").count(), 1);
        cx.simulate_keystrokes("escape");
        cx.simulate_input("usage now");
        cx.simulate_keystrokes("enter");
        assert_eq!(composer_text(&view, cx), "");
        session.read_with(cx, |session, _| assert!(session.entries().is_empty()));

        cx.simulate_input("/nothing-like-this");
        assert!(matches(&view, cx).is_empty());
        cx.simulate_keystrokes("enter");
        assert_eq!(composer_text(&view, cx), "");
        session.read_with(cx, |session, _| {
            assert!(matches!(
                session.entries().first(),
                Some(Entry::User { text, .. }) if text.as_ref() == "/nothing-like-this"
            ))
        });
    }
}
