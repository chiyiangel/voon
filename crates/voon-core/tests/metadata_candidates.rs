use std::path::Path;
use voon_core::{
    AudioFormat, FieldSource,
    db::{CachedTags, Index},
    scan::{FileStamp, TagReader, scan_root},
    tags::TagInfo,
};

struct DifferentTags;
impl TagReader for DifferentTags {
    fn read(&mut self, path: &Path, _: AudioFormat, _: FileStamp) -> Option<TagInfo> {
        Some(TagInfo {
            album: Some(
                if path.ends_with("01.flac") {
                    "Tag A"
                } else {
                    "Tag B"
                }
                .into(),
            ),
            year: Some(if path.ends_with("01.flac") {
                1999
            } else {
                2001
            }),
            ..Default::default()
        })
    }
}

#[test]
fn title_and_year_keep_every_source_before_majority_and_survive_offline_reopen() {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let path = library.path().join("Folder title (1977)");
    std::fs::create_dir(&path).unwrap();
    for file in ["01.flac", "02.flac"] {
        std::fs::write(path.join(file), b"fixture").unwrap();
    }
    std::fs::write(path.join("program.cue"), "TITLE \"Cue title\"\nREM DATE 1988\nFILE \"01.flac\" WAVE\n TRACK 01 AUDIO\n  INDEX 01 00:00:00\nFILE \"02.flac\" WAVE\n TRACK 02 AUDIO\n  INDEX 01 00:00:00\n").unwrap();
    let database = local.path().join("index.sqlite3");
    let mut index = Index::open(&database).unwrap();
    let scan = {
        let mut tags = CachedTags::new(&index, DifferentTags);
        scan_root(library.path(), &mut tags)
    };
    index.store_scan(library.path(), &scan).unwrap();
    let comparison = index.metadata_comparison(&path).unwrap();
    let year = comparison
        .iter()
        .find(|field| field.name == "year")
        .unwrap();
    assert_eq!(year.automatic.as_ref().unwrap().value, "1999");
    assert_eq!(
        year.candidates
            .iter()
            .map(|candidate| candidate.value.as_str())
            .collect::<Vec<_>>(),
        ["1977", "1988", "1999", "2001"]
    );
    assert_eq!(year.candidates[2].source_path, path.join("01.flac"));
    assert_eq!(year.candidates[2].tracks.len(), 1);
    assert!(year.candidates[2].observed_at > 0);
    index.set_user_edit(&path, "year", "2020").unwrap();
    let scan = {
        let mut tags = CachedTags::new(&index, DifferentTags);
        let scan = scan_root(library.path(), &mut tags);
        assert_eq!(tags.reads, 0);
        scan
    };
    index.store_scan(library.path(), &scan).unwrap();
    let current = index.metadata_comparison(&path).unwrap();
    let edited = current.iter().find(|field| field.name == "year").unwrap();
    assert_eq!(
        edited.effective.as_ref().unwrap().source,
        FieldSource::UserEdit
    );
    assert_eq!(edited.effective.as_ref().unwrap().value, "2020");
    assert_eq!(
        edited.candidates[2].observed_at,
        year.candidates[2].observed_at
    );
    drop(index);
    std::fs::rename(library.path(), local.path().join("offline-library")).unwrap();
    let mut index = Index::open(&database).unwrap();
    let current = index.metadata_comparison(&path).unwrap();
    let edited = current.iter().find(|field| field.name == "year").unwrap();
    assert_eq!(edited.candidates.len(), 4);
    index
        .save_user_edits(&path, &[("year".into(), Some("".into()))])
        .unwrap();
    let current = index.metadata_comparison(&path).unwrap();
    let restored = current.iter().find(|field| field.name == "year").unwrap();
    assert_eq!(restored.effective.as_ref().unwrap().value, "1999");
    assert_eq!(restored.candidates, edited.candidates);
}

#[test]
fn a_failed_source_keeps_its_candidate_and_original_time_without_marking_other_files_failed() {
    struct FailingTags;
    impl TagReader for FailingTags {
        fn read(&mut self, _: &Path, _: AudioFormat, _: FileStamp) -> Option<TagInfo> {
            None
        }
        fn read_observation(
            &mut self,
            _: &Path,
            _: AudioFormat,
            _: FileStamp,
        ) -> voon_core::tags::TagObservation {
            voon_core::tags::TagObservation::new(voon_core::tags::TagReadResult::IoFailure(
                "断连".into(),
            ))
        }
    }
    let library = tempfile::tempdir().unwrap();
    let album = library.path().join("Folder (1988)");
    std::fs::create_dir(&album).unwrap();
    for file in ["01.flac", "02.flac"] {
        std::fs::write(album.join(file), b"fixture").unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    let scan = {
        let mut tags = CachedTags::new(&index, DifferentTags);
        scan_root(library.path(), &mut tags)
    };
    index.store_scan(library.path(), &scan).unwrap();
    let before = index.metadata_comparison(&album).unwrap();
    let original = before
        .iter()
        .find(|field| field.name == "title")
        .unwrap()
        .candidates
        .iter()
        .find(|candidate| candidate.source_path.ends_with("01.flac"))
        .unwrap();
    std::fs::write(album.join("01.flac"), b"changed stamp").unwrap();
    let scan = {
        let mut tags = CachedTags::new(&index, FailingTags);
        let scan = scan_root(library.path(), &mut tags);
        assert_eq!(tags.reads, 1);
        scan
    };
    index.store_scan(library.path(), &scan).unwrap();
    let after = index.metadata_comparison(&album).unwrap();
    let title = after.iter().find(|field| field.name == "title").unwrap();
    let failed = title
        .candidates
        .iter()
        .find(|candidate| candidate.source_path.ends_with("01.flac"))
        .unwrap();
    assert_eq!(failed.value, "Tag A");
    assert_eq!(failed.observed_at, original.observed_at);
    assert!(
        matches!(&failed.status, voon_core::SourceStatus::Unconfirmed { reason, .. } if reason == "断连")
    );
    let good = title
        .candidates
        .iter()
        .find(|candidate| candidate.source_path.ends_with("02.flac"))
        .unwrap();
    assert_eq!(good.status, voon_core::SourceStatus::Current);
}

#[test]
fn every_existing_field_keeps_minority_sources_without_inventing_genre_or_orchestra() {
    struct AllTags;
    impl TagReader for AllTags {
        fn read(&mut self, path: &Path, _: AudioFormat, _: FileStamp) -> Option<TagInfo> {
            let minority = path.ends_with("03.flac");
            Some(TagInfo {
                album: Some("Tag album".into()),
                album_artist: Some("Tag album artist".into()),
                artist: Some(if minority { "Solo B" } else { "Solo A" }.into()),
                composer: Some(if minority { "Composer B" } else { "Composer A" }.into()),
                conductor: Some("Tag conductor".into()),
                genre: Some("Classical".into()),
                catalog_number: Some("TAG-123".into()),
                year: Some(1999),
                ..Default::default()
            })
        }
    }
    let library = tempfile::tempdir().unwrap();
    let album = library
        .path()
        .join("Folder artist - Folder title (1977) [12345] [Remaster]");
    std::fs::create_dir(&album).unwrap();
    for file in ["01.flac", "02.flac", "03.flac"] {
        std::fs::write(album.join(file), b"fixture").unwrap();
    }
    std::fs::write(album.join("program.cue"), "TITLE \"Cue album\"\nPERFORMER \"Cue artist\"\nREM GENRE \"Must ignore\"\nREM COMPOSER \"Cue composer\"\nCATALOG CUE-123\nFILE \"01.flac\" WAVE\n TRACK 01 AUDIO\n  PERFORMER \"Cue solo 1\"\n  INDEX 01 00:00:00\nFILE \"02.flac\" WAVE\n TRACK 02 AUDIO\n  PERFORMER \"Cue solo 2\"\n  INDEX 01 00:00:00\n").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    let scan = {
        let mut tags = CachedTags::new(&index, AllTags);
        scan_root(library.path(), &mut tags)
    };
    index.store_scan(library.path(), &scan).unwrap();
    let fields = index.metadata_comparison(&album).unwrap();
    let field = |name: &str| fields.iter().find(|field| field.name == name).unwrap();
    assert_eq!(
        field("composer").automatic.as_ref().unwrap().value,
        "Composer A"
    );
    let minority = field("composer")
        .candidates
        .iter()
        .find(|candidate| candidate.value == "Composer B")
        .unwrap();
    assert_eq!(minority.source_path, album.join("03.flac"));
    assert_eq!(minority.tracks.len(), 1);
    assert_eq!(
        field("performers").automatic.as_ref().unwrap().value,
        "Solo A; Solo B"
    );
    for name in [
        "title",
        "album_artist",
        "year",
        "genre",
        "catalog_number",
        "edition",
        "composer",
        "performers",
        "conductor",
    ] {
        assert!(!field(name).candidates.is_empty(), "missing {name}");
    }
    assert!(
        field("genre")
            .candidates
            .iter()
            .all(|candidate| candidate.source == FieldSource::Tag)
    );
    assert!(field("orchestra").candidates.is_empty());
    assert!(field("orchestra").automatic.is_none());
    index
        .save_user_edits(&album, &[("orchestra".into(), Some("My orchestra".into()))])
        .unwrap();
    let fields = index.metadata_comparison(&album).unwrap();
    let orchestra = fields
        .iter()
        .find(|field| field.name == "orchestra")
        .unwrap();
    assert_eq!(orchestra.user_value.as_deref(), Some("My orchestra"));
    assert!(orchestra.candidates.is_empty());
}

#[test]
fn merge_split_and_undo_keep_only_the_sources_of_each_result() {
    let library = tempfile::tempdir().unwrap();
    let first = library.path().join("First");
    let second = library.path().join("Second");
    for (path, file) in [(&first, "01.flac"), (&second, "02.flac")] {
        std::fs::create_dir(path).unwrap();
        std::fs::write(path.join(file), b"fixture").unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    let scan = {
        let mut tags = CachedTags::new(&index, DifferentTags);
        scan_root(library.path(), &mut tags)
    };
    index.store_scan(library.path(), &scan).unwrap();
    let merged = index
        .merge_albums(&[first.clone(), second.clone()], &first, &[])
        .unwrap();
    let fields = index.metadata_comparison(&merged).unwrap();
    let title = fields.iter().find(|field| field.name == "title").unwrap();
    assert_eq!(
        title
            .candidates
            .iter()
            .filter(|candidate| candidate.source == FieldSource::Tag)
            .count(),
        2
    );
    let parts = index
        .split_album(&merged, voon_core::db::SplitMode::Discs)
        .unwrap();
    for part in &parts {
        let album = index.album(part).unwrap().unwrap();
        let fields = index.metadata_comparison(part).unwrap();
        let title = fields.iter().find(|field| field.name == "title").unwrap();
        let tag = title
            .candidates
            .iter()
            .filter(|candidate| candidate.source == FieldSource::Tag)
            .collect::<Vec<_>>();
        assert_eq!(tag.len(), 1);
        assert_eq!(
            tag[0].source_path,
            *album.tracks().next().unwrap().source.audio_path()
        );
    }
    index.undo_structure().unwrap().unwrap();
    let fields = index.metadata_comparison(&merged).unwrap();
    assert_eq!(
        fields
            .iter()
            .find(|field| field.name == "title")
            .unwrap()
            .candidates
            .len(),
        4
    );
}

#[test]
fn retained_candidates_expose_a_directory_failure_without_inventing_a_new_read_time() {
    let library = tempfile::tempdir().unwrap();
    let album = library.path().join("Album");
    let disc = album.join("CD1");
    std::fs::create_dir_all(&disc).unwrap();
    std::fs::write(disc.join("01.flac"), b"fixture").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    let mut scan = {
        let mut tags = CachedTags::new(&index, DifferentTags);
        scan_root(library.path(), &mut tags)
    };
    index.store_scan(library.path(), &scan).unwrap();
    let fields = index.metadata_comparison(&album).unwrap();
    let time = fields
        .iter()
        .find(|field| field.name == "title")
        .unwrap()
        .candidates
        .iter()
        .find(|candidate| candidate.source == FieldSource::Tag)
        .unwrap()
        .observed_at;
    scan.coverage.incomplete_paths.push(disc.clone());
    scan.issues.push(voon_core::scan::ScanIssue {
        path: disc,
        kind: voon_core::scan::IssueKind::Unreadable {
            error: "Permission denied".into(),
        },
    });
    index.store_scan(library.path(), &scan).unwrap();
    let fields = index.metadata_comparison(&album).unwrap();
    let tag = fields
        .iter()
        .find(|field| field.name == "title")
        .unwrap()
        .candidates
        .iter()
        .find(|candidate| candidate.source == FieldSource::Tag)
        .unwrap();
    assert_eq!(tag.observed_at, time);
    assert!(
        matches!(&tag.status, voon_core::SourceStatus::Unconfirmed { reason, .. } if reason.contains("Permission denied"))
    );
}

#[test]
fn alternate_cue_sources_remain_comparable_without_changing_the_first_sheet_winner() {
    let library = tempfile::tempdir().unwrap();
    let album = library.path().join("Album");
    std::fs::create_dir(&album).unwrap();
    std::fs::write(album.join("image.dff"), b"fixture").unwrap();
    for (file, title) in [("a.cue", "First"), ("b.cue", "Alternate")] {
        std::fs::write(album.join(file), format!("TITLE \"{title}\"\nPERFORMER \"{title}\"\nFILE \"image.dff\" WAVE\n TRACK 01 AUDIO\n INDEX 01 00:00:00\n")).unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(
            library.path(),
            &scan_root(library.path(), &mut voon_core::scan::NoTags),
        )
        .unwrap();
    let fields = index.metadata_comparison(&album).unwrap();
    let title = fields.iter().find(|field| field.name == "title").unwrap();
    assert_eq!(title.automatic.as_ref().unwrap().value, "First");
    assert_eq!(
        title
            .candidates
            .iter()
            .filter(|candidate| candidate.source == FieldSource::Cue)
            .count(),
        2
    );
    let performers = fields
        .iter()
        .find(|field| field.name == "performers")
        .unwrap();
    assert_eq!(performers.automatic.as_ref().unwrap().value, "First");
    assert!(
        performers
            .candidates
            .iter()
            .any(|candidate| candidate.value == "Alternate")
    );
}

#[test]
fn alternate_cue_performers_keep_their_own_boundaries_and_follow_logical_splits() {
    let library = tempfile::tempdir().unwrap();
    let album = library.path().join("Album");
    std::fs::create_dir(&album).unwrap();
    std::fs::write(album.join("image.dff"), b"fixture").unwrap();
    for (file, split, first, second) in [
        ("a.cue", "03:00:00", "Primary first", "Primary second"),
        ("b.cue", "04:00:00", "Alternate first", "Alternate second"),
    ] {
        std::fs::write(album.join(file), format!("TITLE \"{file}\"\nFILE \"image.dff\" WAVE\n TRACK 01 AUDIO\n PERFORMER \"{first}\"\n INDEX 01 00:00:00\n TRACK 02 AUDIO\n PERFORMER \"{second}\"\n INDEX 01 {split}\n")).unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(
            library.path(),
            &scan_root(library.path(), &mut voon_core::scan::NoTags),
        )
        .unwrap();
    let fields = index.metadata_comparison(&album).unwrap();
    let performers = fields
        .iter()
        .find(|field| field.name == "performers")
        .unwrap();
    assert_eq!(
        performers.automatic.as_ref().unwrap().value,
        "Primary first; Primary second"
    );
    let alternate = performers
        .candidates
        .iter()
        .find(|candidate| candidate.value == "Alternate second")
        .unwrap();
    assert!(
        matches!(&alternate.tracks[..], [voon_core::TrackSource::Virtual { cue, start: 18_000, end: None, .. }] if cue == &album.join("b.cue"))
    );
    let first = performers
        .candidates
        .iter()
        .find(|candidate| candidate.value == "Alternate first")
        .unwrap();
    assert!(matches!(
        &first.tracks[..],
        [voon_core::TrackSource::Virtual {
            start: 0,
            end: Some(18_000),
            ..
        }]
    ));
    let parts = index
        .split_album(
            &album,
            voon_core::db::SplitMode::SelectedTracks(vec![voon_core::db::TrackRef {
                disc: 0,
                track: 0,
            }]),
        )
        .unwrap();
    let early = parts
        .iter()
        .find(|part| {
            let album = index.album(part).unwrap().unwrap();
            matches!(
                album.tracks().next().unwrap().source,
                voon_core::TrackSource::Virtual { start: 0, .. }
            )
        })
        .unwrap();
    let fields = index.metadata_comparison(early).unwrap();
    let performers = fields
        .iter()
        .find(|field| field.name == "performers")
        .unwrap();
    assert!(
        !performers
            .candidates
            .iter()
            .any(|candidate| candidate.value == "Alternate second")
    );
    let first = performers
        .candidates
        .iter()
        .find(|candidate| candidate.value == "Alternate first")
        .unwrap();
    assert!(
        matches!(&first.tracks[..], [voon_core::TrackSource::Virtual { cue, start: 0, end: Some(13_500), .. }] if cue == &album.join("b.cue"))
    );
}
