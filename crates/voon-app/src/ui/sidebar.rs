use gpui::{Div, Entity, FontWeight, div, prelude::*, px, rgb};

use super::{
    icons::icon,
    theme::{SIDEBAR_WIDTH, TOOLBAR_HEIGHT, Theme},
    truncating_label::truncating_label,
};
use crate::motion;
use crate::view_model::{AppState, Navigation};

pub fn navigation_label(navigation: Navigation) -> &'static str {
    match navigation {
        Navigation::AllAlbums => "音乐库",
        Navigation::Collections => "套装",
        Navigation::Composers => "作曲家",
        Navigation::Facets => "分面",
        Navigation::UserTags => "标签",
        Navigation::ManualCollections => "合集",
        Navigation::NeedsAttention => "待处理",
    }
}

pub fn render(
    state: &AppState,
    entity: &Entity<AppState>,
    theme: Theme,
    leaving_navigation: Option<Navigation>,
) -> Div {
    let group = |label| {
        div()
            .mt(px(10.))
            .mb(px(2.))
            .px(px(8.))
            .text_size(px(11.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(theme.tertiary))
            .child(label)
    };
    let row = |destination, glyph| {
        let entity = entity.clone();
        let selected = state.navigation() == destination;
        let leaving = leaving_navigation == Some(destination);
        div()
            .id(navigation_label(destination))
            .relative()
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(28.))
            .px(px(8.))
            .rounded(px(6.))
            .cursor_pointer()
            .when(selected, |row| row.font_weight(FontWeight::MEDIUM))
            .hover(|row| row.bg(rgb(theme.selected)))
            .when(selected || leaving, |row| {
                row.child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded(px(6.))
                        .child(motion::fade(
                            gpui::SharedString::from(format!(
                                "{}:{}",
                                if selected {
                                    "nav-select"
                                } else {
                                    "nav-deselect"
                                },
                                navigation_label(destination)
                            )),
                            selected,
                            motion::selection(),
                            div().size_full().rounded(px(6.)).bg(rgb(theme.selected)),
                        )),
                )
            })
            .child(icon(glyph, 16., theme.accent))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .child(truncating_label(navigation_label(destination))),
            )
            .when(destination == Navigation::Collections, |row| {
                row.child(div().flex_none().flex().items_center().child(icon(
                    if state.collections_sidebar_expanded() {
                        "chevron-down"
                    } else {
                        "chevron-right"
                    },
                    14.,
                    theme.tertiary,
                )))
            })
            .when(destination == Navigation::NeedsAttention, |row| {
                row.child(
                    div()
                        .flex_none()
                        .text_xs()
                        .child(state.attention_items().len().to_string()),
                )
            })
            .on_click(move |_, _, cx| {
                entity.update(cx, |state, cx| {
                    if destination == Navigation::Collections {
                        state.toggle_collections_sidebar(cx);
                    } else {
                        state.navigate(destination, cx);
                    }
                })
            })
    };

    div()
        .debug_selector(|| "library-sidebar".into())
        .w(px(SIDEBAR_WIDTH))
        .max_w(px(SIDEBAR_WIDTH))
        .min_w_0()
        .h_full()
        .flex_none()
        .overflow_hidden()
        .flex()
        .flex_col()
        .bg(rgb(theme.sidebar))
        .border_r_1()
        .border_color(rgb(theme.line))
        .px(px(10.))
        .pb(px(10.))
        // Native macOS window controls occupy the prototype's top 52 pixels.
        .child(div().h(px(TOOLBAR_HEIGHT)).flex_none())
        .child(group("资料库"))
        .child(row(Navigation::AllAlbums, "library"))
        .child(row(Navigation::Collections, "box"))
        .when(state.collections_tree_view().is_some(), |sidebar| {
            sidebar.child(motion::fade(
                "collection-tree-shell",
                state.collections_tree_view() == Some(true),
                motion::chip(),
                div().w_full().min_w_0().overflow_x_hidden().child(
                    super::collection_view::navigation_tree(state, entity, theme),
                ),
            ))
        })
        .child(row(Navigation::Composers, "composer"))
        .child(row(Navigation::Facets, "facet"))
        .child(group("整理"))
        .child(row(Navigation::UserTags, "tag"))
        .child(super::tags_view::navigation_tags(
            state, entity, theme, false,
        ))
        .child(row(Navigation::ManualCollections, "stack"))
        .child(super::tags_view::navigation_tags(
            state, entity, theme, true,
        ))
        .child(row(Navigation::NeedsAttention, "inbox"))
        .child(div().flex_1())
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .p(px(8.))
                .text_size(px(11.5))
                .text_color(rgb(theme.secondary))
                .child(
                    div()
                        .size(px(8.))
                        .flex_none()
                        .rounded_full()
                        .bg(rgb(theme.tertiary)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(theme.ink))
                                .child(if state.roots().is_empty() {
                                    "尚未添加音乐库"
                                } else {
                                    "本地音乐索引"
                                }),
                        )
                        .child(if state.roots().is_empty() {
                            "添加后开始建立本地索引".to_string()
                        } else {
                            format!("{} 张专辑", state.albums().len())
                        }),
                ),
        )
}
