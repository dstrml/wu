use super::*;
use crate::{
    AgentKind,
    chat_style::{composer_surface, icon, icon_size_px, tracked_upper},
    session::AgentUpdateStatus,
};
use i18n::t;

const NEW_CHAT_TEXTAREA_MIN: f32 = 76.;
const TEXTAREA_MAX: f32 = 260.;
const ACTIONS_ROW_HEIGHT: f32 = 42.;
const ACTION_BUTTON_SIZE: f32 = 28.;
const ACTION_PRIMARY_GAP: f32 = 8.;
const MODEL_CHIP_PADDING: f32 = 6.;
const ACTION_INSET: f32 = 12.;
const MODEL_CHIP_MAX_FRACTION: f32 = 0.45;
const TRAY_RADIUS: f32 = 16.;
pub(super) const TRAY_SIDE_INSET: f32 = 16.;
pub(super) const TRAY_COMPOSER_OVERLAP: f32 = 18.;
const QUESTION_LINE_HEIGHT: f32 = 20.;
const QUESTION_COLLAPSED_LINES: usize = 4;
const QUESTION_COLLAPSE_CHARS: usize = 160;
const QUESTION_EXPANDED_FRACTION: f32 = 0.4;
const QUESTION_EXPANDED_FALLBACK: f32 = 280.;
const QUESTION_OPTIONS_FRACTION: f32 = 0.3;

pub(super) fn tray_surface(cx: &App) -> gpui::Div {
    div()
        .rounded_t(px(TRAY_RADIUS))
        .bg(composer_surface(cx))
        .border_1()
        .border_color(cx.theme().colors().border)
        .shadow_lg()
        .px(px(4.))
        .pt(px(4.))
        .pb(px(TRAY_COMPOSER_OVERLAP))
        .flex()
        .flex_col()
}

impl ChatView {
    pub(super) fn render_model_chip(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let session = self.session.read(cx);
        let kind = session.kind();
        let settings = session.settings();
        let selection = Selection::resolve(self.store.read(cx).models(kind), settings);
        let model_label = selection.model_label(settings, kind);
        let effort = selection.reasoning.map(view::reasoning_label);
        let fast = selection.fast_on();
        let colors = cx.theme().colors();
        let brand = match kind {
            AgentKind::Codex | AgentKind::Opencode => agent_icon(kind).color(Color::Muted),
            AgentKind::Claude => agent_icon(kind),
        }
        .size(icon_size_px(16., window));
        let session = self.session.clone();
        let store = self.store.clone();
        PopoverMenu::new(SharedString::from(format!(
            "agent-model-picker-{}",
            cx.entity_id()
        )))
        .trigger(
            Chip::new(
                "agent-model-chip",
                16.,
                h_flex()
                    .h(px(32.))
                    .max_w(px(248.))
                    .min_w_0()
                    .px(px(MODEL_CHIP_PADDING))
                    .gap(px(6.))
                    .text_size(ui(12.))
                    .font_weight(FontWeight::NORMAL)
                    .child(brand)
                    .child(div().min_w_0().truncate().child(model_label))
                    .when_some(effort, |this, effort| {
                        this.child(
                            div()
                                .map(|mut suffix| {
                                    suffix.style().flex_shrink = Some(1000.);
                                    suffix
                                })
                                .min_w_0()
                                .truncate()
                                .text_color(colors.text_muted.opacity(0.7))
                                .child(effort),
                        )
                    })
                    .when(fast, |this| {
                        this.child(icon(IconName::AgentFastFilled, px(13.), accent(cx)))
                    })
                    .child(icon(IconName::AgentArrowDown, px(12.), colors.text_muted)),
            )
            .shrinkable()
            .text_colors(colors.text.opacity(0.9), colors.text),
        )
        .menu(move |window, cx| {
            let session = session.clone();
            let store = store.clone();
            Some(cx.new(|cx| ModelPicker::new(session, store, window, cx)))
        })
        .with_handle(self.model_picker.clone())
        .anchor(Anchor::BottomRight)
        .attach(Anchor::TopRight)
        .offset(point(px(0.), px(-6.)))
    }

    pub(super) fn render_send_button(&self, cx: &Context<Self>) -> AnyElement {
        let colors = cx.theme().colors();
        let is_working = self.session.read(cx).is_working();
        let has_text = self.staging_count == 0
            && (!self.composer.read(cx).text(cx).trim().is_empty()
                || !self.attachments.is_empty());
        let focus_handle = self.composer.focus_handle(cx);
        let plate = page(cx);
        let circle = |id: &'static str| {
            div()
                .id(id)
                .size(px(SEND_BUTTON_SIZE))
                .flex_none()
                .rounded_full()
                .bg(colors.text)
                .flex()
                .items_center()
                .justify_center()
        };
        if is_working && !has_text {
            return circle("agent-stop")
                .cursor_pointer()
                .hover(|style| style.opacity(0.85))
                .tooltip(move |_, cx| Tooltip::for_action_in(t!("Stop"), &Stop, &focus_handle, cx))
                .on_click(cx.listener(|this, _, window, cx| this.stop(&Stop, window, cx)))
                .child(div().size(px(11.)).rounded(px(3.)).bg(plate))
                .into_any_element();
        }
        circle("agent-send")
            .when(!has_text, |this| this.opacity(0.35))
            .when(has_text, |this| {
                this.cursor_pointer()
                    .hover(|style| style.opacity(0.85))
                    .on_click(cx.listener(|this, _, window, cx| this.send(&Send, window, cx)))
            })
            .tooltip(move |_, cx| {
                Tooltip::for_action_in(
                    if is_working {
                        t!("Queue message")
                    } else {
                        t!("Send message")
                    },
                    &Send,
                    &focus_handle,
                    cx,
                )
            })
            .child(icon(IconName::AgentSend, px(14.), plate))
            .into_any_element()
    }

    fn render_attach_button(&self, dictating: bool, cx: &Context<Self>) -> impl IntoElement {
        let hover = ink(0.10, cx);
        let muted = cx.theme().colors().text_muted;
        div()
            .id("agent-attach")
            .size(px(ACTION_BUTTON_SIZE))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .tooltip(Tooltip::text(if dictating {
                t!("Cancel dictation")
            } else {
                t!("Attach images")
            }))
            .child(if dictating {
                icon(IconName::AgentClose, px(16.), muted)
            } else {
                icon(IconName::Plus, px(18.), muted)
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                if dictating {
                    this.cancel_dictation(cx);
                } else {
                    this.pick_images(window, cx);
                }
            }))
    }

    pub(super) fn render_pill(
        &self,
        radius: f32,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors();
        let show_commands = self.command_menu.is_some()
            && self.composer.focus_handle(cx).is_focused(window);
        let mut key_context = KeyContext::default();
        key_context.add("AgentComposer");
        if AgentChatSettings::get_global(cx).send_with == AgentChatSendKey::ModifierEnter {
            key_context.add("send_with_modifier");
        }
        if show_commands {
            key_context.add("command_menu");
        }
        let new_chat = {
            let session = self.session.read(cx);
            session.entries().is_empty() && !session.is_working()
        };
        let dictating = self.dictation_active();
        let microphone = self.render_dictation_button(cx);
        let voice_track = microphone
            .is_some()
            .then(|| self.render_voice_track(cx))
            .flatten();
        let model_picker = voice_track.is_none().then(|| {
            div()
                .min_w_0()
                .max_w(gpui::relative(MODEL_CHIP_MAX_FRACTION))
                .child(self.render_model_chip(window, cx))
        });
        let trailing = h_flex()
            .min_w_0()
            .gap(px(ACTION_PRIMARY_GAP))
            .children(voice_track)
            .children(microphone)
            .child(self.render_send_button(cx));
        let textarea_min = if new_chat { NEW_CHAT_TEXTAREA_MIN } else { 0. };
        let pill = div()
            .key_context(key_context)
            .relative()
            .capture_action(cx.listener(|this, _: &editor::actions::Paste, _, cx| {
                if this.paste_images(cx) {
                    cx.stop_propagation();
                }
            }))
            .when(show_commands, |this| this.child(self.render_command_menu(cx)))
            .w_full()
            .rounded(px(radius))
            .border_1()
            .border_color(colors.border)
            .bg(composer_surface(cx))
            .shadow_lg()
            .flex()
            .flex_col()
            .children(self.render_attachment_strip(cx))
            .child(
                div()
                    .flex_none()
                    .px(px(16.))
                    .pt(px(16.))
                    .pb(px(10.))
                    .min_h(px(textarea_min))
                    .max_h(px(TEXTAREA_MAX))
                    .child(self.composer.clone()),
            )
            .child(
                h_flex()
                    .h(px(ACTIONS_ROW_HEIGHT))
                    .pt(px(2.))
                    .pb(px(8.))
                    .px(px(ACTION_INSET))
                    .gap(px(ACTION_PRIMARY_GAP))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(ACTION_PRIMARY_GAP))
                            .child(self.render_attach_button(dictating, cx))
                            .children(model_picker),
                    )
                    .child(trailing),
            );
        v_flex()
            .w_full()
            .gap(px(8.))
            .child(pill)
            .children(self.render_dictation_status(cx))
            .into_any_element()
    }

    pub(super) fn footer_label_shell(icon_name: IconName, cx: &App) -> gpui::Div {
        let color = cx.theme().colors().text_muted.opacity(0.6);
        h_flex()
            .h(px(20.))
            .min_w_0()
            .px(px(8.))
            .gap(px(6.))
            .text_size(ui(12.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(color)
            .child(icon(icon_name, px(12.), color))
    }

    pub(super) fn render_agent_update(&self, cx: &App) -> Option<AnyElement> {
        let kind = self.session.read(cx).kind();
        let status = self.store.read(cx).agent_update(kind)?.clone();
        let label = |icon_name: IconName, text: SharedString| {
            Self::footer_label_shell(icon_name, cx)
                .max_w(px(280.))
                .child(div().min_w_0().truncate().child(text))
        };
        Some(match status {
            AgentUpdateStatus::Updating => label(
                IconName::AgentRestart,
                t!("Updating {agent}…", agent = kind.label()),
            )
            .with_animation(
                "agent-update-pulse",
                Animation::new(Duration::from_millis(1400))
                    .repeat()
                    .with_easing(gpui::pulsating_between(0.5, 1.)),
                |this, delta| this.opacity(delta),
            )
            .into_any_element(),
            AgentUpdateStatus::Finished(message) => {
                label(IconName::AgentCheck, message).into_any_element()
            }
            AgentUpdateStatus::Failed { message, reason } => {
                label(IconName::AgentDangerTriangle, message)
                    .id("agent-update-failed")
                    .tooltip(Tooltip::text(reason))
                    .into_any_element()
            }
        })
    }

    pub(super) fn render_move_to_panel(&self, cx: &Context<Self>) -> Option<impl IntoElement> {
        if self.docked || self.subagent.is_some() {
            return None;
        }
        let color = cx.theme().colors().text_muted.opacity(0.8);
        Some(
            Chip::new(
                "agent-move-to-panel",
                6.,
                div()
                    .size(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(IconName::Split, px(14.), color)),
            )
            .hover_background(ink(0.05, cx))
            .tooltip(t!("Move to Side Panel"))
            .on_click(cx.listener(|this, _, window, cx| {
                this.move_to_panel(&crate::MoveChatToPanel, window, cx)
            })),
        )
    }

    pub(super) fn render_footer(&self, cx: &Context<Self>) -> impl IntoElement {
        let session = self.session.read(cx);
        let kind = session.kind();
        let context = session.context();
        let context_fraction = context
            .and_then(|context| context.fraction())
            .map(|fraction| fraction as f32);
        let usage_fraction = self
            .store
            .read(cx)
            .plan_usage(kind)
            .and_then(|state| state.usage.as_ref())
            .and_then(|usage| usage.used_fraction());
        let store = self.store.clone();
        let ring_hover = ink(0.05, cx);
        h_flex()
            .h(px(24.))
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .pl(px(10.))
                    .children(self.render_agent_update(cx)),
            )
            .children(self.render_background_indicator(cx))
            .child(
                h_flex()
                    .flex_none()
                    .pl(px(4.))
                    .pr(px(10.))
                    .gap(px(4.))
                    .when(kind.has_plan_usage(), |this| {
                        this.child(
                            PopoverMenu::new(SharedString::from(format!(
                                "agent-plan-usage-{}",
                                cx.entity_id()
                            )))
                            .trigger(
                                Chip::new(
                                    "agent-plan-usage-chip",
                                    6.,
                                    usage_chip(usage_fraction, cx),
                                )
                                .hover_background(ring_hover),
                            )
                            .menu(move |_, cx| {
                                let store = store.clone();
                                Some(cx.new(|cx| UsageCard::new(store, kind, cx)))
                            })
                            .anchor(Anchor::BottomRight)
                            .attach(Anchor::TopRight)
                            .offset(point(px(0.), px(-6.))),
                        )
                    })
                    .child(
                        PopoverMenu::new(SharedString::from(format!(
                            "agent-context-usage-{}",
                            cx.entity_id()
                        )))
                        .trigger(
                            Chip::new(
                                "agent-context-chip",
                                6.,
                                context_chip(context_fraction, cx),
                            )
                            .hover_background(ring_hover),
                        )
                        .menu(move |_, cx| Some(cx.new(|cx| ContextCard::new(context, cx))))
                        .anchor(Anchor::BottomRight)
                        .attach(Anchor::TopRight)
                        .offset(point(px(0.), px(-6.))),
                    )
                    .children(self.render_move_to_panel(cx)),
            )
    }

    pub(super) fn render_status_strip(&self, cx: &Context<Self>) -> impl IntoElement {
        let failed = self.session.read(cx).run_failed();
        h_flex()
            .h(px(24.))
            .px(px(24.))
            .text_size(ui(11.))
            .when(failed, |this| {
                this.text_color(cx.theme().status().error)
                    .child(t!("Run failed"))
            })
    }

    pub(super) fn render_scroll_button(&self, cx: &Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let hover_background = colors.elevated_surface_background.blend(ink(0.08, cx));
        let hover_border = colors.border.blend(ink(0.12, cx));
        h_flex()
            .absolute()
            .bottom(px(6.))
            .left_0()
            .right(px(10.))
            .justify_center()
            .child(
                h_flex()
                    .id("agent-scroll-to-bottom")
                    .h(px(30.))
                    .pl(px(11.))
                    .pr(px(13.))
                    .gap(px(6.))
                    .rounded_full()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.elevated_surface_background)
                    .shadow_md()
                    .cursor_pointer()
                    .text_size(ui(13.))
                    .hover(move |style| style.bg(hover_background).border_color(hover_border))
                    .child(div().text_color(colors.text_muted).child("↓"))
                    .child(div().text_color(colors.text).child(t!("Scroll to bottom")))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_scroll_button = false;
                        this.list.set_follow_mode(gpui::FollowMode::Tail);
                        cx.notify();
                    })),
            )
    }

    pub(super) fn pick_option(
        &mut self,
        pending_id: SharedString,
        question_id: String,
        option: String,
        multi_select: bool,
        question_count: usize,
        cx: &mut Context<Self>,
    ) {
        let labels = self.selected_options.entry(question_id).or_default();
        if multi_select {
            if let Some(position) = labels.iter().position(|label| *label == option) {
                labels.remove(position);
            } else {
                labels.push(option);
            }
            cx.notify();
            return;
        }
        *labels = vec![option];
        cx.notify();
        self._auto_advance = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(AUTO_ADVANCE).await;
            this.update(cx, |this, cx| {
                let page = this
                    .question_pages
                    .get(pending_id.as_ref())
                    .copied()
                    .unwrap_or(0);
                if page + 1 < question_count {
                    this.question_pages.insert(pending_id.to_string(), page + 1);
                    cx.notify();
                } else {
                    this.submit_answers(&pending_id, cx);
                }
            })
            .ok();
        });
    }

    pub(super) fn submit_answer(&mut self, _: &SubmitAnswer, _: &mut Window, cx: &mut Context<Self>) {
        let Some(pending) = self.session.read(cx).pending_questions().first() else {
            return;
        };
        if pending.is_permission() {
            return;
        }
        let pending_id = pending.id().to_string();
        let count = pending.questions.len();
        let page_index = self
            .question_pages
            .get(&pending_id)
            .copied()
            .unwrap_or(0)
            .min(count.saturating_sub(1));
        let Some(question) = pending.questions.get(page_index) else {
            return;
        };
        if self.answer_labels(question, cx).is_empty() {
            return;
        }
        self.advance_question(&pending_id, page_index, count, cx);
    }

    fn advance_question(
        &mut self,
        pending_id: &str,
        page_index: usize,
        count: usize,
        cx: &mut Context<Self>,
    ) {
        if page_index + 1 >= count {
            self.submit_answers(pending_id, cx);
        } else {
            self.question_pages
                .insert(pending_id.to_string(), page_index + 1);
            cx.notify();
        }
    }

    pub(super) fn submit_answers(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(pending) = self
            .session
            .read(cx)
            .pending_questions()
            .iter()
            .find(|pending| pending.id() == id)
        else {
            return;
        };
        let answers: Vec<UserInputAnswer> = pending
            .questions
            .iter()
            .map(|question| UserInputAnswer {
                question_id: question.id.clone(),
                labels: self.answer_labels(question, cx),
            })
            .collect();
        for answer in &answers {
            self.selected_options.remove(&answer.question_id);
        }
        self.question_pages.remove(id);
        self.session
            .update(cx, |session, cx| session.answer(id, answers, cx));
    }

    pub(super) fn render_option_row(
        &self,
        key: SharedString,
        number: usize,
        label: AnyElement,
        picked: bool,
        cx: &App,
    ) -> gpui::Stateful<gpui::Div> {
        let colors = cx.theme().colors();
        let hover = ink(0.06, cx);
        h_flex()
            .id(ElementId::Name(key))
            .px(px(14.))
            .py(px(10.))
            .gap(px(12.))
            .rounded(px(12.))
            .border_1()
            .border_color(if picked {
                ink(0.16, cx)
            } else {
                gpui::transparent_black()
            })
            .bg(if picked {
                ink(0.09, cx)
            } else {
                ink(0.025, cx)
            })
            .when(!picked, |this| this.hover(move |style| style.bg(hover)))
            .cursor_pointer()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(ui(13.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if picked {
                        colors.text
                    } else {
                        colors.text.opacity(0.9)
                    })
                    .child(label),
            )
            .when(number <= 9, |this| {
                this.child(
                    div()
                        .flex_none()
                        .size(px(22.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.))
                        .bg(if picked { ink(0.16, cx) } else { ink(0.05, cx) })
                        .text_size(ui(11.))
                        .text_color(if picked {
                            colors.text
                        } else {
                            colors.text_muted.opacity(0.6)
                        })
                        .child(number.to_string()),
                )
            })
    }

    fn render_question_text(&self, id: &str, text: &str, cx: &Context<Self>) -> AnyElement {
        let colors = cx.theme().colors();
        let collapsible = text.lines().count() > QUESTION_COLLAPSED_LINES
            || text.chars().count() > QUESTION_COLLAPSE_CHARS;
        let expanded = self.expanded_questions.contains(id);
        let expanded_max_height = self
            .root_bounds
            .get()
            .map_or(px(QUESTION_EXPANDED_FALLBACK), |bounds| {
                bounds.size.height * QUESTION_EXPANDED_FRACTION
            });
        let toggle_group: SharedString = format!("agent-question-toggle-{id}").into();
        let toggle_id = id.to_string();
        v_flex()
            .mt(px(6.))
            .text_size(ui(15.))
            .line_height(px(QUESTION_LINE_HEIGHT))
            .font_weight(FontWeight::MEDIUM)
            .text_color(colors.text)
            // Character counts can't predict wrapping, so the text scrolls whatever its length.
            .child(
                div()
                    .id(SharedString::from(format!("agent-question-text-{id}")))
                    .max_h(if collapsible && !expanded {
                        px(QUESTION_LINE_HEIGHT * QUESTION_COLLAPSED_LINES as f32)
                    } else {
                        expanded_max_height
                    })
                    .overflow_y_scroll()
                    .child(text.to_string()),
            )
            .when(collapsible, |this| {
                this.child(
                    h_flex()
                        .id(SharedString::from(format!("agent-question-expand-{id}")))
                        .group(toggle_group.clone())
                        .mt(px(6.))
                        .gap(px(5.))
                        .text_size(ui(13.))
                        .font_weight(FontWeight::NORMAL)
                        .text_color(colors.text_muted)
                        .cursor_pointer()
                        .hover(|style| style.text_color(colors.text))
                        .child(if expanded {
                            t!("Show less")
                        } else {
                            t!("Show all")
                        })
                        .child(
                            super::icon(
                                if expanded {
                                    IconName::AgentArrowUp
                                } else {
                                    IconName::AgentArrowDown
                                },
                                12.,
                            )
                            .text_color(colors.text_muted)
                            .group_hover(toggle_group, |style| style.text_color(colors.text)),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !this.expanded_questions.remove(&toggle_id) {
                                this.expanded_questions.insert(toggle_id.clone());
                            }
                            cx.notify();
                        })),
                )
            })
            .into_any_element()
    }

    pub(super) fn render_question_panel(&self, pending: &PendingQuestion, cx: &Context<Self>) -> AnyElement {
        let colors = cx.theme().colors();
        let pending_id: SharedString = pending.id().to_string().into();
        let count = pending.questions.len();
        let page_index = self
            .question_pages
            .get(pending_id.as_ref())
            .copied()
            .unwrap_or(0)
            .min(count.saturating_sub(1));
        let Some(question) = pending.questions.get(page_index) else {
            return div().into_any_element();
        };
        let is_permission = pending.is_permission();
        let selected = self
            .selected_options
            .get(&question.id)
            .cloned()
            .unwrap_or_default();
        let typed_editor = self
            .answer_editors
            .get(&question.id)
            .map(|(editor, _)| editor.clone());
        let answer_editor = typed_editor
            .clone()
            .filter(|_| question.options.is_empty());
        let options: Vec<AnyElement> = if is_permission {
            [(t!("Allow"), true), (t!("Deny"), false)]
                .into_iter()
                .enumerate()
                .map(|(position, (label, allow))| {
                    let pending_id = pending_id.clone();
                    self.render_option_row(
                        format!("permission-{pending_id}-{position}").into(),
                        position + 1,
                        label.into_any_element(),
                        false,
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.session.update(cx, |session, cx| {
                            session.answer_permission(&pending_id, allow, cx)
                        })
                    }))
                    .into_any_element()
                })
                .collect()
        } else {
            question
                .options
                .iter()
                .enumerate()
                .map(|(position, option)| {
                    let picked = selected.contains(option);
                    let pending_id = pending_id.clone();
                    let question_id = question.id.clone();
                    let option = option.clone();
                    let multi_select = question.multi_select;
                    self.render_option_row(
                        format!("option-{}-{position}", question.id).into(),
                        position + 1,
                        option.clone().into_any_element(),
                        picked,
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.pick_option(
                            pending_id.clone(),
                            question_id.clone(),
                            option.clone(),
                            multi_select,
                            count,
                            cx,
                        )
                    }))
                    .into_any_element()
                })
                .chain(
                    typed_editor
                        .filter(|_| !question.options.is_empty())
                        .map(|editor| {
                            let typed = !editor.read(cx).text(cx).trim().is_empty()
                                && (question.multi_select || selected.is_empty());
                            let focus = editor.focus_handle(cx);
                            self.render_option_row(
                                format!("option-{}-typed", question.id).into(),
                                question.options.len() + 1,
                                editor.into_any_element(),
                                typed,
                                cx,
                            )
                            .cursor_text()
                            .on_click(move |_, window, cx| window.focus(&focus, cx))
                            .into_any_element()
                        }),
                )
                .collect()
        };
        let answered = !self.answer_labels(question, cx).is_empty();
        let is_last_page = page_index + 1 >= count;
        let header = if is_permission {
            t!("Permission").to_string()
        } else if question.header == agent_harness::claude::QUESTION_HEADER {
            i18n::translate(agent_harness::claude::QUESTION_HEADER).to_string()
        } else {
            question.header.clone()
        };
        let muted_label = colors.text_muted.opacity(0.6);
        let back_hover = ink(0.06, cx);
        let options_max_height = self
            .root_bounds
            .get()
            .map_or(px(QUESTION_EXPANDED_FALLBACK), |bounds| {
                bounds.size.height * QUESTION_OPTIONS_FRACTION
            });
        let question_focus = self.question_focus.clone();
        let mut key_context = KeyContext::default();
        key_context.add("AgentQuestion");
        if AgentChatSettings::get_global(cx).send_with == AgentChatSendKey::ModifierEnter {
            key_context.add("send_with_modifier");
        }
        v_flex()
            .track_focus(&self.question_focus)
            .key_context(key_context)
            .w_full()
            .rounded(px(COMPOSER_RADIUS))
            .border_1()
            .border_color(colors.border)
            .bg(composer_surface(cx))
            .shadow_lg()
            .child(
                v_flex()
                    .px(px(16.))
                    .pt(px(16.))
                    .child(
                        h_flex()
                            .gap(px(10.))
                            .child(
                                div()
                                    .text_size(ui(10.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(muted_label)
                                    .child(tracked_upper(&header)),
                            )
                            .when(count > 1, |this| {
                                this.child(
                                    div()
                                        .h(px(20.))
                                        .px(px(6.))
                                        .flex()
                                        .items_center()
                                        .rounded(px(6.))
                                        .bg(ink(0.06, cx))
                                        .text_size(ui(10.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(muted_label)
                                        .child(t!(
                                            "{current} of {count}",
                                            current = page_index + 1,
                                            count = count
                                        )),
                                )
                            }),
                    )
                    .child(self.render_question_text(&question.id, &question.question, cx))
                    .when(question.multi_select, |this| {
                        this.child(
                            div()
                                .mt(px(4.))
                                .text_size(ui(12.))
                                .text_color(colors.text_muted.opacity(0.65))
                                .child(t!("Select one or more options.")),
                        )
                    })
                    .child(
                        v_flex()
                            .id("agent-question-options")
                            .mt(px(12.))
                            .gap(px(4.))
                            .max_h(options_max_height)
                            .overflow_y_scroll()
                            .children(options),
                    )
                    .when_some(answer_editor, |this, editor| {
                        this.child(
                            div()
                                .mt(px(12.))
                                .border_t_1()
                                .border_color(hairline(0.06, cx))
                                .pt(px(12.))
                                .pb(px(4.))
                                .px(px(4.))
                                .child(editor),
                        )
                    }),
            )
            .child(
                h_flex()
                    .px(px(16.))
                    .pt(px(4.))
                    .pb(px(16.))
                    .justify_between()
                    .child(if page_index > 0 {
                        let pending_id = pending_id.clone();
                        div()
                            .id("agent-question-back")
                            .px(px(12.))
                            .py(px(6.))
                            .rounded(px(8.))
                            .cursor_pointer()
                            .text_size(ui(13.))
                            .text_color(colors.text_muted)
                            .hover(move |style| style.bg(back_hover).text_color(colors.text))
                            .child(t!("Back"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.question_pages
                                    .insert(pending_id.to_string(), page_index - 1);
                                cx.notify();
                            }))
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    })
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .child(
                                div()
                                    .id("agent-question-stop")
                                    .px(px(12.))
                                    .py(px(6.))
                                    .rounded(px(8.))
                                    .cursor_pointer()
                                    .text_size(ui(13.))
                                    .text_color(colors.text_muted)
                                    .hover(move |style| style.bg(back_hover).text_color(colors.text))
                                    .tooltip(move |_, cx| {
                                        Tooltip::for_action_in(t!("Stop"), &Stop, &question_focus, cx)
                                    })
                                    .child(t!("Stop"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.stop(&Stop, window, cx)
                                    })),
                            )
                            .when(!is_permission, |this| {
                                let pending_id = pending_id.clone();
                                this.child(
                                    div()
                                        .id("agent-question-next")
                                        .px(px(16.))
                                        .py(px(6.))
                                        .rounded(px(8.))
                                        .bg(colors.text)
                                        .text_size(ui(13.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(page(cx))
                                        .when(!answered, |this| this.opacity(0.4))
                                        .when(answered, |this| {
                                            this.cursor_pointer()
                                                .hover(|style| style.opacity(0.9))
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.advance_question(&pending_id, page_index, count, cx)
                                                }))
                                        })
                                        .child(if is_last_page {
                                            t!("Submit")
                                        } else {
                                            t!("Next")
                                        }),
                                )
                            })
                    ),
            )
            .into_any_element()
    }
}

