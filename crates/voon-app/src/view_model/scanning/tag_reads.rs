use std::{
    collections::{BTreeMap, VecDeque},
    path::{Path, PathBuf},
};

use gpui::Context;
use voon_core::{
    Album, AudioFormat, ScanResult,
    db::CachedTags,
    scan::{FileStamp, TagReader},
    tags::{TagInfo, TagObservation},
};

use crate::view_model::{AppState, Result};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScanOperation {
    #[default]
    Incremental,
    Retry,
    ForceTags,
}

impl ScanOperation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Incremental => "扫描",
            Self::Retry => "重试失败项",
            Self::ForceTags => "强制重读标签",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ScanRequest {
    pub root: PathBuf,
    pub operation: ScanOperation,
    pub paths: Vec<PathBuf>,
    /// Loose audio is grouped by tags, so a full structural observation must
    /// replace its old synthetic groups after a successful explicit reread.
    pub full_root: bool,
}

impl ScanRequest {
    pub fn incremental(root: PathBuf) -> Self {
        Self {
            root,
            operation: ScanOperation::Incremental,
            paths: vec![],
            full_root: true,
        }
    }

    pub(super) fn scoped(&self) -> bool {
        self.operation != ScanOperation::Incremental
    }

    pub(super) fn reads(&self, path: &Path) -> bool {
        match self.operation {
            ScanOperation::Incremental => true,
            ScanOperation::Retry => self.paths.iter().any(|scope| path.starts_with(scope)),
            ScanOperation::ForceTags => self.paths.iter().any(|scope| path == scope),
        }
    }

    fn related(&self, path: &Path) -> bool {
        self.paths
            .iter()
            .any(|scope| path.starts_with(scope) || scope.starts_with(path))
    }

    pub(super) fn album_matches(&self, album: &Album) -> bool {
        self.full_root
            || !self.scoped()
            || album
                .tracks()
                .any(|track| self.reads(track.source.audio_path()))
            || (self.operation == ScanOperation::Retry
                && album
                    .physical_directories()
                    .iter()
                    .any(|path| self.related(path)))
    }

    pub(super) fn retain_result(&self, scan: &mut ScanResult) {
        if !self.scoped() || self.full_root {
            return;
        }
        scan.albums.retain(|album| self.album_matches(album));
        let directories: Vec<_> = scan
            .albums
            .iter()
            .flat_map(|album| {
                album
                    .physical_directories()
                    .into_iter()
                    .map(Path::to_path_buf)
            })
            .collect();
        let in_album = |path: &Path| {
            directories
                .iter()
                .any(|directory| path.starts_with(directory))
        };
        scan.issues.retain(|issue| {
            let path = match &issue.kind {
                voon_core::scan::IssueKind::LooseUntagged { file } => file.as_path(),
                _ => issue.path.as_path(),
            };
            in_album(path) || self.related(path)
        });
        scan.tag_observations
            .retain(|path, _| in_album(path) || self.reads(path));
        scan.stamps
            .retain(|(path, _)| in_album(path) || self.reads(path));
        scan.collections.retain(|collection| {
            self.related(&collection.path)
                || directories
                    .iter()
                    .any(|directory| directory.starts_with(&collection.path))
        });
        scan.coverage
            .listed_directories
            .retain(|path| in_album(path) || self.related(path));
        scan.coverage
            .incomplete_paths
            .retain(|path| in_album(path) || self.related(path));
    }
}

pub(super) struct ScopedTags<'a, R: TagReader> {
    pub(super) inner: CachedTags<'a, R>,
    pub(super) request: &'a ScanRequest,
}

impl<R: TagReader> TagReader for ScopedTags<'_, R> {
    fn read(&mut self, path: &Path, format: AudioFormat, stamp: FileStamp) -> Option<TagInfo> {
        self.observe(path, format, stamp)?.info().cloned()
    }

    fn observe(
        &mut self,
        path: &Path,
        format: AudioFormat,
        stamp: FileStamp,
    ) -> Option<TagObservation> {
        if self.request.reads(path) {
            Some(self.inner.read_observation(path, format, stamp))
        } else {
            self.inner.index.tag_observation(path).ok().flatten()
        }
    }
}

impl AppState {
    pub fn scan_operation(&self) -> ScanOperation {
        self.scan_operation
    }

    pub fn album_tag_refresh_notice(&self) -> Option<&'static str> {
        self.album_detail()?.tracks().any(|track| {
            self.roots.iter().any(|root| track.source.audio_path().parent() == Some(root.path.as_path()))
        }).then_some("此专辑含根目录散音频。重读后会按标签更新分组，旧分组可能消失；未选中文件沿用已有标签资料。")
    }

    pub fn force_selected_album_tags(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let album = self.album_detail().ok_or("请先选择要重读的专辑")?;
        let mut by_root: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
        for track in album.tracks() {
            let path = track.source.audio_path();
            let root = self
                .roots
                .iter()
                .find(|root| path.starts_with(&root.path))
                .ok_or("曲目的音乐库根目录已移除")?;
            if self.offline_roots().contains(&root.path) {
                return Err("相关音乐库未连接，请连接后重读标签".into());
            }
            by_root
                .entry(root.path.clone())
                .or_default()
                .push(path.clone());
        }
        let requests: VecDeque<_> = by_root
            .into_iter()
            .map(|(root, mut paths)| {
                paths.sort();
                paths.dedup();
                let full_root = paths
                    .iter()
                    .any(|path| path.parent() == Some(root.as_path()));
                ScanRequest {
                    root,
                    operation: ScanOperation::ForceTags,
                    paths,
                    full_root,
                }
            })
            .collect();
        self.start_requests(requests, cx)
    }
}
