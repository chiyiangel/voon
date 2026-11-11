use super::{AppState, Result};
use crate::platform::PathAction;
use gpui::Context;
use std::path::Path;
use voon_core::TrackSource;

impl AppState {
    pub fn album_player_label(&self) -> Option<&'static str> {
        self.album_detail()?
            .tracks()
            .next()
            .map(|track| player_label(&track.source))
    }

    pub fn track_player_label(&self, disc: usize, track: usize) -> Option<&'static str> {
        self.album_detail()?
            .discs
            .get(disc)?
            .tracks
            .get(track)
            .map(|track| player_label(&track.source))
    }

    /// Open the Album's first audio file; a Virtual track resolves to its image.
    pub fn open_album_in_player(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let album = self.album_detail().ok_or("请先选择专辑")?;
        let track = album
            .tracks()
            .next()
            .ok_or("这张专辑没有可打开的音频文件")?;
        self.open_system_path(track.source.audio_path().clone(), PathAction::Open, cx);
        Ok(())
    }

    pub fn open_track_in_player(
        &mut self,
        album_path: &Path,
        disc: usize,
        track: usize,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        let album = self
            .album_detail()
            .filter(|album| album.path == album_path)
            .ok_or("选中的专辑已改变，请重试")?;
        let track = album
            .discs
            .get(disc)
            .and_then(|disc| disc.tracks.get(track))
            .ok_or("曲目不存在")?;
        self.open_system_path(track.source.audio_path().clone(), PathAction::Open, cx);
        Ok(())
    }
}

fn player_label(source: &TrackSource) -> &'static str {
    match source {
        TrackSource::Virtual { .. } => "用默认播放器打开整个映像",
        TrackSource::File { .. } => "用默认播放器打开",
    }
}
