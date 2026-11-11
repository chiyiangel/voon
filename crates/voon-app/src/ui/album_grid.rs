use gpui::{
    Context, Entity, FontWeight, Render, Subscription, UniformListScrollHandle, Window, div,
    prelude::*, px, rgb, uniform_list,
};
use voon_core::db::AlbumRow;

use super::{
    cover,
    theme::{COMPOSER_PANEL_WIDTH, DETAIL_WIDTH, FACET_PANEL_WIDTH, SIDEBAR_WIDTH, Theme},
};
use crate::view_model::{
    AppState,
    grid::{CoverCache, GRID_GAP, GRID_PADDING},
};

pub struct AlbumGrid {
    state: Entity<AppState>,
    covers: Entity<CoverCache>,
    scroll: UniformListScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl AlbumGrid {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let covers = cx.new(|cx| CoverCache::new(&state, cx));
        let subscriptions = vec![
            cx.observe(&state, |_, _, cx| cx.notify()),
            cx.observe(&covers, |_, _, cx| cx.notify()),
        ];
        Self {
            state,
            covers,
            scroll: UniformListScrollHandle::new(),
            _subscriptions: subscriptions,
        }
    }

    /// Share decoded thumbnails with the detail column instead of rereading the
    /// same directory image or embedded picture.
    pub fn cover_cache(&self) -> Entity<CoverCache> {
        self.covers.clone()
    }
}

impl Render for AlbumGrid {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window);
        // The shell's side columns have fixed widths; the center takes the
        // remainder of the actual window viewport on every resize.
        let panel_width =
            if self.state.read(cx).navigation() == crate::view_model::Navigation::Facets {
                FACET_PANEL_WIDTH
            } else if self.state.read(cx).navigation() == crate::view_model::Navigation::Composers {
                COMPOSER_PANEL_WIDTH
            } else {
                0.
            };
        let width =
            f32::from(window.viewport_size().width) - SIDEBAR_WIDTH - DETAIL_WIDTH - panel_width;
        let layout = self.state.read(cx).grid_layout(width);
        let selected = self.state.read(cx).selected_album_paths();
        if layout.rows == 0 {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .child("音乐库中还没有专辑")
                .child(
                    div()
                        .text_color(rgb(theme.secondary))
                        .text_size(px(12.5))
                        .child("扫描发现的专辑会显示在这里"),
                )
                .into_any_element();
        }
        let state = self.state.clone();
        let covers = self.covers.clone();
        uniform_list("album-grid", layout.rows, move |range, _, cx| {
            range
                .map(|row_index| {
                    let albums = {
                        let state = state.read(cx);
                        state.grid_row(row_index, layout).to_vec()
                    };
                    let cards = albums
                        .into_iter()
                        .map(|album| {
                            let status = album.cover.clone().map(|cover| {
                                covers.update(cx, |cache, cx| cache.request(cover, cx))
                            });
                            let is_selected = selected.contains(&album.path);
                            card(
                                album,
                                status,
                                state.clone(),
                                layout.card_width,
                                is_selected,
                                theme,
                            )
                        })
                        .collect::<Vec<_>>();
                    div()
                        .id(row_index)
                        .flex()
                        .items_start()
                        .gap(px(GRID_GAP))
                        .h(px(layout.card_width + 94.))
                        .children(cards)
                })
                .collect()
        })
        .size_full()
        .px(px(GRID_PADDING))
        .pt(px(GRID_PADDING))
        .pb(px(GRID_PADDING))
        .track_scroll(self.scroll.clone())
        .into_any_element()
    }
}

fn card(
    album: AlbumRow,
    status: Option<crate::view_model::grid::CoverStatus>,
    state: Entity<AppState>,
    edge: f32,
    selected: bool,
    theme: Theme,
) -> impl IntoElement {
    let path = album.path.clone();
    let artist = album
        .metadata
        .album_artist
        .as_ref()
        .map(|field| field.value.clone())
        .unwrap_or_else(|| "未知艺术家".into());
    div()
        .id(gpui::SharedString::from(
            album.path.to_string_lossy().into_owned(),
        ))
        .w(px(edge))
        .flex_none()
        .flex()
        .flex_col()
        .cursor_pointer()
        .child(cover::render(status, edge, selected, theme))
        .child(
            div()
                .mt(px(6.))
                .h(px(32.))
                .text_size(px(12.5))
                .line_height(px(16.))
                .line_clamp(2)
                .font_weight(FontWeight::MEDIUM)
                .child(album.display_title().to_owned()),
        )
        .child(
            div()
                .mt(px(3.))
                .h(px(15.))
                .truncate()
                .text_size(px(11.5))
                .line_height(px(15.))
                .text_color(rgb(theme.secondary))
                .child(artist),
        )
        .child(
            div()
                .mt(px(3.))
                .h(px(14.))
                .text_size(px(11.))
                .line_height(px(14.))
                .text_color(rgb(theme.tertiary))
                .child(format!(
                    "{} 碟 · {} 轨",
                    album.disc_count, album.track_count
                )),
        )
        .on_click(move |event, _, cx| {
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
