use super::{Index, Result};
use crate::scan::{IssueKind, ScanIssue};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttentionItem {
    pub path: PathBuf,
    pub kind: String,
    pub title: String,
    pub explanation: String,
    pub album_path: Option<PathBuf>,
    pub previous_path: Option<PathBuf>,
}

pub(super) fn describe(issue: &ScanIssue) -> (&'static str, String) {
    match &issue.kind {
        IssueKind::CueTargetMissing { cue, file } => (
            "CueTargetMissing",
            format!("分轨表 {} 找不到音频文件 {file}", cue.display()),
        ),
        IssueKind::CueWithoutTracks { cue } => (
            "CueWithoutTracks",
            format!("分轨表 {} 没有曲目", cue.display()),
        ),
        IssueKind::NestedAlbum { inner } => (
            "NestedAlbum",
            format!("专辑目录里还包含另一张专辑：{}", inner.display()),
        ),
        IssueKind::Unreadable { error } => ("Unreadable", format!("目录无法读取：{error}")),
        IssueKind::TitleFromFolderOnly => (
            "TitleFromFolderOnly",
            "标题仅来自目录名，尚无标签或分轨表标题".into(),
        ),
        IssueKind::LooseUntagged { file } => (
            "LooseUntagged",
            format!("根目录散音频缺少专辑标签：{}", file.display()),
        ),
    }
}

fn title(kind: &str) -> &'static str {
    match kind {
        "CueTargetMissing" => "分轨表指向的文件不存在",
        "CueWithoutTracks" => "分轨表没有曲目",
        "NestedAlbum" => "专辑里套着专辑",
        "Unreadable" => "目录无法读取",
        "TitleFromFolderOnly" => "标题仅来自目录名",
        "LooseUntagged" => "根目录散音频缺少专辑标签",
        "PossibleRename" => "可能是改名或移动",
        _ => "需要核对的音乐库问题",
    }
}

impl Index {
    /// Presentation-ready explanations retain exact filesystem paths as data.
    pub fn attention_items(&self) -> Result<Vec<AttentionItem>> {
        let albums = self.albums()?;
        let renames = self.rename_candidates()?;
        let related_album = |path: &Path| {
            albums
                .iter()
                .filter_map(|album| {
                    std::iter::once(&album.path)
                        .chain(&album.source_directories)
                        .filter(|source| path.starts_with(source))
                        .map(|source| source.components().count())
                        .max()
                        .map(|depth| (depth, album))
                })
                .max_by_key(|(depth, _)| *depth)
                .map(|(_, album)| album)
        };
        Ok(self
            .issues()?
            .into_iter()
            .filter(|(path, kind, _)| {
                kind != "TitleFromFolderOnly"
                    || !related_album(path).is_some_and(|album| {
                        album.metadata.title.as_ref().is_some_and(|title| {
                            title.source != crate::FieldSource::FolderName
                                && !title.value.trim().is_empty()
                        })
                    })
            })
            .map(|(path, kind, detail)| {
                let previous_path = renames
                    .iter()
                    .find(|candidate| candidate.new_path == path && kind == "PossibleRename")
                    .map(|candidate| candidate.old_path.clone());
                let explanation = if let Some(previous) = &previous_path {
                    format!("{} → {}", previous.display(), path.display())
                } else if detail.starts_with(&kind) {
                    // Older indexes stored Rust debug text. Keep them readable until
                    // the next rescan replaces their explanations with structured prose.
                    title(&kind).to_string()
                } else {
                    detail
                };
                let album_path = related_album(&path).map(|album| album.path.clone());
                AttentionItem {
                    title: title(&kind).into(),
                    path,
                    kind,
                    explanation,
                    album_path,
                    previous_path,
                }
            })
            .collect())
    }
}
