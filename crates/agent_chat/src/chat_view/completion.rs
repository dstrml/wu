use super::{ChatView, ui};
use crate::{
    AcceptCommand, AgentKind, DismissCommands, NewClaudeChat, NewCodexChat, NewOpencodeChat,
    SelectNextCommand, SelectPreviousCommand, Send, ToggleFocus,
    chat_style::{MENU_ITEM_RADIUS, icon, ink, popover_card, selected_row},
    slash_commands::{
        AppCommand, CommandItem, CommandTarget, CommandToken, TokenKind, command_items,
        completion_token, filter_commands,
    },
};
use agent_harness::SkillRef;
use editor::{FoldPlaceholder, MultiBufferOffset, SelectionEffects, display_map::Crease};
use file_icons::FileIcons;
use gpui::{
    Animation, AnimationExt as _, AnyElement, App, Context, Corners, Focusable as _, FontWeight,
    Hsla, IntoElement, ParentElement, SharedString, Styled, Window, canvas, deferred, div,
    pulsating_between, px, relative,
};
use i18n::t;
use project::{Candidates, PathMatchCandidateSet, ProjectPath};
use std::{
    ops::Range,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};
use ui::{IconName, prelude::*};
use util::{ResultExt as _, paths::compare_rel_paths, rel_path::RelPath};
use workspace::{Toast, notifications::NotificationId};

const MAX_MENTION_RESULTS: usize = 50;

pub(super) struct CompletionMenu {
    pub kind: TokenKind,
    pub token: CommandToken,
    pub selected: usize,
}

#[derive(Clone)]
pub(super) struct MentionItem {
    pub path: PathBuf,
    pub label: String,
    pub detail: String,
    pub is_dir: bool,
}

#[derive(Clone)]
enum Row {
    Command(CommandItem),
    Skill(agent_harness::Skill),
    Mention(MentionItem),
}

impl Row {
    fn icon(&self, size: f32, cx: &App) -> AnyElement {
        let name = match self {
            Row::Command(item) if item.is_skill => IconName::AgentWidget,
            Row::Command(_) => IconName::AgentCommand,
            Row::Skill(_) => IconName::AgentWidget,
            Row::Mention(item) => {
                return file_icon(&item.path, item.is_dir, cx)
                    .size(IconSize::Custom(ui(size)))
                    .into_any_element();
            }
        };
        icon(name, px(size), cx.theme().colors().text_muted).into_any_element()
    }

    fn label(&self) -> String {
        match self {
            Row::Command(item) => format!("/{}", item.name),
            Row::Skill(skill) => skill_display_name(&skill.name),
            Row::Mention(item) => item.label.clone(),
        }
    }

    fn detail(&self) -> String {
        match self {
            Row::Command(item) => item.detail.clone(),
            Row::Skill(skill) => skill.description.clone(),
            Row::Mention(item) => item.detail.clone(),
        }
    }
}

fn skill_display_name(name: &str) -> String {
    name.rsplit(':')
        .next()
        .unwrap_or(name)
        .split(['-', '_', ' '])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut characters = word.chars();
            characters
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + characters.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn mention_text(path: &Path, is_dir: bool, cwd: &Path) -> String {
    let shown = path.strip_prefix(cwd).unwrap_or(path).to_string_lossy();
    let mut text = format!("@{shown}");
    if is_dir && !text.ends_with('/') {
        text.push('/');
    }
    text
}

fn file_icon(path: &Path, is_dir: bool, cx: &App) -> Icon {
    let theme_icon = if is_dir {
        FileIcons::get_folder_icon(false, path, cx)
    } else {
        FileIcons::get_icon(path, cx)
    };
    match theme_icon {
        Some(theme_icon) => Icon::from_path(theme_icon),
        None if is_dir => Icon::new(IconName::AgentFolder),
        None => Icon::new(IconName::AgentDocument),
    }
    .color(Color::Muted)
}

fn badge(row: Row) -> FoldPlaceholder {
    let label: SharedString = row.label().into();
    FoldPlaceholder {
        render: Arc::new(move |fold_id, _, cx| {
            div()
                .id(fold_id)
                .h_full()
                .flex()
                .items_center()
                .child(
                    h_flex()
                        .h(ui(20.))
                        .gap(px(4.))
                        .px(px(5.))
                        .rounded(px(6.))
                        .bg(ink(0.07, cx))
                        .text_size(ui(13.))
                        .text_color(cx.theme().colors().text)
                        .child(row.icon(14., cx))
                        .child(label.clone()),
                )
                .into_any_element()
        }),
        constrain_width: false,
        merge_adjacent: false,
        ..FoldPlaceholder::default()
    }
}

fn mention_item(path: PathBuf, is_dir: bool, cwd: &Path) -> MentionItem {
    let shown = path.strip_prefix(cwd).unwrap_or(&path).to_path_buf();
    let label = shown
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| shown.to_string_lossy().into_owned());
    let detail = shown
        .parent()
        .map(|parent| parent.to_string_lossy().into_owned())
        .unwrap_or_default();
    MentionItem {
        path,
        label,
        detail,
        is_dir,
    }
}

impl ChatView {
    fn session_kind_and_cwd(&self, cx: &App) -> (AgentKind, PathBuf) {
        let session = self.session.read(cx);
        (session.kind(), session.metadata().cwd.clone())
    }

    pub(super) fn command_items(&self, cx: &App) -> Vec<CommandItem> {
        let (kind, cwd) = self.session_kind_and_cwd(cx);
        let catalog = self.store.read(cx).commands(kind, &cwd);
        command_items(catalog.map_or(&[], |catalog| catalog.commands.as_slice()))
    }

    fn rows(&self, cx: &App) -> Vec<Row> {
        let Some(menu) = &self.command_menu else {
            return Vec::new();
        };
        match menu.kind {
            TokenKind::Command => filter_commands(&menu.token.query, self.command_items(cx))
                .into_iter()
                .map(Row::Command)
                .collect(),
            TokenKind::Skill => {
                let query = menu.token.query.to_lowercase();
                let mut skills: Vec<(usize, agent_harness::Skill)> = self
                    .available_skills(cx)
                    .0
                    .into_iter()
                    .filter(|skill| skill.enabled)
                    .filter_map(|skill| {
                        let name = skill.name.to_lowercase();
                        let rank = if name.starts_with(&query) {
                            0
                        } else if name.contains(&query) {
                            1
                        } else {
                            return None;
                        };
                        Some((rank, skill))
                    })
                    .collect();
                skills.sort_by_key(|(rank, _)| *rank);
                skills.into_iter().map(|(_, skill)| Row::Skill(skill)).collect()
            }
            TokenKind::Mention => self.mention_results.iter().cloned().map(Row::Mention).collect(),
        }
    }

    /// Claude Code reports its skills as slash commands.
    fn available_skills(&self, cx: &App) -> (Vec<agent_harness::Skill>, bool) {
        let (kind, cwd) = self.session_kind_and_cwd(cx);
        let store = self.store.read(cx);
        if kind == AgentKind::Claude {
            let Some(catalog) = store.commands(kind, &cwd) else {
                return (Vec::new(), false);
            };
            let skills = catalog
                .commands
                .iter()
                .filter(|command| command.is_skill)
                .map(|command| agent_harness::Skill {
                    name: command.name.clone(),
                    description: command.description.clone(),
                    path: Some(format!("claude-skill:{}", command.name)),
                    enabled: true,
                })
                .collect();
            return (skills, catalog.loaded);
        }
        store
            .skills(kind, &cwd)
            .map_or((Vec::new(), false), |catalog| {
                (catalog.skills.clone(), catalog.loaded)
            })
    }

    #[cfg(test)]
    pub(super) fn completion_names(&self, cx: &App) -> Vec<String> {
        self.rows(cx)
            .into_iter()
            .map(|row| match row {
                Row::Command(item) => item.name,
                Row::Skill(skill) => skill.name,
                Row::Mention(item) => item.label,
            })
            .collect()
    }

    pub(super) fn completion_count(&self, cx: &App) -> usize {
        self.rows(cx).len()
    }

    pub(super) fn update_command_menu(&mut self, cx: &mut Context<Self>) {
        let token = self.composer.update(cx, |editor, cx| {
            let snapshot = editor.display_snapshot(cx);
            let selection = editor.selections.newest::<MultiBufferOffset>(&snapshot);
            if !selection.is_empty() {
                return None;
            }
            completion_token(&editor.text(cx), selection.head().0)
        });
        let Some((kind, token)) = token else {
            self.dismissed_command = None;
            if self.command_menu.take().is_some() {
                cx.notify();
            }
            return;
        };
        if self.dismissed_command.as_ref() == Some(&token) {
            return;
        }
        self.dismissed_command = None;
        let query_changed = match &mut self.command_menu {
            Some(menu) if menu.token == token && menu.kind == kind => return,
            Some(menu) if menu.kind == kind => {
                let changed = menu.token.query != token.query;
                if changed {
                    menu.selected = 0;
                }
                menu.token = token.clone();
                changed
            }
            _ => {
                let (agent, cwd) = self.session_kind_and_cwd(cx);
                self.store.update(cx, |store, cx| match kind {
                    TokenKind::Command => store.refresh_commands(agent, cwd, cx),
                    TokenKind::Skill if agent == AgentKind::Claude => {
                        store.refresh_commands(agent, cwd, cx)
                    }
                    TokenKind::Skill => store.refresh_skills(agent, cwd, cx),
                    TokenKind::Mention => {}
                });
                self.command_menu = Some(CompletionMenu {
                    kind,
                    token: token.clone(),
                    selected: 0,
                });
                true
            }
        };
        if query_changed {
            self.command_scroll.scroll_to_item(0);
            if kind == TokenKind::Mention {
                self.search_mentions(token.query, cx);
            }
        }
        cx.notify();
    }

    fn recent_mentions(
        &self,
        history: Vec<(ProjectPath, Option<PathBuf>)>,
        cwd: &Path,
        cx: &App,
    ) -> Vec<MentionItem> {
        let project = self.project.read(cx);
        history
            .into_iter()
            .filter(|(project_path, _)| {
                project
                    .entry_for_path(project_path, cx)
                    .is_some_and(|entry| entry.is_file())
            })
            .filter_map(|(project_path, abs_path)| {
                abs_path.or_else(|| project.absolute_path(&project_path, cx))
            })
            .take(MAX_MENTION_RESULTS)
            .map(|path| mention_item(path, false, cwd))
            .collect()
    }

    fn root_mentions(&self, cwd: &Path, cx: &App) -> Vec<MentionItem> {
        self.project
            .read(cx)
            .visible_worktrees(cx)
            .flat_map(|worktree| {
                let worktree = worktree.read(cx);
                let mut entries: Vec<_> = worktree
                    .child_entries(RelPath::empty())
                    .filter(|entry| !entry.is_ignored)
                    .collect();
                entries.sort_by(|a, b| {
                    compare_rel_paths((&a.path, a.is_file()), (&b.path, b.is_file()))
                });
                entries
                    .into_iter()
                    .map(|entry| {
                        let path = worktree.abs_path().join(entry.path.as_std_path());
                        mention_item(path, entry.is_dir(), cwd)
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn search_mentions(&mut self, query: String, cx: &mut Context<Self>) {
        let cwd = self.session.read(cx).metadata().cwd.clone();
        if query.is_empty() {
            let workspace = self.workspace.clone();
            // The chat is itself a workspace item, so reading the history while it updates panics.
            self._mention_search = cx.spawn(async move |this, cx| {
                let history = workspace
                    .read_with(cx, |workspace, cx| {
                        workspace.recent_navigation_history(None, cx)
                    })
                    .unwrap_or_default();
                this.update(cx, |this, cx| {
                    let recent = this.recent_mentions(history, &cwd, cx);
                    this.mention_results = if recent.is_empty() {
                        this.root_mentions(&cwd, cx)
                    } else {
                        recent
                    };
                    cx.notify();
                })
                .log_err();
            });
            return;
        }
        let project = self.project.read(cx);
        let worktrees: Vec<_> = project.visible_worktrees(cx).collect();
        let roots: Vec<(usize, PathBuf)> = worktrees
            .iter()
            .map(|worktree| {
                let worktree = worktree.read(cx);
                (worktree.id().to_usize(), worktree.abs_path().to_path_buf())
            })
            .collect();
        let candidate_sets: Vec<PathMatchCandidateSet> = worktrees
            .iter()
            .map(|worktree| PathMatchCandidateSet {
                snapshot: worktree.read(cx).snapshot(),
                include_ignored: false,
                include_root_name: false,
                candidates: Candidates::Entries,
            })
            .collect();
        self._mention_search = cx.spawn(async move |this, cx| {
            let cancel = AtomicBool::new(false);
            let matches = fuzzy_nucleo::match_path_sets(
                candidate_sets.as_slice(),
                &query,
                &None,
                fuzzy_nucleo::Case::Ignore,
                MAX_MENTION_RESULTS,
                &cancel,
                cx.background_executor().clone(),
            )
            .await;
            let mut items: Vec<MentionItem> = matches
                .into_iter()
                .filter_map(|found| {
                    let root = roots
                        .iter()
                        .find(|(id, _)| *id == found.worktree_id)
                        .map(|(_, root)| root)?;
                    let path = root.join(found.path.as_std_path());
                    Some(mention_item(path, found.is_dir, &cwd))
                })
                .collect();
            items.sort_by_key(|item| item.is_dir);
            this.update(cx, |this, cx| {
                this.mention_results = items;
                cx.notify();
            })
            .log_err();
        });
    }

    fn move_command_selection(&mut self, step: isize, cx: &mut Context<Self>) {
        let count = self.completion_count(cx);
        let Some(menu) = &mut self.command_menu else {
            return;
        };
        if count == 0 {
            return;
        }
        menu.selected = (menu.selected as isize + step).rem_euclid(count as isize) as usize;
        self.command_scroll.scroll_to_item(menu.selected);
        cx.notify();
    }

    pub(super) fn select_previous_command(
        &mut self,
        _: &SelectPreviousCommand,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_command_selection(-1, cx);
    }

    pub(super) fn select_next_command(
        &mut self,
        _: &SelectNextCommand,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_command_selection(1, cx);
    }

    pub(super) fn dismiss_commands(
        &mut self,
        _: &DismissCommands,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.command_menu.take() {
            self.dismissed_command = Some(menu.token);
            cx.notify();
        }
    }

    pub(super) fn accept_command(
        &mut self,
        _: &AcceptCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let selected = self.command_menu.as_ref().map(|menu| menu.selected);
        match selected {
            Some(index) if index < self.completion_count(cx) => {
                self.pick_command(index, window, cx)
            }
            _ => self.send(&Send, window, cx),
        }
    }

    pub(super) fn insert_at_token(
        &mut self,
        range: std::ops::Range<usize>,
        insertion: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = self.composer.read(cx).text(cx);
        let next = text.get(range.end..).and_then(|rest| rest.chars().next());
        let insertion = if insertion.is_empty() || next.is_some_and(char::is_whitespace) {
            insertion
        } else {
            format!("{insertion} ")
        };
        self.composer.update(cx, |editor, cx| {
            let cursor = range.start + insertion.len();
            editor.edit(
                [(
                    MultiBufferOffset(range.start)..MultiBufferOffset(range.end),
                    insertion,
                )],
                cx,
            );
            editor.change_selections(SelectionEffects::default(), window, cx, |selections| {
                selections.select_ranges([MultiBufferOffset(cursor)..MultiBufferOffset(cursor)])
            });
        });
    }

    pub(super) fn pick_command(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.rows(cx).into_iter().nth(index) else {
            return;
        };
        let Some(menu) = self.command_menu.take() else {
            return;
        };
        let range = menu.token.range;
        let cwd = self.session.read(cx).metadata().cwd.clone();
        let text = match &row {
            Row::Command(CommandItem {
                target: CommandTarget::App(command),
                ..
            }) => {
                self.insert_at_token(range, String::new(), window, cx);
                self.run_app_command(*command, window, cx);
                cx.notify();
                return;
            }
            Row::Command(item) => format!("/{}", item.name),
            Row::Skill(skill) => {
                if let Some(path) = &skill.path {
                    let skill_ref = SkillRef {
                        name: skill.name.clone(),
                        path: path.clone(),
                    };
                    if !self.composer_skills.contains(&skill_ref) {
                        self.composer_skills.push(skill_ref);
                    }
                }
                format!("${}", skill.name)
            }
            Row::Mention(item) => mention_text(&item.path, item.is_dir, &cwd),
        };
        let badge_range = range.start..range.start + text.len();
        self.insert_at_token(range, text, window, cx);
        self.show_badge(badge_range, row, window, cx);
        cx.notify();
    }

    pub(super) fn insert_mentions(
        &mut self,
        paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if paths.is_empty() {
            return;
        }
        let cwd = self.session.read(cx).metadata().cwd.clone();
        let mentions: Vec<(PathBuf, bool, String)> = paths
            .into_iter()
            .map(|path| {
                let is_dir = path.is_dir();
                let text = mention_text(&path, is_dir, &cwd);
                (path, is_dir, text)
            })
            .collect();
        let text = mentions
            .iter()
            .map(|(_, _, text)| text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let end = self.composer.update(cx, |editor, cx| {
            editor.insert(&format!("{text} "), window, cx);
            let snapshot = editor.display_snapshot(cx);
            editor
                .selections
                .newest::<MultiBufferOffset>(&snapshot)
                .head()
                .0
        });
        let mut start = end.saturating_sub(text.len() + 1);
        for (path, is_dir, text) in mentions {
            let row = Row::Mention(mention_item(path, is_dir, &cwd));
            self.show_badge(start..start + text.len(), row, window, cx);
            start += text.len() + 1;
        }
        window.focus(&self.composer.focus_handle(cx), cx);
    }

    fn show_badge(
        &mut self,
        range: Range<usize>,
        row: Row,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let crease = Crease::simple(
            MultiBufferOffset(range.start)..MultiBufferOffset(range.end),
            badge(row),
        );
        self.composer.update(cx, |editor, cx| {
            editor.fold_creases(vec![crease], false, window, cx)
        });
    }

    pub(super) fn run_app_command(
        &mut self,
        command: AppCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match command {
            AppCommand::Model => self.model_picker.show(window, cx),
            AppCommand::New => match self.session.read(cx).kind() {
                AgentKind::Claude => window.dispatch_action(Box::new(NewClaudeChat), cx),
                AgentKind::Codex => window.dispatch_action(Box::new(NewCodexChat), cx),
                AgentKind::Opencode => window.dispatch_action(Box::new(NewOpencodeChat), cx),
            },
            AppCommand::Resume => window.dispatch_action(Box::new(ToggleFocus), cx),
            AppCommand::Stop => self.session.update(cx, |session, cx| session.stop(cx)),
            AppCommand::Settings | AppCommand::Diff | AppCommand::Files | AppCommand::Terminal => {
                let Some(name) = command.action_name() else {
                    return;
                };
                match cx.build_action(name, None) {
                    Ok(action) => window.dispatch_action(action, cx),
                    Err(error) => log::error!("cannot run {name}: {error}"),
                }
            }
        }
    }

    pub(super) fn redirect_hidden_command(
        &mut self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((name, _)) = agent_harness::leading_command(text) else {
            return false;
        };
        if self.session.read(cx).kind() != AgentKind::Claude
            || !agent_harness::claude::is_hidden_command(name)
            || self
                .command_items(cx)
                .iter()
                .any(|item| item.target == CommandTarget::Agent && item.name == name)
        {
            return false;
        }
        match name {
            "clear" => self.run_app_command(AppCommand::New, window, cx),
            "model" | "effort" | "fast" => self.run_app_command(AppCommand::Model, window, cx),
            "resume" => self.run_app_command(AppCommand::Resume, window, cx),
            _ => {
                let message = t!(
                    "/{name} only works in Claude Code in the terminal.",
                    name = name
                )
                .to_string();
                self.workspace
                    .update(cx, |workspace, cx| {
                        workspace.show_toast(
                            Toast::new(NotificationId::unique::<ChatView>(), message).autohide(),
                            cx,
                        )
                    })
                    .log_err();
            }
        }
        true
    }

    pub(super) fn render_command_menu(&self, cx: &Context<Self>) -> AnyElement {
        let Some(menu) = &self.command_menu else {
            return div().into_any_element();
        };
        let colors = cx.theme().colors();
        let (kind, cwd) = self.session_kind_and_cwd(cx);
        let store = self.store.read(cx);
        let (loading, error, empty_message) = match menu.kind {
            TokenKind::Command => {
                let catalog = store.commands(kind, &cwd);
                (
                    catalog.is_none_or(|catalog| catalog.is_loading() && !catalog.loaded),
                    catalog.and_then(|catalog| catalog.error.clone()),
                    t!("No matching commands"),
                )
            }
            TokenKind::Skill => {
                let (skills, loaded) = self.available_skills(cx);
                let message = if loaded && skills.is_empty() {
                    t!("No skills found")
                } else {
                    t!("No matching skills")
                };
                (!loaded, None, message)
            }
            TokenKind::Mention => (false, None, t!("No matching files")),
        };
        let rows = self.rows(cx);
        let message_row = |text: SharedString, color: Hsla| {
            div()
                .px(px(12.))
                .py(px(10.))
                .text_size(ui(12.))
                .text_color(color)
                .child(text)
        };
        let row_elements: Vec<AnyElement> = rows
            .into_iter()
            .enumerate()
            .map(|(index, row)| {
                let selected = index == menu.selected;
                let detail = row.detail();
                let has_detail = !detail.is_empty();
                h_flex()
                    .id(("agent-completion", index))
                    .flex_none()
                    .gap(px(8.))
                    .px(px(8.))
                    .py(px(6.))
                    .rounded(px(MENU_ITEM_RADIUS))
                    .cursor_pointer()
                    .text_size(ui(13.))
                    .map(|this| {
                        if selected {
                            this.bg(selected_row(cx))
                        } else {
                            let hover = selected_row(cx);
                            this.hover(move |style| style.bg(hover))
                        }
                    })
                    .child(div().flex_none().size(px(16.)).child(row.icon(16., cx)))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(colors.text)
                            .map(|this| {
                                if has_detail {
                                    this.flex_none().max_w(relative(0.55))
                                } else {
                                    this.flex_1()
                                }
                            })
                            .child(row.label()),
                    )
                    .when(has_detail, |this| {
                        this.child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(ui(12.5))
                                .text_color(colors.text_muted)
                                .child(detail),
                        )
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.pick_command(index, window, cx)
                    }))
                    .into_any_element()
            })
            .collect();
        let empty = row_elements.is_empty();
        let card = popover_card(cx)
            .relative()
            .w_full()
            .max_h(px(320.))
            .bg(colors.elevated_surface_background.opacity(0.85))
            .child(
                canvas(
                    |_, _, _| (),
                    |bounds, (), window, _| {
                        window.paint_backdrop_blur(bounds, Corners::all(px(11.)), px(16.))
                    },
                )
                .absolute()
                .inset_0(),
            )
            .when_some(error, |this, error| {
                this.child(message_row(error, cx.theme().status().error))
            })
            .map(|this| {
                if empty && loading {
                    this.child(v_flex().py(px(4.)).gap(px(6.)).children((0..3usize).map(
                        |index| {
                            div()
                                .h(px(28.))
                                .rounded(px(6.))
                                .bg(ink(0.04, cx))
                                .with_animation(
                                    ("agent-completion-skeleton", index),
                                    Animation::new(Duration::from_millis(1400))
                                        .repeat()
                                        .with_easing(pulsating_between(0.35, 0.75)),
                                    |this, delta| this.opacity(delta),
                                )
                        },
                    )))
                } else if empty {
                    this.child(message_row(empty_message, colors.text_muted))
                } else {
                    this.child(
                        v_flex()
                            .id("agent-completion-list")
                            .max_h(px(310.))
                            .overflow_y_scroll()
                            .track_scroll(&self.command_scroll)
                            .gap(px(2.))
                            .children(row_elements),
                    )
                }
            });
        deferred(
            div()
                .absolute()
                .bottom_full()
                .left_0()
                .right_0()
                .pb(px(6.))
                .occlude()
                .child(card),
        )
        .with_priority(1)
        .into_any_element()
    }
}
