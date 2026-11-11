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
