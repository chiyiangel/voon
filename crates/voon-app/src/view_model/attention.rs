use super::{AppState, Result};
use gpui::Context;
use std::path::Path;
use voon_core::db::AttentionItem;

impl AppState {
    pub fn attention_items(&self) -> &[AttentionItem] {
        &self.attention
    }
    pub fn attention_path(&self) -> Option<&Path> {
        self.attention_path.as_deref()
    }

    pub fn album_scan_issues(&self, album: &Path) -> Vec<&AttentionItem> {
        self.attention
            .iter()
            .filter(|item| item.stage.is_some() && item.album_path.as_deref() == Some(album))
            .collect()
    }

    /// A finite background rescan also retries first-discovery failures that
    /// have no Album to select yet.
    pub fn retry_attention(
        &mut self,
        path: &Path,
        kind: &str,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        if !self
            .attention
            .iter()
            .any(|item| item.path == path && item.kind == kind && item.stage.is_some())
        {
            return Err("读取问题已失效，请刷新后重试".into());
        }
        let root = self
            .roots
            .iter()
            .find(|root| path.starts_with(&root.path))
            .ok_or("音乐库根目录不存在")?
            .path
            .clone();
        self.start_scan(root, cx)
    }

    pub fn open_attention(&mut self, path: &Path, kind: &str, cx: &mut Context<Self>) {
        if let Some(item) = self
            .attention
            .iter()
            .find(|item| item.path == path && item.kind == kind)
            .cloned()
        {
            self.attention_path = Some(item.path);
            self.select_album(item.album_path, cx);
        }
    }

    pub fn confirm_rename(&mut self, old: &Path, new: &Path, cx: &mut Context<Self>) -> Result<()> {
        self.validate_rename_candidate(old, new)?;
        self.index.confirm_rename(old, new)?;
        self.reload_library(cx)?;
        self.select_album(Some(new.to_path_buf()), cx);
        Ok(())
    }

    pub fn dismiss_rename(&mut self, old: &Path, new: &Path, cx: &mut Context<Self>) -> Result<()> {
        self.validate_rename_candidate(old, new)?;
        self.index.dismiss_rename(old, new)?;
        self.reload_library(cx)
    }

    fn validate_rename_candidate(&self, old: &Path, new: &Path) -> Result<()> {
        if self
            .index
            .rename_candidates()?
            .iter()
            .any(|candidate| candidate.old_path == old && candidate.new_path == new)
        {
            Ok(())
        } else {
            Err("改名候选已失效，请刷新后重试".into())
        }
    }
}
