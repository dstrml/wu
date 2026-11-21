use super::{ChatView, ui};
use crate::{
    chat_style::{ink, popover_card},
    session::Entry,
};
use gpui::{
    Anchor, AnyElement, Context, IntoElement, ListOffset, ParentElement, SharedString, Styled,
    anchored, deferred, div, px,
};
use i18n::t;
use ui::prelude::*;

const MIN_CONTAINER_WIDTH: f32 = 768.;
const PREVIEW_PROMPT_CHARS: usize = 160;
const PREVIEW_REPLY_CHARS: usize = 200;
const TICK_SLOT: f32 = 10.;
const TICK_GAP: f32 = 3.;
const RAIL_VERTICAL_MARGIN: f32 = 24.;
const MAX_RAIL_TICKS: usize = 12;
const READING_INSET: f32 = 16.;

fn clip(text: &str, max: usize) -> String {
    let line = agent_harness::view::single_line(text);
    if line.chars().count() <= max {
        return line;
    }
    let mut clipped: String = line.chars().take(max - 1).collect();
    clipped.push('…');
    clipped
}

fn prompt_preview(entry: Option<&Entry>) -> String {
    match entry {
        Some(Entry::User {
            text, attachments, ..
        }) if text.trim().is_empty() && !attachments.is_empty() => {
            t!("Attached images").to_string()
        }
        Some(Entry::User { text, .. }) => clip(text, PREVIEW_PROMPT_CHARS),
        _ => String::new(),
    }
}

fn reply_preview(replies: &[Entry], cx: &gpui::App) -> Option<String> {
    replies.iter().find_map(|entry| match entry {
        Entry::Assistant { markdown, .. } => {
            let source = markdown.read(cx).source();
            (!source.trim().is_empty()).then(|| clip(source, PREVIEW_REPLY_CHARS))
        }
        _ => None,
    })
}

fn turn_for_tick(tick: usize, leading_turns: usize) -> usize {
    tick + leading_turns
}

fn tick_for_turn(turn: usize, leading_turns: usize) -> usize {
    turn.saturating_sub(leading_turns)
}

fn rail_slots(height: f32) -> usize {
    let usable = (height - 2. * RAIL_VERTICAL_MARGIN).max(TICK_SLOT);
    (((usable + TICK_GAP) / (TICK_SLOT + TICK_GAP)).floor() as usize).clamp(1, MAX_RAIL_TICKS)
}

fn tick_buckets(count: usize, capacity: usize) -> Vec<(usize, usize)> {
    if count == 0 {
        return Vec::new();
    }
    let capacity = capacity.clamp(1, count);
    (0..capacity)
        .map(|bucket| (bucket * count / capacity, (bucket + 1) * count / capacity))
        .collect()
}

impl ChatView {
    fn leading_turns(&self) -> usize {
        match self.transcript.turns.first() {
            Some(turn) if turn.user.is_none() => 1,
            _ => 0,
        }
    }

    fn jump_to_tick(&mut self, tick: usize, cx: &mut Context<Self>) {
        let turn = turn_for_tick(tick, self.leading_turns());
        if turn >= self.transcript.turns.len() {
            return;
        }
        self.list.scroll_to(ListOffset {
            item_ix: self.transcript.turn_rows(turn).start,
            offset_in_item: px(0.),
        });
        cx.notify();
    }

    fn active_tick(&self) -> Option<usize> {
        let reading_line = self.scroll_top() + px(READING_INSET + 0.5);
        let top_turn = self
            .transcript
            .turn_rows
            .partition_point(|first_row| self.list.offset_for_item(*first_row) <= reading_line)
            .checked_sub(1)?;
        Some(tick_for_turn(top_turn, self.leading_turns()))
    }

    pub(super) fn render_outline(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let container = self.container_bounds.get()?;
        if f32::from(container.size.width) < MIN_CONTAINER_WIDTH {
            return None;
        }
        let leading_turns = self.leading_turns();
        let tick_count = self.transcript.turns.len().saturating_sub(leading_turns);
        if tick_count < 2 {
            return None;
        }
        let colors = cx.theme().colors();
        let active = self.active_tick();
        let buckets = tick_buckets(tick_count, rail_slots(f32::from(container.size.height)));
        let active_bucket = active.and_then(|active| {
            buckets
                .iter()
                .position(|(start, end)| active >= *start && active < *end)
        });
        let hovered = self.outline_hover;
        let rest_color = ink(0.16, cx);
        let lit_color = colors.text.opacity(0.8);
        Some(
            v_flex()
                .absolute()
                .left(px(16.))
                .top_0()
                .bottom_0()
                .w(px(26.))
                .items_start()
                .justify_center()
                .gap(px(TICK_GAP))
                .children(buckets.into_iter().enumerate().map(|(bucket, (start, end))| {
                    let representative = active
                        .filter(|active| *active >= start && *active < end)
                        .unwrap_or(start);
                    let bucket_len = end - start;
                    let is_hovered = hovered == Some(bucket);
                    let is_active = active_bucket == Some(bucket);
                    let preview = is_hovered
                        .then(|| {
                            self.transcript
                                .turns
                                .get(turn_for_tick(representative, leading_turns))
                        })
                        .flatten()
                        .map(|turn| {
                            let entries = self.entries(cx);
                            let prompt =
                                prompt_preview(turn.user.and_then(|user| entries.get(user)));
                            let replies = entries.get(turn.items.clone()).unwrap_or_default();
                            (
                                SharedString::from(prompt),
                                reply_preview(replies, cx).map(SharedString::from),
                            )
                        });
                    div()
                        .id(("agent-outline", bucket))
                        .relative()
                        .h(px(TICK_SLOT))
                        .w_full()
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .child(
                            div()
                                .h(px(2.))
                                .w(px(if is_hovered { 20. } else { 12. }))
                                .rounded(px(1.))
                                .bg(if is_active || is_hovered {
                                    lit_color
                                } else {
                                    rest_color
                                }),
                        )
                        .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                            if *hovering {
                                this.outline_hover = Some(bucket);
                            } else if this.outline_hover == Some(bucket) {
                                this.outline_hover = None;
                            }
                            cx.notify();
                        }))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.jump_to_tick(representative, cx)
                        }))
                        .when_some(preview, |this, (preview_prompt, preview_reply)| {
                            this.child(deferred(
                                anchored()
                                    .anchor(Anchor::LeftCenter)
                                    .snap_to_window_with_margin(px(8.))
                                    .child(
                                        div().pl(px(26.)).child(
                                            popover_card(cx)
                                                .w(px(280.))
                                                .p(px(8.))
                                                .gap(px(6.))
                                                .child(
                                                    div()
                                                        .text_size(ui(12.))
                                                        .text_color(colors.text)
                                                        .child(preview_prompt),
                                                )
                                                .when_some(preview_reply, |this, reply| {
                                                    this.child(
                                                        div()
                                                            .text_size(ui(11.))
                                                            .text_color(colors.text_muted)
                                                            .child(reply),
                                                    )
                                                })
                                                .when(bucket_len > 1, |this| {
                                                    this.child(
                                                        div()
                                                            .text_size(ui(10.))
                                                            .text_color(colors.text_muted)
                                                            .child(t!(
                                                                "{count} prompts",
                                                                count = bucket_len
                                                            )),
                                                    )
                                                }),
                                        ),
                                    ),
                            ))
                        })
                }))
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AgentKind,
        session::tests::{feed_events, feed_user, new_store},
    };
    use agent_harness::AgentEvent;
    use gpui::{TestAppContext, WeakEntity};
    use project::FakeFs;

    #[gpui::test]
    async fn ticks_follow_the_messages_the_user_sent(cx: &mut TestAppContext) {
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
        let reply = |text: &str| vec![AgentEvent::TextDelta { text: text.into() }];
        feed_events(&session, reply("Before any message."), cx);
        for turn in 0..3 {
            feed_user(&session, &format!("question {turn}"), cx);
            feed_events(&session, reply(&format!("Answer {turn}.")), cx);
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
        chat.update(cx, |chat, cx| {
            assert_eq!(chat.leading_turns(), 1);
            assert_eq!(chat.transcript.turns.len(), 4);
            chat.jump_to_tick(1, cx);
            let second_question = chat.transcript.turn_rows(2).start;
            assert_eq!(chat.list.logical_scroll_top().item_ix, second_question);
            assert_eq!(chat.transcript.entry_for_row(second_question), Some(3));

            let entries = chat.entries(cx);
            let turn = &chat.transcript.turns[2];
            assert_eq!(
                prompt_preview(turn.user.and_then(|user| entries.get(user))),
                "question 1"
            );
            let replies = entries.get(turn.items.clone()).unwrap_or_default();
            assert_eq!(reply_preview(replies, cx).as_deref(), Some("Answer 1."));
        });
        assert_eq!(tick_for_turn(0, 1), 0);
        assert_eq!(tick_for_turn(3, 1), 2);
    }
}
