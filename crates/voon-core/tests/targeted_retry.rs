use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
use voon_core::{
    AudioFormat, AudioProperties, TrackSource,
    db::{CachedTags, Index},
    scan::{
        FileStamp, IssueKind, ScanEvent, ScanIssue, ScanOptions, ScanResult, TagReader,
        retry_tag_paths, scan_root, scan_scopes_with_options,
    },
    tags::{TagInfo, TagObservation, TagReadResult},
};

struct Reader {
    result: TagReadResult,
    expected: Option<PathBuf>,
}
impl TagReader for Reader {
    fn read(&mut self, _: &Path, _: AudioFormat, _: FileStamp) -> Option<TagInfo> {
        None
    }
    fn read_observation(&mut self, path: &Path, _: AudioFormat, _: FileStamp) -> TagObservation {
        if let Some(expected) = &self.expected {
            assert_eq!(path, expected);
        }
        TagObservation::new(self.result.clone())
    }
}
fn failure() -> Reader {
    Reader {
        result: TagReadResult::ParseFailure("invalid utf-8".into()),
        expected: None,
    }
}
fn retry(index: &Index, root: &Path, path: &Path, result: TagReadResult) -> ScanResult {
    let mut tags = CachedTags::new(
        index,
        Reader {
            result,
            expected: Some(path.into()),
        },
    )
    .with_forced_paths([path.into()]);
    let scan = retry_tag_paths(root, &[path.into()], &mut tags, &AtomicBool::new(false));
    assert_eq!(tags.reads, 1);
    assert!(scan.coverage.listed_directories.is_empty());
    assert!(scan.albums.is_empty());
    scan
}

#[test]
fn tag_retry_preserves_cue_structure_other_failures_and_user_edits_without_walking() {
    let library = tempfile::tempdir().unwrap();
    let root = library.path();
    let album = root.join("Symphonies");
    let cd1 = album.join("CD1");
    let cd2 = album.join("CD2");
    for dir in [&cd1, &cd2] {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("image.ape"), b"unchanged source").unwrap();
    }
    let audio = cd1.join("image.ape");
    let cue = cd1.join("image.cue");
    fs::write(&cue, "TITLE \"CUE title\"\nFILE \"image.wav\" WAVE\n TRACK 01 AUDIO\n TITLE \"First\"\n INDEX 01 00:00:00\n TRACK 02 AUDIO\n TITLE \"Second\"\n INDEX 01 01:00:00\n").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    let initial = scan_root(root, &mut CachedTags::new(&index, failure()));
    index.store_scan(root, &initial).unwrap();
    index.set_user_edit(&album, "title", "我的整理").unwrap();
    let structural = ScanResult {
        issues: vec![ScanIssue {
            path: cd2.join("unreadable.cue"),
            kind: IssueKind::CueUnreadable {
                error: "keep this issue".into(),
            },
        }],
        ..Default::default()
    };
    index.store_partial_scan(root, &structural).unwrap();
    let other = index.tag_observation(&cd2.join("image.ape")).unwrap();
    let old = index.album(&album).unwrap().unwrap();
    let bytes = fs::read(&audio).unwrap();
    let cue_bytes = fs::read(&cue).unwrap();
    let stamp = FileStamp::of(&audio);
    // Make the sibling subtree unavailable: retry must rely on indexed data.
    fs::remove_file(cd2.join("image.ape")).unwrap();
    fs::remove_dir(&cd2).unwrap();
    let scan = retry(
        &index,
        root,
        &audio,
        TagReadResult::Tagged(TagInfo {
            album: Some("Recovered".into()),
            year: Some(1993),
            properties: AudioProperties {
                duration_ms: Some(180_000),
                sample_rate: Some(44_100),
                ..Default::default()
            },
            ..Default::default()
        }),
    );
    index.store_tag_retry(root, &scan).unwrap();
    let saved = index.album(&album).unwrap().unwrap();
    assert_eq!(saved.metadata.title.as_ref().unwrap().value, "我的整理");
    assert_eq!(
        index
            .automatic_metadata(&album)
            .unwrap()
            .unwrap()
            .title
            .unwrap()
            .value,
        "Recovered"
    );
    assert_eq!(saved.metadata.year.as_ref().unwrap().value, 1993);
    assert_eq!(
        saved
            .tracks()
            .map(|track| track.source.clone())
            .collect::<Vec<_>>(),
        old.tracks()
            .map(|track| track.source.clone())
            .collect::<Vec<_>>()
    );
    let tracks: Vec<_> = saved
        .tracks()
        .filter(|track| track.source.audio_path() == &audio)
        .collect();
    assert!(matches!(tracks[0].source, TrackSource::Virtual { .. }));
    assert_eq!(tracks[0].properties.duration_ms, Some(60_000));
    assert_eq!(tracks[1].properties.duration_ms, Some(120_000));
    assert_eq!(tracks[0].title.as_ref().unwrap().value, "First");
    assert_eq!(
        index.tag_observation(&cd2.join("image.ape")).unwrap(),
        other
    );
    let issues = index.attention_items().unwrap();
    assert!(
        !issues
            .iter()
            .any(|issue| issue.path == audio && issue.kind == "TagParseFailure")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.path == cd2.join("image.ape") && issue.kind == "TagParseFailure")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.path == cd2.join("unreadable.cue") && issue.kind == "CueUnreadable")
    );
    assert_eq!(fs::read(&audio).unwrap(), bytes);
    assert_eq!(fs::read(&cue).unwrap(), cue_bytes);
    assert_eq!(FileStamp::of(&audio), stamp);
}

#[test]
fn loose_retry_groups_only_current_sources_and_keeps_unrelated_ungrouped_failures() {
    let library = tempfile::tempdir().unwrap();
    let root = library.path();
    let a = root.join("a.flac");
    let b = root.join("b.flac");
    let deleted = root.join("deleted.flac");
    for path in [&a, &b, &deleted] {
        fs::write(path, b"source").unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    let scan = scan_root(root, &mut CachedTags::new(&index, failure()));
    index.store_scan(root, &scan).unwrap();
    // Cache entries intentionally outlive structural removals.
    fs::remove_file(&deleted).unwrap();
    let scan = scan_root(root, &mut CachedTags::new(&index, failure()));
    index.store_scan(root, &scan).unwrap();
    let other_issues: Vec<_> = index
        .attention_items()
        .unwrap()
        .into_iter()
        .filter(|item| item.path == b || item.explanation.contains("b.flac"))
        .collect();
    let scan = retry(
        &index,
        root,
        &a,
        TagReadResult::Tagged(TagInfo {
            album: Some("Recovered group".into()),
            ..Default::default()
        }),
    );
    index.store_tag_retry(root, &scan).unwrap();
    let album = index
        .album(&root.join(voon_core::LOOSE_DIR).join("_/Recovered group"))
        .unwrap()
        .unwrap();
    assert_eq!(album.tracks().count(), 1);
    assert_eq!(album.tracks().next().unwrap().source.audio_path(), &a);
    let issues = index.attention_items().unwrap();
    assert!(
        other_issues.iter().all(
            |previous| issues.iter().any(|issue| issue.path == previous.path
                && issue.kind == previous.kind
                && issue.explanation == previous.explanation)
        ),
        "before: {other_issues:?}\nafter: {issues:?}"
    );
    assert!(
        !issues
            .iter()
            .any(|issue| issue.path == deleted || issue.explanation.contains("deleted.flac"))
    );
    assert!(
        !issues
            .iter()
            .any(|issue| issue.explanation.contains("a.flac"))
    );
}

#[test]
fn structural_retry_visits_only_the_affected_album_and_keeps_disc_identity() {
    let library = tempfile::tempdir().unwrap();
    let root = library.path();
    let album = root.join("Album");
    let other = root.join("Unrelated/DoNotWalk");
    for path in [album.join("CD1"), album.join("CD2"), other.clone()] {
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("image.flac"), b"fixture").unwrap();
    }
    let original = scan_root(root, &mut voon_core::scan::NoTags);
    let mut visited = vec![];
    let scan = scan_scopes_with_options(
        root,
        &[album.join("CD1/image.cue")],
        &original.albums,
        &mut voon_core::scan::NoTags,
        &AtomicBool::new(false),
        &mut |event| {
            if let ScanEvent::Directory(path) = event {
                visited.push(path.to_path_buf());
            }
        },
        &ScanOptions::default(),
    );
    assert_eq!(visited, vec![album.clone()]);
    assert!(
        scan.coverage
            .listed_directories
            .iter()
            .all(|path| path.starts_with(&album))
    );
    assert_eq!(scan.albums.len(), 1);
    assert_eq!(scan.albums[0].path, album);
    assert_eq!(scan.albums[0].discs.len(), 2);
    let mut visited = vec![];
    scan_scopes_with_options(
        root,
        &[root.join("missing.flac")],
        &original.albums,
        &mut voon_core::scan::NoTags,
        &AtomicBool::new(false),
        &mut |event| {
            if let ScanEvent::Directory(path) = event {
                visited.push(path.to_path_buf());
            }
        },
        &ScanOptions::default(),
    );
    assert_eq!(
        visited,
        vec![root.to_path_buf()],
        "root-file recovery must not descend into album folders"
    );
}

#[test]
fn retry_keeps_local_merge_and_retains_success_when_the_source_disappears() {
    let library = tempfile::tempdir().unwrap();
    let root = library.path();
    let a = root.join("A");
    let b = root.join("B");
    for dir in [&a, &b] {
        fs::create_dir(dir).unwrap();
        fs::write(dir.join("01.flac"), b"fixture").unwrap();
    }
    let audio = a.join("01.flac");
    let mut index = Index::open_in_memory().unwrap();
    let initial = scan_root(
        root,
        &mut CachedTags::new(
            &index,
            Reader {
                result: TagReadResult::Tagged(TagInfo {
                    album: Some("Initial".into()),
                    ..Default::default()
                }),
                expected: None,
            },
        ),
    );
    index.store_scan(root, &initial).unwrap();
    let merged = index
        .merge_albums(&[a.clone(), b.clone()], &a, &[])
        .unwrap();
    index
        .set_user_edit(&merged, "title", "Merged edit")
        .unwrap();
    let scan = retry(
        &index,
        root,
        &audio,
        TagReadResult::Tagged(TagInfo {
            album: Some("New tags".into()),
            year: Some(1999),
            ..Default::default()
        }),
    );
    index.store_tag_retry(root, &scan).unwrap();
    let saved = index.album(&merged).unwrap().unwrap();
    assert_eq!(saved.tracks().count(), 2);
    assert_eq!(saved.metadata.title.as_ref().unwrap().value, "Merged edit");
    assert_eq!(saved.metadata.year.as_ref().unwrap().value, 1999);
    fs::remove_file(&audio).unwrap();
    let mut tags = CachedTags::new(&index, voon_core::scan::DiskTags);
    let scan = retry_tag_paths(
        root,
        std::slice::from_ref(&audio),
        &mut tags,
        &AtomicBool::new(false),
    );
    assert_eq!(tags.reads, 1);
    assert!(matches!(
        scan.tag_observations[&audio].result,
        TagReadResult::IoFailure(_)
    ));
    index.store_tag_retry(root, &scan).unwrap();
    let retained = index.album(&merged).unwrap().unwrap();
    assert_eq!(retained.tracks().count(), 2);
    assert_eq!(retained.metadata, saved.metadata);
    assert!(
        index
            .attention_items()
            .unwrap()
            .iter()
            .any(|item| item.path == audio
                && item.kind == "TagIoFailure"
                && item.explanation.contains("保留上次成功资料"))
    );
    let before = index.metadata_comparison(&merged).unwrap();
    let cancelled = retry_tag_paths(
        root,
        &[audio],
        &mut CachedTags::new(&index, failure()),
        &AtomicBool::new(true),
    );
    assert!(cancelled.cancelled);
    assert!(cancelled.tag_observations.is_empty());
    index.store_tag_retry(root, &cancelled).unwrap();
    assert_eq!(index.metadata_comparison(&merged).unwrap(), before);
}

#[test]
fn structural_retry_keeps_unvisited_collection_ancestors_and_other_albums() {
    let library = tempfile::tempdir().unwrap();
    let root = library.path();
    let collection = root.join("Collection");
    let album = collection.join("Album");
    let other = collection.join("Other");
    for dir in [&album, &other] {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("01.flac"), b"fixture").unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    let original = scan_root(root, &mut voon_core::scan::NoTags);
    index.store_scan(root, &original).unwrap();
    let before = index.collections().unwrap();
    let path = album.join("01.cue");
    let scan = scan_scopes_with_options(
        root,
        std::slice::from_ref(&path),
        &original.albums,
        &mut voon_core::scan::NoTags,
        &AtomicBool::new(false),
        &mut |_| {},
        &ScanOptions::default(),
    );
    index.store_scoped_scan(root, &scan, &[path]).unwrap();
    assert_eq!(index.collections().unwrap(), before);
    assert!(index.album(&other).unwrap().is_some());
}
