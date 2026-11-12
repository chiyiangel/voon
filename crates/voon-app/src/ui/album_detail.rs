use std::{collections::HashSet, path::PathBuf};

use gpui::{Context, Entity, FontWeight, Render, Subscription, Window, div, prelude::*, px, rgb};
use voon_core::{Album, Track, TrackSource};

use super::{cover, empty, theme::Theme};
use crate::view_model::{AppState, grid::CoverCache};

mod files;
mod overview;
mod tracks;

pub(super) const FIELD_LABELS: &[(&str, &str)] = &[
    ("title", "标题"),
    ("album_artist", "专辑艺术家"),
    ("composer", "作曲家"),
    ("performers", "演奏者"),
    ("conductor", "指挥"),
    ("orchestra", "乐团"),
    ("year", "年份"),
    ("genre", "流派"),
    ("catalog_number", "唱片编号"),
    ("edition", "版次"),
];

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum DetailPage {
    #[default]
    Overview,
    Tracks,
    Organization,
    Files,
}

impl DetailPage {
    const ALL: [(Self, &'static str, &'static str); 4] = [
        (Self::Overview, "detail-overview", "概览"),
        (Self::Tracks, "detail-tracks", "曲目"),
        (Self::Organization, "detail-organization", "整理"),
        (Self::Files, "detail-files", "文件"),
    ];
}

pub struct AlbumDetail {
    state: Entity<AppState>,
    covers: Entity<CoverCache>,
    _subscriptions: Vec<Subscription>,
    page: DetailPage,
    album_path: Option<PathBuf>,
    sources_open: bool,
    metadata_open: bool,
    reading_open: bool,
    open_discs: HashSet<usize>,
    open_tracks: HashSet<(usize, usize)>,
}

impl AlbumDetail {
    pub fn new(
        state: Entity<AppState>,
        covers: Entity<CoverCache>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscriptions = vec![
            cx.observe(&state, |this, state, cx| {
                let path = state
                    .read(cx)
                    .album_detail()
                    .map(|album| album.path.clone());
                if this.album_path != path {
                    this.album_path = path;
                    this.sources_open = false;
                    this.metadata_open = false;
                    this.reading_open = false;
                    this.open_discs = HashSet::from([0]);
                    this.open_tracks.clear();
                }
                cx.notify();
            }),
            cx.observe(&covers, |_, _, cx| cx.notify()),
        ];
        Self {
            album_path: state
                .read(cx)
                .album_detail()
                .map(|album| album.path.clone()),
            state,
            covers,
            _subscriptions: subscriptions,
            page: DetailPage::default(),
            sources_open: false,
            metadata_open: false,
            reading_open: false,
            open_discs: HashSet::from([0]),
            open_tracks: HashSet::new(),
        }
    }
}

impl Render for AlbumDetail {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let state = self.state.read(cx);
        let Some(album) = state.album_detail().cloned() else {
            return empty::detail(theme).into_any_element();
        };
        let offline = album.physical_directories().iter().any(|path| {
            state
                .offline_roots()
                .iter()
                .any(|root| path.starts_with(root))
        });
        let issue_count = state.album_scan_issues(&album.path).len();
        let player_label = state.album_player_label().unwrap_or("用默认播放器打开");
        let status = album
            .cover
            .clone()
            .map(|cover| self.covers.update(cx, |cache, cx| cache.request(cover, cx)));
        let cover_state = self.state.clone();
        let edit_state = self.state.clone();
        let player_state = self.state.clone();
        let mut actions = div().flex().flex_wrap().gap_3().text_size(px(12.));
        if !offline {
            actions = actions.child(
                div()
                    .id("open-album-player")
                    .debug_selector(|| "open-album-player".into())
                    .cursor_pointer()
                    .px_2()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(theme.accent))
                    .text_color(rgb(theme.window))
                    .child(player_label)
                    .on_click(move |_, _, cx| {
                        player_state.update(cx, |state, cx| {
                            if let Err(error) = state.open_album_in_player(cx) {
                                state.report_error(error, cx);
                            }
                        })
                    }),
            );
        }
        actions = actions.child(
            div()
                .id("edit-album-information")
                .debug_selector(|| "edit-album-information".into())
                .cursor_pointer()
                .px_2()
                .py_2()
                .rounded_md()
                .bg(rgb(theme.well))
                .child("编辑信息")
                .on_click(move |_, _, cx| {
                    edit_state.update(cx, |state, cx| {
                        if let Err(error) = state.begin_album_edit(cx) {
                            state.report_error(error, cx);
                        }
                    })
                }),
        );
        let header = div()
            .flex_none()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(theme.secondary))
                    .child("专辑详情"),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .items_start()
                    .child(
                        div()
                            .id("choose-album-cover")
                            .cursor_pointer()
                            .flex_none()
                            .child(cover::render(status, 72., false, theme))
                            .on_click(move |_, _, cx| {
                                cover_state.update(cx, |state, cx| {
                                    if let Err(error) = state.begin_cover_picker(cx) {
                                        state.report_error(error, cx);
                                    }
                                })
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(15.))
                                    .child(album.display_title().to_owned()),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(theme.secondary))
                                    .child(
                                        album
                                            .metadata
                                            .album_artist
                                            .as_ref()
                                            .map(|field| field.value.clone())
                                            .unwrap_or_else(|| "未知艺术家".into()),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(theme.secondary))
                    .child(format!(
                        "{} 碟 · {} 轨{}",
                        album.discs.len(),
                        album.tracks().count(),
                        audio_summary(&album)
                    )),
            )
            .child(actions);
        let mut tabs = div()
            .flex_none()
            .flex()
            .border_b_1()
            .border_color(rgb(theme.line));
        for (page, id, label) in DetailPage::ALL {
            let active = self.page == page;
            tabs = tabs.child(
                div()
                    .id(id)
                    .debug_selector(move || id.into())
                    .flex_1()
                    .cursor_pointer()
                    .py_3()
                    .text_center()
                    .text_size(px(12.))
                    .text_color(rgb(if active {
                        theme.accent
                    } else {
                        theme.secondary
                    }))
                    .border_b_2()
                    .border_color(rgb(if active { theme.accent } else { theme.line }))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.page = page;
                        cx.notify();
                    })),
            );
        }
        let mut body = div()
            .id("album-detail")
            .size_full()
            .flex()
            .flex_col()
            .child(header);
        if offline || issue_count > 0 {
            body = body.child(
                div()
                    .id("detail-warning")
                    .debug_selector(|| "detail-warning".into())
                    .cursor_pointer()
                    .mx_4()
                    .mb_2()
                    .p_2()
                    .rounded_md()
                    .bg(rgb(theme.well))
                    .text_size(px(11.))
                    .text_color(rgb(theme.accent))
                    .child(if offline {
                        "目录离线 · 可查看已保存资料 ›".into()
                    } else {
                        format!("{issue_count} 项读取提示 · 查看详情 ›")
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page = DetailPage::Files;
                        this.reading_open = true;
                        cx.notify();
                    })),
            );
        }
        let content = match self.page {
            DetailPage::Overview => self.overview(&album, theme, cx),
            DetailPage::Tracks => self.tracks(&album, offline, theme, cx),
            DetailPage::Organization => {
                super::tags_view::album_organization(&self.state, &album.path, theme, cx)
                    .into_any_element()
            }
            DetailPage::Files => self.files(&album, offline, theme, cx),
        };
        // Page-specific scroll identity keeps long file/track views from scrolling the summary away.
        let scroll_id = DetailPage::ALL
            .iter()
            .find(|(page, _, _)| *page == self.page)
            .unwrap()
            .1;
        body.child(tabs)
            .child(
                div()
                    .id(gpui::SharedString::from(format!("detail-page-{scroll_id}")))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(content),
            )
            .into_any_element()
    }
}

fn audio_summary(album: &Album) -> String {
    let mut tracks = album.tracks();
    let Some(first) = tracks.next() else {
        return String::new();
    };
    if tracks.any(|track| {
        track.format != first.format
            || track.properties.sample_rate != first.properties.sample_rate
            || track.properties.bit_depth != first.properties.bit_depth
    }) {
        return " · 多种音频规格".into();
    }
    let mut summary = format!(" · {}", first.format.label());
    if let Some(depth) = first.properties.bit_depth {
        summary.push_str(&format!(" · {depth} bit"));
    }
    if let Some(rate) = first.properties.sample_rate {
        summary.push_str(&format!(" / {} kHz", rate as f64 / 1000.));
    }
    summary
}

#[cfg(test)]
mod tests;
