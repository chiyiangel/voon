use super::theme::{FACET_PANEL_WIDTH, Theme};
use crate::view_model::{AppState, facets::FacetDimension};
use gpui::{AnyElement, Div, Entity, FontWeight, div, prelude::*, px, rgb};

pub fn panel(state: &AppState, entity: &Entity<AppState>, theme: Theme) -> impl IntoElement {
    let clear = entity.clone();
    let mut groups: Vec<AnyElement> = vec![];
    for dimension in FacetDimension::ALL {
        let options: Vec<_> = state
            .facet_options()
            .iter()
            .filter(|option| option.value.dimension() == dimension)
            .collect();
        if options.is_empty() {
            continue;
        }
        groups.push(
            div()
                .mt(px(12.))
                .mb(px(4.))
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(theme.tertiary))
                .child(dimension.label())
                .into_any_element(),
        );
        for option in options {
            let value = option.value.clone();
            let entity = entity.clone();
            groups.push(
                div()
                    .id(gpui::SharedString::from(format!("facet:{value:?}")))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h(px(25.))
                    .cursor_pointer()
                    .child(
                        div()
                            .size(px(14.))
                            .flex_none()
                            .rounded(px(4.))
                            .border_1()
                            .border_color(rgb(theme.line))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(option.selected, |checkbox| {
                                checkbox
                                    .bg(rgb(theme.selection))
                                    .border_color(rgb(theme.selection))
                                    .text_color(rgb(theme.window))
                            })
                            .text_size(px(10.))
                            .child(if option.selected { "✓" } else { "" }),
                    )
                    .child(div().text_size(px(12.5)).child(option.label.clone()))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(rgb(theme.tertiary))
                            .child(option.count.to_string()),
                    )
                    .on_click(move |_, _, cx| {
                        entity.update(cx, |state, cx| state.toggle_facet(value.clone(), cx))
                    })
                    .into_any_element(),
            );
        }
    }
    div()
        .id("facet-panel")
        .w(px(FACET_PANEL_WIDTH))
        .h_full()
        .flex_none()
        .overflow_y_scroll()
        .px(px(14.))
        .pb(px(14.))
        .bg(rgb(theme.window))
        .border_r_1()
        .border_color(rgb(theme.line))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .pt(px(10.))
                .text_size(px(12.))
                .child(format!("{} 张专辑", state.browsed_albums().len()))
                .child(
                    div()
                        .id("clear-facets")
                        .cursor_pointer()
                        .text_color(rgb(theme.selection))
                        .child("清除筛选")
                        .on_click(move |_, _, cx| {
                            clear.update(cx, |state, cx| state.clear_facets(cx))
                        }),
                ),
        )
        .children(groups)
}

pub fn chips(state: &AppState, entity: &Entity<AppState>, theme: Theme) -> Div {
    let active: Vec<_> = state
        .facet_options()
        .iter()
        .filter(|option| option.selected)
        .collect();
    div()
        .when(!active.is_empty(), |chips| {
            chips.flex().flex_wrap().gap(px(6.)).px(px(18.)).pt(px(10.))
        })
        .children(active.into_iter().map(|option| {
            let value = option.value.clone();
            let entity = entity.clone();
            div()
                .id(gpui::SharedString::from(format!("active:{value:?}")))
                .flex()
                .items_center()
                .gap(px(5.))
                .px(px(9.))
                .h(px(22.))
                .rounded_full()
                .bg(rgb(theme.well))
                .text_color(rgb(theme.selection))
                .text_size(px(12.))
                .cursor_pointer()
                .child(format!("{}：{} ×", value.dimension().label(), option.label))
                .on_click(move |_, _, cx| {
                    entity.update(cx, |state, cx| state.toggle_facet(value.clone(), cx))
                })
        }))
}
