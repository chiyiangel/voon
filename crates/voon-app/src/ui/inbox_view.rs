use super::theme::Theme;
use crate::view_model::AppState;
use gpui::{App, Entity, IntoElement, div, prelude::*, rgb};

pub fn render(entity: &Entity<AppState>, theme: Theme, cx: &App) -> impl IntoElement {
    let state = entity.read(cx);
    let mut list = div()
        .id("attention-list")
        .size_full()
        .overflow_y_scroll()
        .flex()
        .flex_col();
    if state.attention_items().is_empty() {
        return list.items_center().justify_center().child("没有待处理项目");
    }
    for (position, item) in state.attention_items().iter().enumerate() {
        let state = entity.clone();
        let path = item.path.clone();
        let kind = item.kind.clone();
        let mut row = div()
            .id(("attention-item", position))
            .cursor_pointer()
            .p_4()
            .border_b_1()
            .border_color(rgb(theme.line))
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_sm().child(item.title.clone()))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(theme.secondary))
                    .child(item.explanation.clone()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(theme.tertiary))
                    .child(item.path.display().to_string()),
            )
            .on_click(move |_, _, cx| {
                state.update(cx, |state, cx| state.open_attention(&path, &kind, cx))
            });
        if item.stage.is_some() {
            let retry_state = entity.clone();
            let retry_path = item.path.clone();
            let retry_kind = item.kind.clone();
            row = row.child(
                div()
                    .id(("retry-structure-read", position))
                    .cursor_pointer()
                    .text_color(rgb(theme.accent))
                    .child("重试读取")
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        retry_state.update(cx, |state, cx| {
                            if let Err(error) = state.retry_attention(&retry_path, &retry_kind, cx)
                            {
                                state.report_error(error, cx);
                            }
                        });
                    }),
            );
        }
        if let Some(old) = &item.previous_path {
            let confirm_state = entity.clone();
            let dismiss_state = entity.clone();
            let old_confirm = old.clone();
            let old_dismiss = old.clone();
            let new_confirm = item.path.clone();
            let new_dismiss = item.path.clone();
            row = row.child(
                div()
                    .flex()
                    .gap_4()
                    .child(
                        div()
                            .id(("confirm-rename", position))
                            .cursor_pointer()
                            .text_color(rgb(theme.accent))
                            .child("确认")
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                confirm_state.update(cx, |state, cx| {
                                    if let Err(error) =
                                        state.confirm_rename(&old_confirm, &new_confirm, cx)
                                    {
                                        state.report_error(error, cx);
                                    }
                                });
                            }),
                    )
                    .child(
                        div()
                            .id(("dismiss-rename", position))
                            .cursor_pointer()
                            .text_color(rgb(theme.secondary))
                            .child("忽略")
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                dismiss_state.update(cx, |state, cx| {
                                    if let Err(error) =
                                        state.dismiss_rename(&old_dismiss, &new_dismiss, cx)
                                    {
                                        state.report_error(error, cx);
                                    }
                                });
                            }),
                    ),
            );
        }
        list = list.child(row);
    }
    list
}
