//! Rename/move candidates across rescans, loose audio in a library root,
//! and nested albums.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use voon_core::db::{Index, RenameCandidate};
use voon_core::scan::{FileStamp, IssueKind, NoTags, TagReader};
use voon_core::tags::TagInfo;
use voon_core::*;

fn touch(root: &Path, rel: &str, bytes: &[u8]) -> PathBuf {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, bytes).unwrap();
    path
}

/// Tags keyed by file name, standing in for real audio files.
struct FakeTags(HashMap<String, TagInfo>);

impl TagReader for FakeTags {
    fn read(&mut self, path: &Path, _: AudioFormat, _: FileStamp) -> Option<TagInfo> {
        let name = path.file_name()?.to_str()?;
        self.0.get(name).cloned()
    }
}

fn tagged(
    album: Option<&str>,
    album_artist: Option<&str>,
    track: u32,
    disc: Option<u32>,
) -> TagInfo {
    TagInfo {
        title: Some(format!("Track {track}")),
        album: album.map(str::to_string),
        album_artist: album_artist.map(str::to_string),
        track: Some(track),
        disc,
        ..Default::default()
    }
}

fn library() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path();
    touch(r, "Old Name/CD1/01.flac", b"aaaa");
    touch(r, "Old Name/CD2/01.flac", b"bbbbbb");
    touch(r, "Other/01.flac", b"cc");
    dir
}

#[test]
fn rename_is_proposed_and_confirm_migrates_user_data() {
    let lib = library();
    let root = lib.path();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();

    let old = root.join("Old Name");
    index.set_user_edit(&old, "title", "我的标题").unwrap();
    index.add_user_tag(&old, "收藏").unwrap();
    let mix = index.create_manual_collection("周末").unwrap();
    index.add_to_manual_collection(mix, &old).unwrap();

    // The user renames and moves the folder on the NAS.
    let new = root.join("Box/New Name [24-96]");
    fs::create_dir_all(new.parent().unwrap()).unwrap();
    fs::rename(&old, &new).unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();

    let candidates = index.rename_candidates().unwrap();
    assert_eq!(
        candidates,
        vec![RenameCandidate {
            old_path: old.clone(),
            new_path: new.clone()
        }]
    );
    assert!(
        index
            .issues()
            .unwrap()
            .iter()
            .any(|(p, kind, _)| p == &new && kind == "PossibleRename")
    );

    // A further rescan keeps the candidate until the user decides.
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    assert_eq!(index.rename_candidates().unwrap().len(), 1);

    let moved = index.confirm_rename(&old, &new).unwrap();
    assert_eq!(
        (moved.edits, moved.tags, moved.manual_collections),
        (1, 1, 1)
    );
    assert!(index.rename_candidates().unwrap().is_empty());
    let albums = index.albums().unwrap();
    let album = albums.iter().find(|a| a.path == new).unwrap();
    assert_eq!(
        album.metadata.title,
        Some(Field::new("我的标题".into(), FieldSource::UserEdit))
    );
    assert_eq!(index.user_tags(&new).unwrap(), vec!["收藏"]);
    assert!(index.user_tags(&old).unwrap().is_empty());
    assert_eq!(index.manual_collection_albums(mix).unwrap(), vec![new]);
}

#[test]
fn changed_content_or_duplicates_are_not_paired() {
    let lib = library();
    let root = lib.path();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();

    // Renamed, but a file also changed size: not the same album.
    fs::rename(root.join("Other"), root.join("Other 2")).unwrap();
    fs::write(root.join("Other 2/01.flac"), b"changed").unwrap();
    // Renamed, and an identical copy appeared too: ambiguous.
    fs::rename(root.join("Old Name"), root.join("Copy A")).unwrap();
    touch(root, "Copy B/CD1/01.flac", b"aaaa");
    touch(root, "Copy B/CD2/01.flac", b"bbbbbb");
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    assert!(index.rename_candidates().unwrap().is_empty());
}

#[test]
fn dismissed_and_stale_candidates_go_away() {
    let lib = library();
    let root = lib.path();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    fs::rename(root.join("Other"), root.join("Other 2")).unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    assert_eq!(index.rename_candidates().unwrap().len(), 1);
    index
        .dismiss_rename(&root.join("Other"), &root.join("Other 2"))
        .unwrap();
    assert!(index.rename_candidates().unwrap().is_empty());

    // Renamed back before the user decided: the candidate is stale.
    fs::rename(root.join("Old Name"), root.join("Tmp")).unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    assert_eq!(index.rename_candidates().unwrap().len(), 1);
    fs::rename(root.join("Tmp"), root.join("Old Name")).unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    assert!(index.rename_candidates().unwrap().is_empty());
}

#[test]
fn candidate_expires_when_the_destination_content_changes_before_confirmation() {
    let lib = library();
    let root = lib.path();
    let old = root.join("Other");
    let new = root.join("Other 2");
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    index
        .set_user_edit(&old, "title", "原专辑的整理结果")
        .unwrap();
    index.add_user_tag(&old, "原专辑标签").unwrap();

    fs::rename(&old, &new).unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    assert_eq!(index.rename_candidates().unwrap().len(), 1);

    // The proposed destination is replaced before the user confirms it.
    // Its path remains present, but its audio no longer has the same signature.
    fs::write(
        new.join("01.flac"),
        b"a different album with a different size",
    )
    .unwrap();
    index
        .store_partial_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    assert!(index.confirm_rename(&old, &new).is_err());
    assert_eq!(index.user_tags(&old).unwrap(), ["原专辑标签"]);
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    let candidates = index.rename_candidates().unwrap();
    let confirmation = index.confirm_rename(&old, &new);
    assert!(
        candidates.is_empty() && confirmation.is_err(),
        "内容已变化的候选必须清除并拒绝迁移：candidates={candidates:?}, confirmation={confirmation:?}"
    );
    assert_eq!(index.user_tags(&old).unwrap(), ["原专辑标签"]);
    assert!(index.user_tags(&new).unwrap().is_empty());
}

#[test]
fn loose_root_audio_is_grouped_by_album_tags() {
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path();
    for f in [
        "a1.flac",
        "a2.flac",
        "b1.mp3",
        "c1.flac",
        "untagged.flac",
        "noalbum.flac",
    ] {
        touch(r, f, b"x");
    }
    touch(r, "CD1/01.flac", b"x");
    let mut tags = FakeTags(HashMap::from([
        (
            "a2.flac".into(),
            tagged(Some("淡淡幽情"), Some("邓丽君"), 2, Some(1)),
        ),
        (
            "a1.flac".into(),
            tagged(Some("淡淡幽情"), Some("邓丽君"), 1, Some(1)),
        ),
        // Same album title, different album artist: a different album.
        (
            "b1.mp3".into(),
            tagged(Some("淡淡幽情"), Some("翻唱"), 1, None),
        ),
        // Disc 2 of the first album.
        (
            "c1.flac".into(),
            tagged(Some("淡淡幽情"), Some("邓丽君"), 1, Some(2)),
        ),
        ("noalbum.flac".into(), tagged(None, Some("邓丽君"), 1, None)),
    ]));
    let scan = scan_root(r, &mut tags);

    let loose: Vec<&Album> = scan
        .albums
        .iter()
        .filter(|a| a.path.starts_with(r.join(LOOSE_DIR)))
        .collect();
    assert_eq!(loose.len(), 2);
    let deng = loose
        .iter()
        .find(|a| a.path == r.join(LOOSE_DIR).join("邓丽君").join("淡淡幽情"))
        .unwrap();
    assert_eq!(deng.display_title(), "淡淡幽情");
    assert_eq!(
        deng.metadata.title.as_ref().unwrap().source,
        FieldSource::Tag
    );
    assert_eq!(
        deng.discs.iter().map(|d| d.number).collect::<Vec<_>>(),
        vec![1, 2]
    );
    let disc1: Vec<_> = deng.discs[0].tracks.iter().map(|t| t.number).collect();
    assert_eq!(disc1, vec![Some(1), Some(2)]);

    let mut untagged: Vec<String> = scan
        .issues
        .iter()
        .filter_map(|i| match &i.kind {
            IssueKind::LooseUntagged { file } => {
                Some(file.file_name().unwrap().to_string_lossy().into_owned())
            }
            _ => None,
        })
        .collect();
    untagged.sort();
    assert_eq!(untagged, vec!["noalbum.flac", "untagged.flac"]);

    // A `CD1` folder in the root is an album of its own, not a disc.
    assert!(scan.albums.iter().any(|a| a.path == r.join("CD1")));
    // No album is named after the root itself.
    assert!(scan.albums.iter().all(|a| a.path != r));

    // Loose albums round-trip through the index.
    let mut index = Index::open_in_memory().unwrap();
    index.store_scan(r, &scan).unwrap();
    assert_eq!(index.albums().unwrap().len(), scan.albums.len());
}

#[cfg(unix)]
#[test]
fn an_unrelated_failed_child_does_not_block_loose_audio_updates_or_new_groups() {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let root = library.path();
    touch(root, "a1.flac", b"first");
    let mut tags = FakeTags(HashMap::from([
        (
            "a1.flac".into(),
            tagged(Some("原分组"), Some("艺术家"), 1, None),
        ),
        (
            "a2.flac".into(),
            tagged(Some("原分组"), Some("艺术家"), 2, None),
        ),
        (
            "b1.flac".into(),
            tagged(Some("新分组"), Some("艺术家"), 1, None),
        ),
    ]));
    let database = local.path().join("index.sqlite3");
    let mut index = Index::open(&database).unwrap();
    index.store_scan(root, &scan_root(root, &mut tags)).unwrap();
    touch(root, "a2.flac", b"second");
    touch(root, "b1.flac", b"new group");
    std::os::unix::fs::symlink(root.join("不存在"), root.join("无关专辑链接")).unwrap();
    index.store_scan(root, &scan_root(root, &mut tags)).unwrap();
    let reopened = Index::open(&database).unwrap();
    let original = root.join(LOOSE_DIR).join("艺术家/原分组");
    let added = root.join(LOOSE_DIR).join("艺术家/新分组");
    assert_eq!(
        reopened.album(&original).unwrap().unwrap().tracks().count(),
        2
    );
    assert_eq!(reopened.album(&added).unwrap().unwrap().tracks().count(), 1);
    assert!(
        reopened
            .attention_items()
            .unwrap()
            .iter()
            .any(|item| item.path == root.join("无关专辑链接"))
    );
}

#[test]
fn confirming_a_child_retry_does_not_remove_unobserved_loose_audio() {
    let library = tempfile::tempdir().unwrap();
    let root = library.path();
    touch(root, "loose.flac", b"root audio");
    touch(root, "重试范围/专辑/01.flac", b"nested audio");
    let mut tags = FakeTags(HashMap::from([(
        "loose.flac".into(),
        tagged(Some("散音频"), Some("艺术家"), 1, None),
    )]));
    let mut index = Index::open_in_memory().unwrap();
    index.store_scan(root, &scan_root(root, &mut tags)).unwrap();
    fs::remove_dir_all(root.join("重试范围/专辑")).unwrap();
    let mut scoped = scan_root(root, &mut NoTags);
    scoped.issues.clear();
    index
        .store_scoped_scan(root, &scoped, &[root.join("重试范围")])
        .unwrap();
    assert!(
        index
            .album(&root.join(LOOSE_DIR).join("艺术家/散音频"))
            .unwrap()
            .is_some()
    );
    assert!(index.album(&root.join("重试范围/专辑")).unwrap().is_none());
}

#[test]
fn scoped_structural_commits_preserve_outside_state_and_the_complete_rename_baseline() {
    let library = tempfile::tempdir().unwrap();
    let root = library.path();
    touch(root, "范围/旧路径/scoped.flac", b"longer scoped audio");
    touch(root, "范围外/other.flac", b"other");
    let mut tags = FakeTags(HashMap::from([(
        "other.flac".into(),
        tagged(Some("范围外已知标题"), None, 1, None),
    )]));
    let mut index = Index::open_in_memory().unwrap();
    index.store_scan(root, &scan_root(root, &mut tags)).unwrap();
    let old = root.join("范围/旧路径");
    let new = root.join("范围/新路径");
    fs::rename(&old, &new).unwrap();
    let current = scan_root(root, &mut NoTags);
    index
        .store_scoped_scan(root, &current, &[root.join("范围")])
        .unwrap();
    assert_eq!(
        index
            .album(&root.join("范围外"))
            .unwrap()
            .unwrap()
            .display_title(),
        "范围外已知标题"
    );
    assert!(index.album(&old).unwrap().is_none());
    assert!(index.album(&new).unwrap().is_some());
    assert!(index.rename_candidates().unwrap().is_empty());
    let mut cancelled = voon_core::ScanResult {
        cancelled: true,
        ..Default::default()
    };
    cancelled
        .coverage
        .listed_directories
        .push(root.to_path_buf());
    index
        .store_scoped_scan(root, &cancelled, &[root.join("范围")])
        .unwrap();
    assert!(index.album(&new).unwrap().is_some());
    index.store_scan(root, &current).unwrap();
    assert_eq!(
        index.rename_candidates().unwrap(),
        [RenameCandidate {
            old_path: old,
            new_path: new
        }]
    );
}

#[cfg(unix)]
#[test]
fn an_unknown_root_audio_entry_never_reports_a_partial_loose_album_as_complete() {
    let library = tempfile::tempdir().unwrap();
    let root = library.path();
    touch(root, "b.wav", b"remaining audio");
    std::os::unix::fs::symlink(root.join("missing"), root.join("a.wav")).unwrap();
    let mut tags = FakeTags(HashMap::from([(
        "b.wav".into(),
        tagged(Some("共同分组"), None, 2, None),
    )]));
    let mut reported = vec![];
    let scan = voon_core::scan_root_with(
        root,
        &mut tags,
        &std::sync::atomic::AtomicBool::new(false),
        &mut |event| {
            if let voon_core::scan::ScanEvent::Album(album) = event {
                reported.push(album.path.clone());
            }
        },
    );
    assert!(reported.is_empty());
    assert!(scan.albums.is_empty());
    assert!(
        scan.incomplete_paths()
            .contains(&root.join("a.wav").as_path())
    );
}

#[test]
fn nested_album_is_kept_and_flagged() {
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path();
    touch(r, "Outer/01.flac", b"x");
    touch(r, "Outer/Bonus Album/01.flac", b"x");
    let scan = scan_root(r, &mut NoTags);
    assert_eq!(scan.albums.len(), 2);
    assert!(scan.issues.iter().any(|i| i.path == r.join("Outer")
        && i.kind
            == IssueKind::NestedAlbum {
                inner: r.join("Outer/Bonus Album")
            }));
}
