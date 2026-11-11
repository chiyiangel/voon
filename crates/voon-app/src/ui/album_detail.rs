use gpui::{Context, Entity, FontWeight, Render, Subscription, Window, div, prelude::*, px, rgb};
use voon_core::{Album, FieldSource, Track, TrackSource};

use super::{cover, empty, theme::Theme};
use crate::view_model::{AppState, grid::CoverCache};

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

pub struct AlbumDetail {
    state: Entity<AppState>,
    covers: Entity<CoverCache>,
    _subscriptions: Vec<Subscription>,
}

impl AlbumDetail {
    pub fn new(
        state: Entity<AppState>,
        covers: Entity<CoverCache>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscriptions = vec![
            cx.observe(&state, |_, _, cx| cx.notify()),
            cx.observe(&covers, |_, _, cx| cx.notify()),
        ];
        Self {
            state,
            covers,
            _subscriptions: subscriptions,
        }
    }
}

impl Render for AlbumDetail {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let (album, images, loading, image_error, offline) = {
            let state = self.state.read(cx);
            (
                state.album_detail().cloned(),
                state.attachment_images().to_vec(),
                state.attachment_images_loading(),
                state.attachment_images_error().map(str::to_owned),
                state.album_detail().is_some_and(|album| {
                    state
                        .offline_roots()
                        .iter()
                        .any(|root| album.path.starts_with(root))
                }),
            )
        };
        let Some(album) = album else {
            return empty::detail(theme).into_any_element();
        };
        let status = album
            .cover
            .clone()
            .map(|cover| self.covers.update(cx, |cache, cx| cache.request(cover, cx)));
        let mut body = div()
            .id("album-detail")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_none()
                    .p_4()
                    .border_b_1()
                    .border_color(rgb(theme.line))
                    .flex()
                    .gap_3()
                    .items_start()
                    .child({
                        let state = self.state.clone();
                        div()
                            .id("choose-album-cover")
                            .cursor_pointer()
                            .child(cover::render(status, 112., false, theme))
                            .on_click(move |_, _, cx| {
                                state.update(cx, |state, cx| {
                                    if let Err(error) = state.begin_cover_picker(cx) {
                                        state.report_error(error, cx);
                                    }
                                })
                            })
                    })
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
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(theme.tertiary))
                                    .child(format!(
                                        "{} 碟 · {} 轨",
                                        album.discs.len(),
                                        album.tracks().count()
                                    )),
                            ),
                    ),
            )
            .child(metadata(&album, theme))
            .child(super::tags_view::album_organization(
                &self.state,
                &album.path,
                theme,
                cx,
            ));
        let edit_state = self.state.clone();
        let observations = self.state.read(cx).album_tag_observations();
        let mut tag_status = div().flex_none().p_4().flex().flex_col().gap_2().child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child("标签读取状态"),
        );
        match observations {
            Ok(observations) => {
                for (path, observation) in observations {
                    let label = match observation.result.status() {
                        "Tagged" => "已读取标签",
                        "NoTags" => "已确认无标签",
                        "Unsupported" => "格式尚未支持",
                        "IoFailure" => "暂时无法读取",
                        _ => "标签解析失败",
                    };
                    tag_status =
                        tag_status.child(div().text_xs().text_color(rgb(theme.secondary)).child(
                            format!(
                                "{label} · {}{}",
                                path.display(),
                                if observation.last_success.is_some() {
                                    " · 保留上次成功资料，尚未重新确认"
                                } else {
                                    ""
                                }
                            ),
                        ));
                }
            }
            Err(_) => tag_status = tag_status.child("无法读取本地标签状态，请重新打开专辑"),
        }
        body = body.child(tag_status);
        body = body.child(
            div()
                .id("edit-album-information")
                .flex_none()
                .cursor_pointer()
                .px_4()
                .py_2()
                .text_color(rgb(theme.accent))
                .child("编辑信息")
                .on_click(move |_, _, cx| {
                    edit_state.update(cx, |state, cx| {
                        if let Err(error) = state.begin_album_edit(cx) {
                            state.report_error(error, cx);
                        }
                    })
                }),
        );
        if !offline {
            let state = self.state.clone();
            body = body.child(
                div()
                    .id("open-album-player")
                    .cursor_pointer()
                    .px_4()
                    .py_2()
                    .text_color(rgb(theme.accent))
                    .child("用默认播放器打开")
                    .on_click(move |_, _, cx| {
                        state.update(cx, |state, cx| {
                            if let Err(error) = state.open_album_in_player(cx) {
                                state.report_error(error, cx);
                            }
                        })
                    }),
            );
        }
        for (disc_position, disc) in album.discs.iter().enumerate() {
            body = body.child(
                div()
                    .flex_none()
                    .p_4()
                    .border_b_1()
                    .border_color(rgb(theme.line))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(format!(
                        "碟 {} · {} 轨",
                        disc.number,
                        disc.tracks.len()
                    )))
                    .children(
                        disc.tracks
                            .iter()
                            .enumerate()
                            .map(|(track_position, track)| {
                                let state = self.state.clone();
                                let album_path = album.path.clone();
                                div().child(track_row(track, theme)).when(!offline, |row| {
                                    row.child(
                                        div()
                                            .id((
                                                "open-track-player",
                                                disc_position * 100_000 + track_position,
                                            ))
                                            .cursor_pointer()
                                            .text_xs()
                                            .text_color(rgb(theme.accent))
                                            .child("用默认播放器打开")
                                            .on_click(move |_, _, cx| {
                                                state.update(cx, |state, cx| {
                                                    if let Err(error) = state.open_track_in_player(
                                                        &album_path,
                                                        disc_position,
                                                        track_position,
                                                        cx,
                                                    ) {
                                                        state.report_error(error, cx);
                                                    }
                                                })
                                            }),
                                    )
                                })
                            }),
                    ),
            );
        }
        let mut attachments = div()
            .flex_none()
            .p_4()
            .border_b_1()
            .border_color(rgb(theme.line))
            .flex()
            .flex_col()
            .gap_2()
            .child(div().font_weight(FontWeight::SEMIBOLD).child("附件图片"));
        if loading {
            attachments = attachments.child(
                div()
                    .text_color(rgb(theme.secondary))
                    .child("正在读取图片列表…"),
            );
        } else if let Some(error) = image_error {
            let state = self.state.clone();
            attachments = attachments
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(theme.secondary))
                        .child(error),
                )
                .child(
                    div()
                        .id("reload-attachment-images")
                        .cursor_pointer()
                        .text_color(rgb(theme.accent))
                        .child("重新载入")
                        .on_click(move |_, _, cx| {
                            state.update(cx, |state, cx| {
                                if let Err(error) = state.reload_attachment_images(cx) {
                                    state.report_error(error, cx);
                                }
                            })
                        }),
                );
        } else if images.is_empty() {
            attachments =
                attachments.child(div().text_color(rgb(theme.tertiary)).child("没有附件图片"));
        } else {
            attachments =
                attachments.children(images.into_iter().enumerate().map(|(position, image)| {
                    let state = self.state.clone();
                    let label = image
                        .strip_prefix(&album.path)
                        .unwrap_or(&image)
                        .display()
                        .to_string();
                    div()
                        .id(("attachment-image", position))
                        .cursor_pointer()
                        .text_size(px(12.))
                        .text_color(rgb(theme.accent))
                        .child(format!("查看 · {label}"))
                        .on_click(move |_, _, cx| {
                            state.update(cx, |state, cx| {
                                if let Err(error) = state.open_attachment_image(image.clone(), cx) {
                                    state.report_error(error, cx);
                                }
                            })
                        })
                }));
        }
        body = body.child(attachments);
        let mut location = div()
            .flex_none()
            .p_4()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().font_weight(FontWeight::SEMIBOLD).child("位置"))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(theme.secondary))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(
                        album
                            .physical_directories()
                            .into_iter()
                            .map(|path| div().child(path.display().to_string())),
                    ),
            );
        if offline {
            location = location.child(div().text_color(rgb(theme.tertiary)).child("音乐库未挂载"));
        } else {
            let state = self.state.clone();
            let label = if cfg!(target_os = "macos") {
                "在 Finder 中显示"
            } else {
                "在文件管理器中显示"
            };
            location = location.child(
                div()
                    .id("reveal-album")
                    .cursor_pointer()
                    .text_color(rgb(theme.accent))
                    .child(label)
                    .on_click(move |_, _, cx| {
                        state.update(cx, |state, cx| {
                            if let Err(error) = state.reveal_selected_album(cx) {
                                state.report_error(error, cx);
                            }
                        })
                    }),
            );
        }
        body.child(location).into_any_element()
    }
}

fn metadata(album: &Album, theme: Theme) -> impl IntoElement {
    let fields = album.metadata.entries();
    let mut section = div()
        .flex_none()
        .p_4()
        .border_b_1()
        .border_color(rgb(theme.line))
        .flex()
        .flex_col()
        .gap_3()
        .child(div().font_weight(FontWeight::SEMIBOLD).child("专辑信息"));
    for &(name, label) in FIELD_LABELS {
        let field = fields.iter().find(|(field, _, _)| *field == name);
        let mut row = div()
            .flex()
            .items_start()
            .gap_2()
            .text_size(px(11.5))
            .child(
                div()
                    .w(px(66.))
                    .flex_none()
                    .text_color(rgb(theme.secondary))
                    .child(label),
            )
            .child(
                div().flex_1().min_w_0().child(
                    field
                        .map(|(_, value, _)| value.clone())
                        .unwrap_or_else(|| "未填写".into()),
                ),
            );
        if let Some((_, _, source)) = field {
            let label = match source {
                FieldSource::Tag => "标签",
                FieldSource::Cue => "CUE",
                FieldSource::FolderName => "目录名",
                FieldSource::UserEdit => "我的编辑",
            };
            row = row.child(
                div()
                    .flex_none()
                    .px_1()
                    .rounded_sm()
                    .bg(rgb(theme.well))
                    .text_color(rgb(theme.secondary))
                    .text_size(px(10.))
                    .child(label),
            );
        }
        section = section.child(row);
    }
    if let Some(collection) = &album.collection {
        section = section.child(
            div()
                .text_size(px(11.5))
                .text_color(rgb(theme.secondary))
                .child(format!("所属合集 · {}", collection.display())),
        );
    }
    section
}

fn track_row(track: &Track, theme: Theme) -> impl IntoElement {
    let title = track
        .title
        .as_ref()
        .map(|field| field.value.clone())
        .unwrap_or_else(|| {
            track
                .source
                .audio_path()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        });
    let mut row = div().flex().flex_col().gap_1().text_size(px(11.5)).child(
        div()
            .flex()
            .items_start()
            .gap_2()
            .child(
                div()
                    .w(px(20.))
                    .flex_none()
                    .text_color(rgb(theme.tertiary))
                    .child(
                        track
                            .number
                            .map(|number| number.to_string())
                            .unwrap_or_else(|| "—".into()),
                    ),
            )
            .child(div().flex_1().min_w_0().child(title))
            .child(
                div()
                    .flex_none()
                    .text_color(rgb(theme.secondary))
                    .child(duration(track.properties.duration_ms)),
            ),
    );
    let mut properties = vec![track.format.label().to_owned()];
    properties.push(
        track
            .properties
            .sample_rate
            .map(|rate| format!("{} kHz", rate as f64 / 1000.))
            .unwrap_or_else(|| "采样率未知".into()),
    );
    properties.push(
        track
            .properties
            .bit_depth
            .map(|depth| format!("{depth} 位"))
            .unwrap_or_else(|| "位深未知".into()),
    );
    if let Some(channels) = track.properties.channels {
        properties.push(format!("{channels} 声道"));
    }
    row = row.child(
        div()
            .text_size(px(10.5))
            .text_color(rgb(theme.secondary))
            .child(properties.join(" · ")),
    );
    match &track.source {
        TrackSource::Virtual {
            image,
            cue,
            start,
            end,
        } => {
            row = row
                .child(div().text_color(rgb(theme.accent)).child(format!(
                    "虚拟曲目 · {} → {}",
                    cue_time(*start),
                    end.map(cue_time).unwrap_or_else(|| "整轨末尾".into())
                )))
                .child(
                    div()
                        .text_size(px(10.))
                        .text_color(rgb(theme.tertiary))
                        .child(format!("整轨文件：{}", image.display())),
                )
                .child(
                    div()
                        .text_size(px(10.))
                        .text_color(rgb(theme.tertiary))
                        .child(format!("CUE：{}", cue.display())),
                );
        }
        TrackSource::File { path } => {
            row = row.child(
                div()
                    .text_size(px(10.))
                    .text_color(rgb(theme.tertiary))
                    .child(format!("文件：{}", path.display())),
            );
        }
    }
    row
}

fn duration(milliseconds: Option<u64>) -> String {
    let Some(milliseconds) = milliseconds else {
        return "时长未知".into();
    };
    let seconds = milliseconds / 1000;
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

fn cue_time(frames: u32) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        frames / 75 / 60,
        frames / 75 % 60,
        frames % 75
    )
}
