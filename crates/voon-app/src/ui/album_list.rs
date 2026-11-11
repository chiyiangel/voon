//! Virtualized rows share the state query with the cover grid.

use gpui::{
    Context, Entity, FontWeight, Render, Subscription, UniformListScrollHandle, Window, div,
    prelude::*, px, relative, rgb, uniform_list,
};
use voon_core::db::AlbumRow;

use super::{cover, theme::Theme};
use crate::view_model::grid::{CoverCache, CoverStatus};
use crate::view_model::{AppState, SortOrder, browse::SortDirection};

const COLUMNS: [(SortOrder, f32); 8] = [
    (SortOrder::Title, 0.25),
    (SortOrder::Artist, 0.16),
    (SortOrder::Composer, 0.12),
    (SortOrder::Year, 0.07),
    (SortOrder::Format, 0.07),
    (SortOrder::Resolution, 0.11),
    (SortOrder::DiscTracks, 0.08),
    (SortOrder::Collection, 0.14),
];

pub struct AlbumList {
    state: Entity<AppState>,
    covers: Entity<CoverCache>,
    scroll: UniformListScrollHandle,
    _subscription: Subscription,
    _cover_subscription: Subscription,
}

impl AlbumList {
    pub fn new(
        state: Entity<AppState>,
        covers: Entity<CoverCache>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.observe(&state, |_, _, cx| cx.notify());
        let cover_subscription = cx.observe(&covers, |_, _, cx| cx.notify());
        Self {
            state,
            covers,
            scroll: UniformListScrollHandle::new(),
            _subscription: subscription,
            _cover_subscription: cover_subscription,
        }
    }
}

impl Render for AlbumList {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let snapshot = self.state.read(cx);
        let count = snapshot.browsed_albums().len();
        let mut header = div()
            .h(px(34.))
            .flex_none()
            .flex()
            .items_center()
            .bg(rgb(theme.toolbar))
            .border_b_1()
            .border_color(rgb(theme.line));
        for (column, width) in COLUMNS {
            let state = self.state.clone();
            let arrow = if snapshot.sort_order() == column {
                if snapshot.sort_direction() == SortDirection::Ascending {
                    " ↑"
                } else {
                    " ↓"
                }
            } else {
                ""
            };
            header = header.child(
                div()
                    .id(("sort-column", column as usize))
                    .w(relative(width))
                    .flex_none()
                    .min_w_0()
                    .px(px(8.))
                    .cursor_pointer()
                    .truncate()
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(if snapshot.sort_order() == column {
                        theme.ink
                    } else {
                        theme.secondary
                    }))
                    .child(format!("{}{arrow}", column.label()))
                    .on_click(move |_, _, cx| {
                        state.update(cx, |state, cx| state.toggle_sort(column, cx))
                    }),
            );
        }
        let state = self.state.clone();
        let covers = self.covers.clone();
        let rows = uniform_list("album-list-rows", count, move |range, _, cx| {
            let snapshot = state.read(cx);
            let selected = snapshot.selected_album_paths();
            let albums = snapshot.browsed_albums()[range].to_vec();
            albums
                .into_iter()
                .map(|album| {
                    let selected = selected.contains(&album.path);
                    let status = album
                        .cover
                        .clone()
                        .map(|cover| covers.update(cx, |cache, cx| cache.request(cover, cx)));
                    row(album, status, selected, state.clone(), theme)
                })
                .collect()
        })
        .size_full()
        .track_scroll(self.scroll.clone());
        let mut table = div()
            .h_full()
            .w_full()
            .min_w(px(960.))
            .flex()
            .flex_col()
            .child(header);
        if count == 0 {
            table = table.child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(theme.secondary))
                    .child("没有符合条件的专辑"),
            );
        } else {
            table = table.child(div().flex_1().min_h_0().relative().child(rows));
        }
        div()
            .id("album-list")
            .size_full()
            .flex()
            .overflow_x_scroll()
            .child(table)
    }
}

fn row(
    album: AlbumRow,
    status: Option<CoverStatus>,
    selected: bool,
    state: Entity<AppState>,
    theme: Theme,
) -> gpui::Stateful<gpui::Div> {
    let path = album.path.clone();
    let mut row = div()
        .id(gpui::SharedString::from(
            path.to_string_lossy().into_owned(),
        ))
        .h(px(42.))
        .w_full()
        .flex()
        .items_center()
        .cursor_pointer()
        .bg(rgb(if selected {
            theme.selected
        } else {
            theme.window
        }))
        .border_b_1()
        .border_color(rgb(theme.line))
        .hover(move |style| style.bg(rgb(if selected { theme.selected } else { theme.well })));
    for (column, width) in COLUMNS {
        row = row.child(
            div()
                .w(relative(width))
                .flex_none()
                .min_w_0()
                .px(px(8.))
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(px(11.5))
                .text_color(rgb(if column == SortOrder::Title {
                    theme.ink
                } else {
                    theme.secondary
                }))
                .when(column == SortOrder::Title, |cell| {
                    cell.child(cover::render(status.clone(), 24., false, theme))
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(column.cell_text(&album)),
                ),
        );
    }
    row.on_click(move |event, _, cx| {
        let modifiers = event.modifiers();
        state.update(cx, |state, cx| {
            state.click_album(
                path.clone(),
                modifiers.platform || modifiers.control,
                modifiers.shift,
                cx,
            )
        })
    })
}
