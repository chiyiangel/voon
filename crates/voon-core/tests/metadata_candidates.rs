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
