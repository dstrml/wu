use super::{
    AgentPanel, HARNESS_ICON_SIZE, LIST_GAP, LIST_PAD_TOP, SPACE_SM, chat_roots, faded_label,
    format_time_ago, glyph, open_chat,
};
use crate::{
    chat_style::{text_faint, ui},
    chat_view::ChatView,
    session::{SavedMessage, SessionSummary},
};
use collections::HashMap;
use gpui::{AnyElement, Context, SharedString, Window, div, px};
use i18n::t;
use time::OffsetDateTime;
use ui::{IconName, Tooltip, prelude::*};
use workspace::{Toast, notifications::NotificationId};

const PREVIEW_LINES: usize = 3;
const PREVIEW_CHARS: usize = 240;

fn preview(text: &str) -> String {
    let mut words = text.split_whitespace();
    let mut preview = String::new();
    for word in words.by_ref() {
        if preview.chars().count() + word.chars().count() > PREVIEW_CHARS {
            preview.push('…');
            return preview;
        }
        if !preview.is_empty() {
            preview.push(' ');
        }
        preview.push_str(word);
    }
    preview
}

impl AgentPanel {
    pub(super) fn open_saved_message(
        &mut self,
        message: SavedMessage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let task = self
            .store
            .update(cx, |store, cx| store.open_session(&message.chat_id, cx));
        let workspace = self.workspace.clone();
        cx.spawn_in(window, async move |_, cx| {
            let session = task.await?;
            workspace.update_in(cx, |workspace, window, cx| {
                open_chat(workspace, session.clone(), window, cx);
                let view = workspace.items_of_type::<ChatView>(cx).find(|view| {
                    let view = view.read(cx);
                    view.session() == &session && !view.is_subagent()
                });
                let found = view.is_some_and(|view| {
                    view.update(cx, |view, cx| {
                        view.reveal_message(message.entry, &message.text, cx)
                    })
                });
                if !found {
                    workspace.show_toast(
                        Toast::new(
                            NotificationId::unique::<SavedMessage>(),
                            t!("This message is no longer in its chat.").to_string(),
                        )
                        .autohide(),
                        cx,
                    );
                }
            })
        })
        .detach_and_log_err(cx);
    }

    pub(super) fn render_saved_messages(&self, cx: &Context<Self>) -> AnyElement {
        let store = self.store.read(cx);
        let chats: HashMap<String, SessionSummary> = store
            .sessions_in(&chat_roots(&self.project, cx), cx)
            .into_iter()
            .map(|session| (session.id.clone(), session))
            .collect();
        let query = self.search.read(cx).text(cx).trim().to_lowercase();
        let mut messages: Vec<(SavedMessage, SessionSummary)> = store
            .saved_messages()
            .iter()
            .filter_map(|message| Some((message.clone(), chats.get(&message.chat_id)?.clone())))
            .filter(|(message, chat)| {
                query.is_empty()
                    || message.text.to_lowercase().contains(&query)
                    || chat.title.to_lowercase().contains(&query)
            })
            .collect();
        messages.sort_by_key(|(message, _)| std::cmp::Reverse(message.saved_at));
        if messages.is_empty() {
            return div()
                .px(px(SPACE_SM * 2.))
                .pt(px(LIST_PAD_TOP + SPACE_SM))
                .text_size(ui(12.))
                .text_color(text_faint(cx))
                .child(if query.is_empty() {
                    t!("No saved messages yet. Hover a message and click the bookmark to save it.")
                } else {
                    t!("No matching saved messages")
                })
                .into_any_element();
        }
        let now = OffsetDateTime::now_utc().unix_timestamp();
        v_flex()
            .id("agent-saved-messages")
            .size_full()
            .overflow_y_scroll()
            .px(px(SPACE_SM))
            .pt(px(LIST_PAD_TOP))
            .pb(px(SPACE_SM))
            .gap(px(LIST_GAP))
            .children(
                messages
                    .into_iter()
                    .enumerate()
                    .map(|(position, (message, chat))| {
                        self.render_saved_message(position, message, &chat, now, cx)
                    }),
            )
            .into_any_element()
    }

    fn render_saved_message(
        &self,
        position: usize,
        message: SavedMessage,
        chat: &SessionSummary,
        now: i64,
        cx: &Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors();
        let subline = colors.text_muted.opacity(0.7);
        let group = SharedString::from(format!("agent-saved-{position}"));
        let author = if message.from_agent {
            SharedString::new_static(chat.kind.label())
        } else {
            t!("You")
        };
        let details = format!(
            "{author} · {} · {}",
            chat.title,
            format_time_ago(message.saved_at, now)
        );
        let remove = {
            let message = message.clone();
            div()
                .id(SharedString::from(format!("agent-saved-remove-{position}")))
                .flex_none()
                .size(px(18.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5.))
                .cursor_pointer()
                .visible_on_hover(group.clone())
                .hover(|style| style.bg(colors.element_hover))
                .tooltip(Tooltip::text(t!("Remove from saved")))
                .child(glyph(IconName::AgentClose, 11., colors.text_muted))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.store.update(cx, |store, cx| {
                        store.toggle_saved_message(
                            &message.chat_id,
                            message.entry,
                            message.text.clone(),
                            message.from_agent,
                            cx,
                        )
                    });
                }))
        };
        v_flex()
            .id(SharedString::from(format!("agent-saved-row-{position}")))
            .group(group)
            .gap(px(4.))
            .rounded(px(8.))
            .px(px(SPACE_SM))
            .py(px(8.))
            .text_color(colors.text.opacity(0.8))
            .cursor_pointer()
            .hover(|style| style.bg(colors.element_hover).text_color(colors.text))
            .child(
                h_flex()
                    .items_start()
                    .gap(px(SPACE_SM))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(ui(13.))
                            .line_height(px(18.))
                            .line_clamp(PREVIEW_LINES)
                            .text_ellipsis()
                            .child(preview(&message.text)),
                    )
                    .child(remove),
            )
            .child(
                h_flex()
                    .gap(px(4.))
                    .child(glyph(
                        if message.from_agent {
                            IconName::AgentBot
                        } else {
                            IconName::AgentChatRoundLine
                        },
                        HARNESS_ICON_SIZE - 2.,
                        subline,
                    ))
                    .child(faded_label(
                        SharedString::from(format!("agent-saved-details-{position}")),
                        true,
                        div()
                            .text_size(ui(11.))
                            .line_height(px(14.))
                            .text_color(subline)
                            .child(details),
                    )),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.open_saved_message(message.clone(), window, cx)
            }))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_join_lines_and_stop_at_a_word_boundary() {
        assert_eq!(preview("  Fix the\n\nlogin   bug "), "Fix the login bug");
        let long = "word ".repeat(100);
        let shown = preview(&long);
        assert!(shown.ends_with("word…"), "{shown}");
        assert!(shown.chars().count() <= PREVIEW_CHARS + 2);
    }
}
