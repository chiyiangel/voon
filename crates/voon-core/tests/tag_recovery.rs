use std::path::Path;
use voon_core::{
    AudioFormat,
    db::{CachedTags, Index},
    scan::{FileStamp, TagReader, scan_root},
    tags::{TagInfo, TagObservation, TagReadResult},
};

struct Reader(TagReadResult);
impl TagReader for Reader {
    fn read(&mut self, _: &Path, _: AudioFormat, _: FileStamp) -> Option<TagInfo> {
        None
    }
    fn read_observation(&mut self, _: &Path, _: AudioFormat, _: FileStamp) -> TagObservation {
        TagObservation::new(self.0.clone())
    }
}

#[test]
fn a_repaired_dsf_with_the_same_stamp_is_retried_after_a_header_parse_failure() {
    let library = tempfile::tempdir().unwrap();
    let album = library.path().join("DSF");
    std::fs::create_dir(&album).unwrap();
    let audio = album.join("01.dsf");
    std::fs::write(&audio, [0u8; 80]).unwrap();
    let stamp = FileStamp::of(&audio).unwrap();
    let modified = std::fs::metadata(&audio).unwrap().modified().unwrap();
    let mut index = Index::open_in_memory().unwrap();
    let scan = {
        let mut tags = CachedTags::new(&index, voon_core::scan::DiskTags);
        let scan = scan_root(library.path(), &mut tags);
        assert_eq!(tags.reads, 1);
        scan
    };
    index.store_scan(library.path(), &scan).unwrap();
    assert!(matches!(
        index.tag_observation(&audio).unwrap().unwrap().result,
        TagReadResult::ParseFailure(_)
    ));

    let mut repaired = [0u8; 80];
    repaired[..4].copy_from_slice(b"DSD ");
    repaired[4..12].copy_from_slice(&28u64.to_le_bytes());
    repaired[12..20].copy_from_slice(&80u64.to_le_bytes());
    repaired[28..32].copy_from_slice(b"fmt ");
    repaired[32..40].copy_from_slice(&52u64.to_le_bytes());
    repaired[52..56].copy_from_slice(&2u32.to_le_bytes());
    repaired[56..60].copy_from_slice(&2_822_400u32.to_le_bytes());
    repaired[60..64].copy_from_slice(&1u32.to_le_bytes());
    repaired[64..72].copy_from_slice(&8_467_200u64.to_le_bytes());
    std::fs::write(&audio, repaired).unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&audio)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    assert_eq!(FileStamp::of(&audio), Some(stamp));

    let scan = {
        let mut tags = CachedTags::new(&index, voon_core::scan::DiskTags);
        let scan = scan_root(library.path(), &mut tags);
        assert_eq!(tags.reads, 1, "普通重扫应重试同 stamp 的 DSF 解析失败");
        scan
    };
    index.store_scan(library.path(), &scan).unwrap();
    assert!(matches!(
        index.tag_observation(&audio).unwrap().unwrap().result,
        TagReadResult::NoTags(_)
    ));
    assert_eq!(
        index.album(&album).unwrap().unwrap().discs[0].tracks[0].properties,
        voon_core::AudioProperties {
            sample_rate: Some(2_822_400),
            bit_depth: Some(1),
            channels: Some(2),
            duration_ms: Some(3_000),
        }
    );
    let mut tags = CachedTags::new(&index, voon_core::scan::DiskTags);
    scan_root(library.path(), &mut tags);
    assert_eq!(tags.reads, 0, "成功无标签的 DSF 应继续复用缓存");
}

#[test]
fn forcing_selected_audio_rereads_it_without_changing_other_successful_observations() {
    let library = tempfile::tempdir().unwrap();
    let selected = library.path().join("甲");
    let other = library.path().join("乙");
    for album in [&selected, &other] {
        std::fs::create_dir(album).unwrap();
        std::fs::write(album.join("01.flac"), b"unchanged audio").unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    let scan = {
        let mut tags = CachedTags::new(
            &index,
            Reader(TagReadResult::Tagged(TagInfo {
                album: Some("原标签".into()),
                ..Default::default()
            })),
        );
        scan_root(library.path(), &mut tags)
    };
    index.store_scan(library.path(), &scan).unwrap();
    let untouched = index.tag_observation(&other.join("01.flac")).unwrap();
    let scan = {
        let mut tags = CachedTags::new(
            &index,
            Reader(TagReadResult::Tagged(TagInfo {
                album: Some("强制重读的标签".into()),
                ..Default::default()
            })),
        )
        .with_forced_paths([selected.join("01.flac")]);
        let scan = scan_root(library.path(), &mut tags);
        assert_eq!(tags.reads, 1);
        scan
    };
    index.store_scan(library.path(), &scan).unwrap();

    assert_eq!(
        index.album(&selected).unwrap().unwrap().display_title(),
        "强制重读的标签"
    );
    assert_eq!(
        index.album(&other).unwrap().unwrap().display_title(),
        "原标签"
    );
    assert_eq!(
        index.tag_observation(&other.join("01.flac")).unwrap(),
        untouched
    );
}

struct DelayedReader {
    started: std::sync::mpsc::Sender<()>,
    resume: std::sync::mpsc::Receiver<()>,
}

impl TagReader for DelayedReader {
    fn read(&mut self, _: &Path, _: AudioFormat, _: FileStamp) -> Option<TagInfo> {
        None
    }

    fn read_observation(&mut self, _: &Path, _: AudioFormat, _: FileStamp) -> TagObservation {
        self.started.send(()).unwrap();
        self.resume
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        TagObservation::new(TagReadResult::IoFailure("旧任务迟到的读取失败".into()))
    }
}

#[test]
fn a_late_older_read_cannot_replace_a_newer_successful_source_observation() {
    let library = tempfile::tempdir().unwrap();
    let album = library.path().join("专辑");
    std::fs::create_dir(&album).unwrap();
    let audio = album.join("01.flac");
    std::fs::write(&audio, b"unchanged audio").unwrap();
    let storage = tempfile::tempdir().unwrap();
    let database = storage.path().join("index.sqlite3");
    drop(Index::open(&database).unwrap());
    let (started, start_receiver) = std::sync::mpsc::channel();
    let (resume, resume_receiver) = std::sync::mpsc::channel();
    let old_database = database.clone();
    let old_root = library.path().to_path_buf();
    let old_audio = audio.clone();
    let old = std::thread::spawn(move || {
        let index = Index::open(&old_database).unwrap();
        let mut tags = CachedTags::new(
            &index,
            DelayedReader {
                started,
                resume: resume_receiver,
            },
        )
        .with_forced_paths([old_audio]);
        scan_root(&old_root, &mut tags);
    });
    start_receiver
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let mut index = Index::open(&database).unwrap();
    let fresh = {
        let mut tags = CachedTags::new(
            &index,
            Reader(TagReadResult::Tagged(TagInfo {
                album: Some("更新任务成功的标题".into()),
                ..Default::default()
            })),
        )
        .with_forced_paths([audio.clone()]);
        scan_root(library.path(), &mut tags)
    };
    index.store_scan(library.path(), &fresh).unwrap();
    let accepted = index.tag_observation(&audio).unwrap().unwrap();
    resume.send(()).unwrap();
    old.join().unwrap();

    assert_eq!(index.tag_observation(&audio).unwrap().unwrap(), accepted);
    drop(index);
    let reopened = Index::open(&database).unwrap();
    assert_eq!(reopened.tag_observation(&audio).unwrap().unwrap(), accepted);
    assert_eq!(
        reopened.album(&album).unwrap().unwrap().display_title(),
        "更新任务成功的标题"
    );
}

#[test]
fn ordinary_rescan_retries_same_stamp_failure_then_reuses_success_after_reopen() {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let album = library.path().join("专辑");
    std::fs::create_dir(&album).unwrap();
    std::fs::write(album.join("01.flac"), b"fixture").unwrap();
    let database = local.path().join("index.sqlite3");
    let mut index = Index::open(&database).unwrap();
    let scan = {
        let mut reader =
            CachedTags::new(&index, Reader(TagReadResult::IoFailure("临时断连".into())));
        let scan = scan_root(library.path(), &mut reader);
        assert_eq!(reader.reads, 1);
        scan
    };
    index.store_scan(library.path(), &scan).unwrap();
    assert!(index.attention_items().unwrap().iter().any(|item| {
        item.kind == "TagIoFailure"
            && item.explanation.contains("标签读取")
            && item.explanation.contains("临时断连")
            && item.path.ends_with("01.flac")
    }));
    drop(index);

    let mut index = Index::open(&database).unwrap();
    let scan = {
        let mut reader = CachedTags::new(
            &index,
            Reader(TagReadResult::Tagged(TagInfo {
                album: Some("恢复的标题".into()),
                ..Default::default()
            })),
        );
        let scan = scan_root(library.path(), &mut reader);
        assert_eq!(reader.reads, 1, "失败不应成为同 stamp 的永久负缓存");
        scan
    };
    index.store_scan(library.path(), &scan).unwrap();
    assert_eq!(
        index.album(&album).unwrap().unwrap().display_title(),
        "恢复的标题"
    );
    assert!(
        !index
            .attention_items()
            .unwrap()
            .iter()
            .any(|item| item.kind == "TagIoFailure")
    );
    drop(index);
    let index = Index::open(&database).unwrap();
    let mut reader = CachedTags::new(&index, Reader(TagReadResult::IoFailure("不应读取".into())));
    let scan = scan_root(library.path(), &mut reader);
    assert_eq!(reader.reads, 0);
    assert_eq!(
        scan.albums[0].metadata.title.as_ref().unwrap().value,
        "恢复的标题"
    );
}

#[test]
fn failed_observation_keeps_previous_success_and_read_time_until_recovered() {
    let library = tempfile::tempdir().unwrap();
    let album = library.path().join("专辑");
    std::fs::create_dir(&album).unwrap();
    let audio = album.join("01.flac");
    std::fs::write(&audio, b"original").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    let scan = {
        let mut reader = CachedTags::new(
            &index,
            Reader(TagReadResult::Tagged(TagInfo {
                album: Some("上次成功标题".into()),
                year: Some(1999),
                ..Default::default()
            })),
        );
        scan_root(library.path(), &mut reader)
    };
    index.store_scan(library.path(), &scan).unwrap();
    let original = index.tag_observation(&audio).unwrap().unwrap();
    std::fs::write(&audio, b"changed source triggers reread").unwrap();
    let scan = {
        let mut reader =
            CachedTags::new(&index, Reader(TagReadResult::ParseFailure("坏帧".into())));
        scan_root(library.path(), &mut reader)
    };
    index.store_scan(library.path(), &scan).unwrap();
    let failed = index.tag_observation(&audio).unwrap().unwrap();
    assert_eq!(failed.data_observed_at(), Some(original.observed_at));
    assert_eq!(failed.info(), original.info());
    assert!(matches!(failed.result, TagReadResult::ParseFailure(_)));
    assert_eq!(
        index.album(&album).unwrap().unwrap().display_title(),
        "上次成功标题"
    );
    assert!(index.attention_items().unwrap().iter().any(|item| {
        item.kind == "TagParseFailure" && item.explanation.contains("尚未重新确认")
    }));
    let mut reader = CachedTags::new(&index, Reader(TagReadResult::NoTags(TagInfo::default())));
    let scan = scan_root(library.path(), &mut reader);
    assert_eq!(reader.reads, 1);
    index.store_scan(library.path(), &scan).unwrap();
    assert_eq!(
        index.album(&album).unwrap().unwrap().display_title(),
        "专辑"
    );
    assert!(matches!(
        index.tag_observation(&audio).unwrap().unwrap().result,
        TagReadResult::NoTags(_)
    ));
    let mut reader = CachedTags::new(&index, Reader(TagReadResult::IoFailure("不应读取".into())));
    scan_root(library.path(), &mut reader);
    assert_eq!(reader.reads, 0, "成功无标签也可以缓存");
}

#[test]
fn unsupported_is_visible_and_unchanged_ordinary_scans_do_not_repeat_it() {
    let library = tempfile::tempdir().unwrap();
    let album = library.path().join("DSD");
    std::fs::create_dir(&album).unwrap();
    let audio = album.join("image.dff");
    std::fs::write(&audio, b"fixture").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    let scan = {
        let mut reader = CachedTags::new(&index, voon_core::scan::DiskTags);
        let scan = scan_root(library.path(), &mut reader);
        assert_eq!(reader.reads, 1);
        scan
    };
    index.store_scan(library.path(), &scan).unwrap();
    let observed = index.tag_observation(&audio).unwrap().unwrap();
    assert!(matches!(observed.result, TagReadResult::Unsupported(_)));
    assert!(
        index
            .attention_items()
            .unwrap()
            .iter()
            .any(|item| item.kind == "TagUnsupported")
    );
    let mut reader = CachedTags::new(&index, Reader(TagReadResult::IoFailure("不应读取".into())));
    scan_root(library.path(), &mut reader);
    assert_eq!(reader.reads, 0);
    assert_eq!(
        index.tag_observation(&audio).unwrap().unwrap().observed_at,
        observed.observed_at
    );
}
