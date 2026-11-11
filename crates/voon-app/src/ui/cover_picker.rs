use super::{cover, theme::Theme};
use crate::view_model::{AppState, grid::CoverCache};
use gpui::{
    Context, Entity, FocusHandle, FontWeight, KeyBinding, MouseButton, Render, Subscription,
    Window, actions, div, prelude::*, px, relative, rgb, rgba, uniform_list,
};
use std::path::PathBuf;

actions!(voon_cover_picker, [CancelCoverPicker]);

pub struct CoverPicker {
    state: Entity<AppState>,
    covers: Entity<CoverCache>,
    focus: FocusHandle,
    album: Option<PathBuf>,
    _subscriptions: Vec<Subscription>,
}

impl CoverPicker {
    pub fn new(
        state: Entity<AppState>,
        covers: Entity<CoverCache>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.bind_keys([KeyBinding::new(
            "escape",
            CancelCoverPicker,
            Some("VoonCoverPicker"),
        )]);
        let subscriptions = vec![
            cx.observe(&state, |_, _, cx| cx.notify()),
            cx.observe(&covers, |_, _, cx| cx.notify()),
        ];
        Self {
            state,
            covers,
            focus: cx.focus_handle(),
            album: None,
            _subscriptions: subscriptions,
        }
    }
}

impl Render for CoverPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(draft) = self.state.read(cx).cover_picker().cloned() else {
            self.album = None;
            return div().into_any_element();
        };
        if self.album.as_ref() != Some(&draft.album_path) {
            window.focus(&self.focus);
            self.album = Some(draft.album_path.clone());
        }
        let theme = Theme::for_window(window);
        let automatic = draft
            .automatic
            .clone()
            .map(|cover| self.covers.update(cx, |cache, cx| cache.request(cover, cx)));
        let cancel = self.state.clone();
        let apply = self.state.clone();
        let restore = self.state.clone();
        let state = self.state.clone();
        let covers = self.covers.clone();
        let count = draft.candidates.len();
        let candidates = uniform_list(
            "cover-candidate-rows",
            count.div_ceil(4),
            move |range, _, cx| {
                range
                    .map(|row| {
                        let (candidates, selected) = {
                            let state = state.read(cx);
                            let draft = state.cover_picker().unwrap();
                            let start = (row * 4).min(draft.candidates.len());
                            (
                                draft.candidates[start..(start + 4).min(draft.candidates.len())]
                                    .to_vec(),
                                draft.selected.clone(),
                            )
                        };
                        let cards = candidates
                            .into_iter()
                            .enumerate()
                            .map(|(column, candidate)| {
                                let status = covers.update(cx, |cache, cx| {
                                    cache.request(candidate.cover.clone(), cx)
                                });
                                let is_selected = selected.as_ref() == Some(&candidate.cover);
                                let state = state.clone();
                                div()
                                    .id(row * 4 + column)
                                    .w(px(128.))
                                    .flex_none()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .cursor_pointer()
                                    .child(cover::render(Some(status), 128., is_selected, theme))
                                    .child(
                                        div()
                                            .h(px(32.))
                                            .text_size(px(11.5))
                                            .line_height(px(16.))
                                            .line_clamp(2)
                                            .child(candidate.label),
                                    )
                                    .on_click(move |_, _, cx| {
                                        state.update(cx, |state, cx| {
                                            if let Err(error) = state.select_cover_candidate(
                                                Some(candidate.cover.clone()),
                                                cx,
                                            ) {
                                                state.report_error(error, cx);
                                            }
                                        })
                                    })
                            })
                            .collect::<Vec<_>>();
                        div()
                            .h(px(180.))
                            .flex()
                            .gap(px(16.))
                            .justify_center()
                            .children(cards)
                    })
                    .collect()
            },
        )
        .size_full();
        let mut body = div().flex_1().min_h_0().px_5().py_3().relative();
        if draft.loading {
            body = body.child("正在读取候选封面…");
        } else if count == 0 {
            body = body.child("没有找到候选封面，仍可恢复自动选择。");
        } else {
            body = body.child(candidates);
        }
        if let Some(error) = draft.error {
            body = body.child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(0xc0352b))
                    .child(error),
            );
        }
        div()
            .id("cover-picker-overlay")
            .absolute()
            .inset_0()
            .size_full()
            .occlude()
            .bg(rgba(0x00000066))
            .flex()
            .items_center()
            .justify_center()
            .track_focus(&self.focus)
            .key_context("VoonCoverPicker")
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_action(cx.listener(|this, _: &CancelCoverPicker, _, cx| {
                this.state
                    .update(cx, |state, cx| state.cancel_cover_picker(cx))
            }))
            .child(
                div()
                    .w(px(640.))
                    .h(px(600.))
                    .max_w(relative(0.94))
                    .max_h(relative(0.94))
                    .rounded_lg()
                    .shadow_lg()
                    .bg(rgb(theme.window))
                    .text_color(rgb(theme.ink))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .p_5()
                            .flex_none()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .border_b_1()
                            .border_color(rgb(theme.line))
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("选择封面"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(theme.secondary))
                                    .child(draft.title),
                            ),
                    )
                    .child(
                        div()
                            .id("restore-auto-cover")
                            .mx_5()
                            .my_3()
                            .p_3()
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap_3()
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(if draft.selected.is_none() {
                                theme.selection
                            } else {
                                theme.line
                            }))
                            .cursor_pointer()
                            .child(cover::render(automatic, 56., false, theme))
                            .child(
                                div().flex().flex_col().gap_1().child("自动选择").child(
                                    div()
                                        .text_size(px(11.5))
                                        .text_color(rgb(theme.secondary))
                                        .child("优先专辑目录、碟目录和附件中的封面，其次内嵌封面"),
                                ),
                            )
                            .on_click(move |_, _, cx| {
                                restore.update(cx, |state, cx| {
                                    if let Err(error) = state.select_cover_candidate(None, cx) {
                                        state.report_error(error, cx);
                                    }
                                })
                            }),
                    )
                    .child(body)
                    .child(
                        div()
                            .p_4()
                            .flex_none()
                            .border_t_1()
                            .border_color(rgb(theme.line))
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(11.5))
                                    .text_color(rgb(theme.secondary))
                                    .child("只保存到本地索引，不修改音乐文件"),
                            )
                            .child(
                                div()
                                    .id("cancel-cover-picker")
                                    .cursor_pointer()
                                    .child("取消")
                                    .on_click(move |_, _, cx| {
                                        cancel.update(cx, |state, cx| state.cancel_cover_picker(cx))
                                    }),
                            )
                            .child(
                                div()
                                    .id("apply-cover-choice")
                                    .cursor_pointer()
                                    .px_4()
                                    .py_2()
                                    .rounded_md()
                                    .bg(rgb(theme.accent))
                                    .text_color(rgb(theme.window))
                                    .child("应用")
                                    .on_click(move |_, _, cx| {
                                        apply.update(cx, |state, cx| {
                                            if let Err(error) = state.apply_cover_choice(cx) {
                                                state.report_error(error, cx);
                                            }
                                        })
                                    }),
                            ),
                    ),
            )
            .into_any_element()
    }
}
