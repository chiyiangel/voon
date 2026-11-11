use std::path::PathBuf;

use gpui::{
    Context, Entity, Focusable, FontWeight, KeyBinding, MouseButton, Render, Subscription, Window,
    actions, div, prelude::*, px, relative, rgb, rgba,
};
use voon_core::FieldSource;

use super::{
    album_detail::FIELD_LABELS,
    text_input::{TextChanged, TextInput},
    theme::Theme,
};
use crate::view_model::AppState;

actions!(album_edit, [CancelAlbumEdit]);

pub struct EditSheet {
    state: Entity<AppState>,
    album: Option<PathBuf>,
    inputs: Vec<(&'static str, Entity<TextInput>)>,
    _input_subscriptions: Vec<Subscription>,
    _state_subscription: Subscription,
}

impl EditSheet {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.bind_keys([KeyBinding::new(
            "escape",
            CancelAlbumEdit,
            Some("VoonAlbumEdit"),
        )]);
        let subscription = cx.observe(&state, |_, _, cx| cx.notify());
        Self {
            state,
            album: None,
            inputs: Vec::new(),
            _input_subscriptions: Vec::new(),
            _state_subscription: subscription,
        }
    }
}

impl Render for EditSheet {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(draft) = self.state.read(cx).album_edit_draft().cloned() else {
            self.album = None;
            self.inputs.clear();
            self._input_subscriptions.clear();
            return div().into_any_element();
        };
        if self.album.as_deref() != Some(draft.album_path()) {
            self.inputs.clear();
            self._input_subscriptions.clear();
            for field in draft.fields() {
                let name = field.name();
                let input = cx.new(|cx| TextInput::new("使用自动值", cx));
                input.update(cx, |input, cx| input.set_text(field.value().to_owned(), cx));
                let state = self.state.clone();
                let album_path = draft.album_path().to_path_buf();
                self._input_subscriptions.push(cx.subscribe(
                    &input,
                    move |_, _, event: &TextChanged, cx| {
                        state.update(cx, |state, cx| {
                            if state
                                .album_edit_draft()
                                .is_none_or(|draft| draft.album_path() != album_path)
                            {
                                return;
                            }
                            if let Err(error) =
                                state.set_album_edit_field(name, event.text.clone(), cx)
                            {
                                state.report_error(error, cx);
                            }
                        });
                    },
                ));
                self.inputs.push((name, input));
            }
            self.album = Some(draft.album_path().to_path_buf());
            if let Some((_, input)) = self.inputs.first() {
                window.focus(&input.read(cx).focus_handle(cx));
            }
        }
        let theme = Theme::for_window(window);
        let cancel = self.state.clone();
        let close = self.state.clone();
        let save = self.state.clone();
        let mut fields = div()
            .id("album-edit-fields")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_5()
            .flex()
            .flex_col()
            .gap_3();
        for &(name, label) in FIELD_LABELS {
            let field = draft.field(name).unwrap();
            let input = &self
                .inputs
                .iter()
                .find(|(field, _)| *field == name)
                .unwrap()
                .1;
            if input.read(cx).text() != field.value() {
                input.update(cx, |input, cx| input.set_text(field.value().to_owned(), cx));
            }
            let source = match field.source() {
                Some(FieldSource::Tag) => "标签",
                Some(FieldSource::Cue) => "CUE",
                Some(FieldSource::FolderName) => "目录名",
                Some(FieldSource::UserEdit) => "我的编辑",
                None => "未填写",
            };
            let mut row = div()
                .flex_none()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .w(px(78.))
                        .flex_none()
                        .text_color(rgb(theme.secondary))
                        .child(label),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .px_2()
                        .py_1()
                        .border_1()
                        .border_color(rgb(theme.line))
                        .rounded_md()
                        .bg(rgb(theme.well))
                        .child(input.clone()),
                )
                .child(
                    div()
                        .w(px(56.))
                        .flex_none()
                        .text_xs()
                        .text_color(rgb(theme.secondary))
                        .child(source),
                );
            if field.source() == Some(FieldSource::UserEdit) {
                let state = self.state.clone();
                row = row.child(
                    div()
                        .id(gpui::SharedString::from(format!(
                            "restore-edit-field-{name}"
                        )))
                        .cursor_pointer()
                        .w(px(78.))
                        .flex_none()
                        .text_xs()
                        .text_color(rgb(theme.accent))
                        .child("恢复自动值")
                        .on_click(move |_, _, cx| {
                            state.update(cx, |state, cx| {
                                if let Err(error) = state.restore_album_edit_field(name, cx) {
                                    state.report_error(error, cx);
                                }
                            })
                        }),
                );
            } else {
                row = row.child(div().w(px(78.)).flex_none());
            }
            fields = fields.child(row);
        }
        if let Some(error) = draft.error() {
            fields = fields.child(div().text_color(rgb(0xc0352b)).child(error.to_owned()));
        }
        div().id("album-edit-overlay").absolute().inset_0().size_full().occlude().bg(rgba(0x00000066))
            .flex().items_center().justify_center().key_context("VoonAlbumEdit")
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_action(cx.listener(|this, _: &CancelAlbumEdit, _, cx| this.state.update(cx, |state, cx| state.cancel_album_edit(cx))))
            .child(div().w(px(680.)).max_w(relative(0.94)).max_h(relative(0.94)).rounded_lg().shadow_lg().bg(rgb(theme.window)).text_color(rgb(theme.ink)).flex().flex_col()
                .child(div().flex_none().p_5().border_b_1().border_color(rgb(theme.line)).flex().flex_col().gap_2()
                    .child(div().flex().items_center().justify_between()
                        .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("编辑专辑信息"))
                        .child(div().id("close-album-edit").cursor_pointer().text_color(rgb(theme.secondary)).child("关闭").on_click(move |_, _, cx| close.update(cx, |state, cx| state.cancel_album_edit(cx)))))
                    .child(div().text_sm().text_color(rgb(theme.secondary)).child("只保存到 Voon 本地，不会修改音乐文件。留空或恢复自动值可重新使用自动信息。")))
                .child(fields)
                .child(div().flex_none().p_4().border_t_1().border_color(rgb(theme.line)).flex().justify_end().gap_4()
                    .child(div().id("cancel-album-edit").cursor_pointer().px_3().py_2().child("取消").on_click(move |_, _, cx| cancel.update(cx, |state, cx| state.cancel_album_edit(cx))))
                    .child(div().id("save-album-edit").cursor_pointer().px_4().py_2().rounded_md().bg(rgb(theme.accent)).text_color(rgb(theme.window)).child("保存").on_click(move |_, _, cx| save.update(cx, |state, cx| { if let Err(error) = state.save_album_edit(cx) && state.album_edit_draft().is_none() { state.report_error(error, cx); } })))))
            .into_any_element()
    }
}
