use super::*;

impl AlbumDetail {
    pub(super) fn tracks(
        &mut self,
        album: &Album,
        offline: bool,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let mut body = div().flex().flex_col();
        if album.discs.is_empty() {
            body = body.child(div().p_4().child("没有曲目"));
        }
        for (disc_position, disc) in album.discs.iter().enumerate() {
            let open = self.open_discs.contains(&disc_position);
            let mut section = div()
                .flex_none()
                .p_4()
                .border_b_1()
                .border_color(rgb(theme.line))
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .id(("detail-disc", disc_position))
                        .debug_selector(move || format!("detail-disc-{disc_position}"))
                        .cursor_pointer()
                        .py_2()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!(
                            "{} 碟 {} · {} 轨",
                            if open { "⌄" } else { "›" },
                            disc.number,
                            disc.tracks.len()
                        ))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !this.open_discs.remove(&disc_position) {
                                this.open_discs.insert(disc_position);
                            }
                            cx.notify();
                        })),
                );
            if open {
                for (track_position, track) in disc.tracks.iter().enumerate() {
                    let key = (disc_position, track_position);
                    let expanded = self.open_tracks.contains(&key);
                    let mut row = div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .py_2()
                        .border_b_1()
                        .border_color(rgb(theme.line))
                        .child(
                            div()
                                .id(("detail-track", disc_position * 100_000 + track_position))
                                .debug_selector(move || {
                                    format!("detail-track-{disc_position}-{track_position}")
                                })
                                .cursor_pointer()
                                .child(track_row(track, theme, expanded))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if !this.open_tracks.remove(&key) {
                                        this.open_tracks.insert(key);
                                    }
                                    cx.notify();
                                })),
                        );
                    if expanded && !offline {
                        let state = self.state.clone();
                        let album_path = album.path.clone();
                        let label = state
                            .read(cx)
                            .track_player_label(disc_position, track_position)
                            .unwrap_or("用默认播放器打开");
                        row = row.child(
                            div()
                                .id((
                                    "open-track-player",
                                    disc_position * 100_000 + track_position,
                                ))
                                .debug_selector(move || {
                                    format!("open-track-player-{disc_position}-{track_position}")
                                })
                                .cursor_pointer()
                                .text_xs()
                                .text_color(rgb(theme.accent))
                                .child(label)
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
                        );
                    }
                    section = section.child(row);
                }
            }
            body = body.child(section);
        }
        body.into_any_element()
    }
}

fn track_row(track: &Track, theme: Theme, expanded: bool) -> impl IntoElement {
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
    if !expanded {
        return row;
    }
    if let Some(artist) = &track.artist {
        row = row.child(
            div()
                .text_color(rgb(theme.secondary))
                .child(format!("演奏者：{}", artist.value)),
        );
    }
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
