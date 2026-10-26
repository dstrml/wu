use collections::{HashMap, HashSet};
use command_palette;
use editor::{CompletionProvider, Editor};
use fuzzy::StringMatchCandidate;
use gpui::{Context, Entity, SharedString, Window};
use language::{self, ToOffset};
use project::{self, CompletionDisplayOptions};

pub struct ActionCompletionProvider {
    action_names: Vec<&'static str>,
    humanized_names: HashMap<&'static str, SharedString>,
    localized_names: HashMap<&'static str, SharedString>,
}

impl ActionCompletionProvider {
    pub fn new(
        action_names: Vec<&'static str>,
        humanized_names: HashMap<&'static str, SharedString>,
    ) -> Self {
        let localized_names = action_names
            .iter()
            .filter_map(|&name| {
                let localized_name = command_palette::localized_action_name(name)?;
                Some((name, localized_name.into()))
            })
            .collect();
        Self {
            action_names,
            humanized_names,
            localized_names,
        }
    }
}

impl CompletionProvider for ActionCompletionProvider {
    fn completions(
        &self,
        buffer: &Entity<language::Buffer>,
        buffer_position: language::Anchor,
        _trigger: editor::CompletionContext,
        _window: &mut Window,
        cx: &mut Context<Editor>,
    ) -> gpui::Task<anyhow::Result<Vec<project::CompletionResponse>>> {
        let buffer = buffer.read(cx);
        let query_byte_length: usize = buffer
            .reversed_chars_at(buffer_position)
            .take_while(|&char| is_action_name_char(char))
            .map(char::len_utf8)
            .sum();

        let start_anchor = buffer.anchor_before(
            buffer_position
                .to_offset(&buffer)
                .saturating_sub(query_byte_length),
        );

        let replace_range = start_anchor..buffer_position;
        let snapshot = buffer.text_snapshot();
        let query: String = snapshot.text_for_range(replace_range.clone()).collect();
        let normalized_query = command_palette::normalize_action_query(&query);

        let mut candidates = Vec::with_capacity(self.action_names.len());
        for (ix, &name) in self.action_names.iter().enumerate() {
            let humanized = self
                .humanized_names
                .get(name)
                .cloned()
                .unwrap_or_else(|| name.into());
            candidates.push(StringMatchCandidate::new(ix, &humanized));
            if let Some(localized_name) = self.localized_names.get(name) {
                candidates.push(StringMatchCandidate::new(ix, localized_name));
            }
        }

        let executor = cx.background_executor().clone();
        let executor_for_fuzzy = executor.clone();
        let action_names = self.action_names.clone();
        let humanized_names = self.humanized_names.clone();
        let localized_names = self.localized_names.clone();

        executor.spawn(async move {
            let mut matches = fuzzy::match_strings(
                &candidates,
                &normalized_query,
                true,
                true,
                candidates.len(),
                &Default::default(),
                executor_for_fuzzy,
            )
            .await;
            let mut matched_actions = HashSet::default();
            matches.retain(|string_match| matched_actions.insert(string_match.candidate_id));

            let completions: Vec<project::Completion> = matches
                .iter()
                .take(50)
                .map(|m| {
                    let action_name = action_names[m.candidate_id];
                    let label = localized_names
                        .get(action_name)
                        .or_else(|| humanized_names.get(action_name))
                        .cloned()
                        .unwrap_or_else(|| action_name.into());

                    project::Completion {
                        replace_range: replace_range.clone(),
                        label: language::CodeLabel::plain(label.to_string(), None),
                        new_text: action_name.to_string(),
                        documentation: None,
                        source: project::CompletionSource::Custom,
                        icon_path: None,
                        icon_color: None,
                        match_start: None,
                        snippet_deduplication_key: None,
                        insert_text_mode: None,
                        confirm: None,
                        group: None,
                    }
                })
                .collect();

            Ok(vec![project::CompletionResponse {
                completions,
                display_options: CompletionDisplayOptions {
                    dynamic_width: true,
                },
                is_incomplete: true,
            }])
        })
    }

    fn filter_completions(&self) -> bool {
        false
    }

    fn sort_completions(&self) -> bool {
        false
    }

    fn is_completion_trigger(
        &self,
        _buffer: &Entity<language::Buffer>,
        _position: language::Anchor,
        text: &str,
        _trigger_in_words: bool,
        _cx: &mut Context<Editor>,
    ) -> bool {
        text.chars().last().is_some_and(is_action_name_char)
    }
}

fn is_action_name_char(char: char) -> bool {
    char.is_alphanumeric() || char == '_' || char == ':'
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;
    use crate::tests::SimplifiedChinese;
    use editor::{actions::ConfirmCompletion, code_context_menus::CodeContextMenu};
    use gpui::{Focusable as _, TestAppContext, VisualTestContext};
    use workspace::AppState;

    fn action_name_editor(cx: &mut TestAppContext) -> (Entity<Editor>, &mut VisualTestContext) {
        cx.update(|cx| {
            let _state = AppState::test(cx);
            editor::init(cx);
        });
        let action_names = vec!["editor::Backspace", "editor::Delete", "workspace::Save"];
        let humanized_names = action_names
            .iter()
            .map(|&name| (name, command_palette::humanize_action_name(name).into()))
            .collect();
        let (editor, cx) = cx.add_window_view(|window, cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_completion_provider(Some(Rc::new(ActionCompletionProvider::new(
                action_names,
                humanized_names,
            ))));
            editor
        });
        editor.update_in(cx, |editor, window, cx| {
            window.focus(&editor.focus_handle(cx), cx)
        });
        (editor, cx)
    }

    fn type_text(editor: &Entity<Editor>, text: &str, cx: &mut VisualTestContext) {
        for char in text.chars() {
            editor.update_in(cx, |editor, window, cx| {
                editor.handle_input(&char.to_string(), window, cx)
            });
            cx.run_until_parked();
        }
    }

    fn visible_labels(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> Vec<String> {
        editor.read_with(cx, |editor, _| {
            match editor.context_menu().borrow().as_ref() {
                Some(CodeContextMenu::Completions(menu)) => {
                    let completions = menu.completions.borrow();
                    menu.entries
                        .borrow()
                        .iter()
                        .filter_map(|entry| entry.as_match())
                        .map(|string_match| {
                            completions[string_match.candidate_id].label.text.clone()
                        })
                        .collect()
                }
                _ => Vec::new(),
            }
        })
    }

    #[gpui::test]
    async fn test_accepting_completion_replaces_chinese_query(cx: &mut TestAppContext) {
        let _simplified_chinese = SimplifiedChinese::enable();
        let (editor, cx) = action_name_editor(cx);

        type_text(&editor, "退格", cx);
        assert_eq!(visible_labels(&editor, cx), ["编辑器: 退格"]);

        let confirm_task = editor.update_in(cx, |editor, window, cx| {
            editor.confirm_completion(&ConfirmCompletion::default(), window, cx)
        });
        if let Some(confirm_task) = confirm_task {
            confirm_task.await.unwrap();
        }
        assert_eq!(
            editor.read_with(cx, |editor, cx| editor.text(cx)),
            "editor::Backspace"
        );
    }

    #[gpui::test]
    async fn test_english_query_keeps_chinese_labelled_completion(cx: &mut TestAppContext) {
        let _simplified_chinese = SimplifiedChinese::enable();
        let (editor, cx) = action_name_editor(cx);

        type_text(&editor, "back", cx);
        assert_eq!(visible_labels(&editor, cx), ["编辑器: 退格"]);
    }
}
