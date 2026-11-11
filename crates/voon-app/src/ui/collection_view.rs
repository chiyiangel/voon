use super::{icons::icon, theme::Theme};
use crate::view_model::AppState;
use gpui::{Div, Entity, FontWeight, div, prelude::*, px, rgb};

pub fn navigation_tree(
    state: &AppState,
    entity: &Entity<AppState>,
    theme: Theme,
) -> impl IntoElement {
    div()
        .id("collection-tree")
        .max_h(px(280.))
        .overflow_y_scroll()
        .children(state.collection_rows().into_iter().map(|row| {
            let select_state = entity.clone();
            let toggle_state = entity.clone();
            let select_path = row.path.clone();
            let toggle_path = row.path.clone();
            div()
                .id(gpui::SharedString::from(format!(
                    "collection:{}",
                    row.path.display()
                )))
                .flex()
                .items_center()
                .gap(px(5.))
                .h(px(26.))
                .pl(px(8. + row.depth as f32 * 14.))
                .pr(px(8.))
                .rounded(px(6.))
                .cursor_pointer()
                .when(row.selected, |item| item.bg(rgb(theme.selected)))
                .hover(|item| item.bg(rgb(theme.selected)))
                .child(
                    div()
                        .id("expand")
                        .w(px(12.))
                        .text_color(rgb(theme.tertiary))
                        .child(if row.has_children {
                            if row.expanded { "⌄" } else { "›" }
                        } else {
                            ""
                        })
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            toggle_state
                                .update(cx, |state, cx| state.toggle_collection(&toggle_path, cx));
                        }),
                )
                .child(icon("box", 13., theme.accent))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.5))
                        .child(row.name),
                )
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(rgb(theme.tertiary))
                        .child(row.album_count.to_string()),
                )
                .on_click(move |_, _, cx| {
                    select_state.update(cx, |state, cx| {
                        if let Err(error) = state.select_collection(Some(select_path.clone()), cx) {
                            state.report_error(error, cx);
                        }
                    })
                })
        }))
}

pub fn summary(state: &AppState, theme: Theme) -> Div {
    let Some(collection) = state.selected_collection() else {
        return div();
    };
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(18.))
        .py(px(12.))
        .border_b_1()
        .border_color(rgb(theme.line))
        .child(icon("box", 22., theme.accent))
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child(collection.name.clone()),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(rgb(theme.secondary))
                .child(format!("{} 张专辑", state.browsed_albums().len())),
        )
}
