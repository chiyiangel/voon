use super::{AppState, Result};
use crate::platform::PathAction;
use gpui::Context;
use std::path::Path;

impl AppState {
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
