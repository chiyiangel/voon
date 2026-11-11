use super::{Index, Result};
use crate::scan::{IssueKind, ScanIssue};
use crate::tags::TagFailureKind;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttentionItem {
    pub path: PathBuf,
    pub kind: String,
    pub title: String,
    pub explanation: String,
    pub album_path: Option<PathBuf>,
    pub previous_path: Option<PathBuf>,
    /// Structure failures retain the last accepted Album, when one exists.
    pub stage: Option<String>,
    pub retains_previous: bool,
    /// All affected Albums; the navigation target above is only one shortcut.
    pub affected_albums: Vec<PathBuf>,
}

impl AttentionItem {
    pub fn can_retry_read(&self) -> bool {
        self.stage.is_some()
            || TagFailureKind::from_attention_kind(&self.kind)
                .is_some_and(TagFailureKind::retryable)
    }
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
        IssueKind::CueUnreadable { error } => ("CueUnreadable", format!("分轨表读取失败：{error}")),
        IssueKind::TitleFromFolderOnly => (
            "TitleFromFolderOnly",
            "标题仅来自目录名，尚无标签或分轨表标题".into(),
        ),
        IssueKind::LooseUntagged { file } => (
            "LooseUntagged",
            format!("根目录散音频缺少专辑标签：{}", file.display()),
        ),
        IssueKind::TagRead {
            failure,
            reason,
            retained,
        } => (
            failure.attention_kind(),
            format!(
                "标签读取：{reason}{}",
                if *retained {
                    "；保留上次成功资料，尚未重新确认"
                } else {
                    "；尚无成功资料"
                }
            ),
        ),
    }
}

fn title(kind: &str) -> &'static str {
    if let Some(failure) = TagFailureKind::from_attention_kind(kind) {
        return failure.attention_title();
    }
    match kind {
        "CueTargetMissing" => "分轨表指向的文件不存在",
        "CueWithoutTracks" => "分轨表没有曲目",
        "NestedAlbum" => "专辑里套着专辑",
        "Unreadable" => "目录无法读取",
        "CueUnreadable" => "分轨表无法读取",
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
                let mut album_path = related_album(&path).map(|album| album.path.clone());
                let stage = match kind.as_str() {
                    "Unreadable" => Some("目录列表"),
                    "CueUnreadable" => Some("分轨表读取"),
                    "CueWithoutTracks" | "CueTargetMissing" => Some("分轨表结构"),
                    _ => None,
                };
                let affected_albums: Vec<PathBuf> = if stage.is_some() {
                    albums
                        .iter()
                        .filter(|album| {
                            let loose = album
                                .path
                                .components()
                                .any(|part| part.as_os_str() == crate::LOOSE_DIR);
                            album.source_directories.iter().any(|source| {
                                source.starts_with(&path)
                                    || if loose {
                                        path.parent() == Some(source.as_path())
                                            && path
                                                .extension()
                                                .and_then(|extension| extension.to_str())
                                                .and_then(crate::AudioFormat::from_extension)
                                                .is_some()
                                    } else {
                                        path.starts_with(source)
                                    }
                            })
                        })
                        .map(|album| album.path.clone())
                        .collect()
                } else {
                    vec![]
                };
                if stage.is_some() {
                    album_path = album_path
                        .filter(|album| affected_albums.contains(album))
                        .or_else(|| affected_albums.first().cloned());
                }
                let retains_previous = !affected_albums.is_empty();
                let explanation = if let Some(stage) = stage {
                    format!(
                        "阶段：{stage}；{explanation}。{}",
                        if retains_previous {
                            "已保留上次完整资料，尚未重新确认。"
                        } else {
                            "尚无完整资料，暂不发布专辑。"
                        }
                    )
                } else {
                    explanation
                };
                AttentionItem {
                    title: if stage.is_some() {
                        "本次读取不完整"
                    } else {
                        title(&kind)
                    }
                    .into(),
                    path,
                    kind,
                    explanation,
                    album_path,
                    previous_path,
                    stage: stage.map(str::to_owned),
                    retains_previous,
                    affected_albums,
                }
            })
            .collect())
    }

    /// Structure problems associated with an accepted physical or logical Album.
    pub fn album_scan_issues(&self, album: &Path) -> Result<Vec<AttentionItem>> {
        Ok(self
            .attention_items()?
            .into_iter()
            .filter(|item| {
                item.stage.is_some() && item.affected_albums.iter().any(|path| path == album)
            })
            .collect())
    }
}
