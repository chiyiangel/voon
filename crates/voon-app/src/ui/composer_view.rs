use super::theme::{COMPOSER_PANEL_WIDTH, Theme};
use crate::view_model::AppState;
use gpui::{Entity, FontWeight, div, prelude::*, px, rgb};

pub fn panel(state: &AppState, entity: &Entity<AppState>, theme: Theme) -> impl IntoElement {
    let all = entity.clone();
    div()
        .id("composer-panel")
        .w(px(COMPOSER_PANEL_WIDTH))
        .h_full()
        .flex_none()
        .overflow_y_scroll()
        .p(px(14.))
        .border_r_1()
        .border_color(rgb(theme.line))
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(theme.tertiary))
                .mb(px(8.))
                .child("作曲家"),
        )
        .child(
            div()
                .id("all-composers")
                .cursor_pointer()
                .p_2()
                .rounded_md()
                .text_size(px(12.5))
                .child("全部作曲家")
                .when(state.selected_composer().is_none(), |row| {
                    row.bg(rgb(theme.selected))
                })
                .on_click(move |_, _, cx| {
                    all.update(cx, |state, cx| {
                        if let Err(error) = state.select_composer(None, cx) {
                            state.report_error(error, cx);
                        }
                    })
                }),
        )
        .children(state.composer_options().iter().map(|composer| {
            let name = composer.name.clone();
            let entity = entity.clone();
            div()
                .id(gpui::SharedString::from(format!("composer:{name}")))
                .flex()
                .items_center()
                .gap_2()
                .cursor_pointer()
                .px_2()
                .py_3()
                .rounded_md()
                .when(state.selected_composer() == Some(name.as_str()), |row| {
                    row.bg(rgb(theme.selected))
                })
                .hover(|row| row.bg(rgb(theme.well)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(13.))
                        .child(name.clone()),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(theme.tertiary))
                        .child(format!("{} 张", composer.count)),
                )
                .on_click(move |_, _, cx| {
                    entity.update(cx, |state, cx| {
                        if let Err(error) = state.select_composer(Some(name.clone()), cx) {
                            state.report_error(error, cx);
                        }
                    })
                })
        }))
}
