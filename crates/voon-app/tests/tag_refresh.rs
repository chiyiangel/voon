use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use gpui::{AppContext, Entity, TestAppContext};
use voon_app::view_model::{AppState, ScanState};
use voon_core::{
    AudioFormat,
    db::{CachedTags, Index},
    scan::{FileStamp, TagReader, scan_root},
    tags::{TagInfo, TagObservation, TagReadResult},
};

fn write_wave(path: &Path) {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend(40u32.to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(44_100u32.to_le_bytes());
    bytes.extend(88_200u32.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(4u32.to_le_bytes());
    bytes.extend([0; 4]);
    fs::write(path, bytes).unwrap();
}

struct SeedTags {
    failed: Vec<PathBuf>,
}

struct LooseSeedTags;
impl TagReader for LooseSeedTags {
    fn read(&mut self, path: &Path, _: AudioFormat, _: FileStamp) -> Option<TagInfo> {
        Some(TagInfo {
            album: Some(path.file_stem().unwrap().to_string_lossy().into_owned()),
            ..Default::default()
        })
    }
}
impl TagReader for SeedTags {
    fn read(&mut self, _: &Path, _: AudioFormat, _: FileStamp) -> Option<TagInfo> {
        None
    }
    fn read_observation(&mut self, path: &Path, _: AudioFormat, _: FileStamp) -> TagObservation {
        TagObservation::new(
            if self.failed.iter().any(|failed| path.starts_with(failed)) {
                TagReadResult::IoFailure("测试中的临时断连".into())
            } else {
                TagReadResult::Tagged(TagInfo {
                    album: Some("上次成功的标签标题".into()),
                    ..Default::default()
                })
            },
        )
    }
}

fn finish(state: &Entity<AppState>, cx: &mut TestAppContext) -> usize {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        state.update(cx, |state, cx| state.poll_scan(cx));
        if let Some(reads) = state.read_with(cx, |state, _| {
            assert!(state.error().is_none(), "{:?}", state.error());
            match state.scan_state() {
                ScanState::Done { reads, .. } => Some(*reads),
                _ => None,
            }
        }) {
            return reads;
        }
        assert!(Instant::now() < deadline, "标签处理未完成");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[gpui::test]
fn forcing_one_album_reads_only_its_audio_and_preserves_unrelated_success_and_failure(
    cx: &mut TestAppContext,
) {
    let library = tempfile::tempdir().unwrap();
    let root = library.path().canonicalize().unwrap();
    let selected = root.join("甲");
    let failed = root.join("乙");
    let untouched = root.join("丙");
    for album in [&selected, &failed, &untouched] {
        fs::create_dir(album).unwrap();
        write_wave(&album.join("01.wav"));
    }
    let local = tempfile::tempdir().unwrap();
    let database = local.path().join("index.sqlite3");
    let mut index = Index::open(&database).unwrap();
    let scan = {
        let mut tags = CachedTags::new(
            &index,
            SeedTags {
                failed: vec![failed.clone()],
            },
        );
        scan_root(&root, &mut tags)
    };
    index.store_scan(&root, &scan).unwrap();
    let old_failure = index.tag_observation(&failed.join("01.wav")).unwrap();
    let old_success = index.tag_observation(&untouched.join("01.wav")).unwrap();
    let source_before = fs::read(selected.join("01.wav")).unwrap();
    let stamp_before = FileStamp::of(&selected.join("01.wav"));
    let state = cx.new(|_| AppState::new(index, root.clone()).unwrap());
    state.update(cx, |state, cx| {
        state.select_album(Some(selected.clone()), cx);
        state.force_selected_album_tags(cx).unwrap();
    });
    assert_eq!(finish(&state, cx), 1);

    state.read_with(cx, |state, _| {
        assert_eq!(state.album_detail().unwrap().display_title(), "甲");
        assert_eq!(state.albums().len(), 3);
    });
    let reopened = Index::open(&database).unwrap();
    assert_eq!(
        reopened.tag_observation(&failed.join("01.wav")).unwrap(),
        old_failure
    );
    assert_eq!(
        reopened.tag_observation(&untouched.join("01.wav")).unwrap(),
        old_success
    );
    assert_eq!(
        reopened.album(&untouched).unwrap().unwrap().display_title(),
        "上次成功的标签标题"
    );
    assert_eq!(fs::read(selected.join("01.wav")).unwrap(), source_before);
    assert_eq!(FileStamp::of(&selected.join("01.wav")), stamp_before);
}

#[gpui::test]
fn retrying_one_tag_failure_reads_only_that_source_and_clears_only_its_issue(
    cx: &mut TestAppContext,
) {
    let library = tempfile::tempdir().unwrap();
    let root = library.path().canonicalize().unwrap();
    let album = root.join("两次失败");
    let other = root.join("其他成功");
    for directory in [&album, &other] {
        fs::create_dir(directory).unwrap();
    }
    for audio in [
        album.join("01.wav"),
        album.join("02.wav"),
        other.join("01.wav"),
    ] {
        write_wave(&audio);
    }
    let local = tempfile::tempdir().unwrap();
    let database = local.path().join("index.sqlite3");
    let mut index = Index::open(&database).unwrap();
    let scan = {
        let mut tags = CachedTags::new(
            &index,
            SeedTags {
                failed: vec![album.clone()],
            },
        );
        scan_root(&root, &mut tags)
    };
    index.store_scan(&root, &scan).unwrap();
    let untouched = index.tag_observation(&album.join("02.wav")).unwrap();
    let success = index.tag_observation(&other.join("01.wav")).unwrap();
    let state = cx.new(|_| AppState::new(index, root.clone()).unwrap());

    state.update(cx, |state, cx| {
        state
            .retry_attention(&album.join("01.wav"), "TagIoFailure", cx)
            .unwrap()
    });
    assert_eq!(finish(&state, cx), 1);

    state.read_with(cx, |state, _| {
        assert_eq!(state.scan_operation().label(), "重试失败项");
        assert!(
            !state
                .attention_items()
                .iter()
                .any(|item| item.path == album.join("01.wav") && item.kind == "TagIoFailure")
        );
        assert!(
            state
                .attention_items()
                .iter()
                .any(|item| item.path == album.join("02.wav") && item.kind == "TagIoFailure")
        );
        assert_eq!(state.albums().len(), 2);
    });
    let reopened = Index::open(&database).unwrap();
    assert_eq!(
        reopened.tag_observation(&album.join("02.wav")).unwrap(),
        untouched
    );
    assert_eq!(
        reopened.tag_observation(&other.join("01.wav")).unwrap(),
        success
    );
    assert!(matches!(
        reopened
            .tag_observation(&album.join("01.wav"))
            .unwrap()
            .unwrap()
            .result,
        TagReadResult::NoTags(_)
    ));
}

#[gpui::test]
fn retrying_a_recovered_directory_confirms_its_removed_album_without_touching_other_failures(
    cx: &mut TestAppContext,
) {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let root = library.path().canonicalize().unwrap();
    let scope = root.join("失败的系列");
    let removed = scope.join("已删除");
    let other = root.join("其他系列/保留");
    for path in [&removed, &other] {
        fs::create_dir_all(path).unwrap();
        write_wave(&path.join("01.wav"));
    }
    let database = local.path().join("index.sqlite3");
    let mut index = Index::open(&database).unwrap();
    let original = {
        let mut tags = CachedTags::new(
            &index,
            SeedTags {
                failed: vec![other.clone()],
            },
        );
        scan_root(&root, &mut tags)
    };
    index.store_scan(&root, &original).unwrap();
    index
        .set_user_edit(&removed, "title", "保留的本地整理")
        .unwrap();
    let untouched = index.tag_observation(&other.join("01.wav")).unwrap();
    let mut failed = voon_core::ScanResult::default();
    failed.issues.push(voon_core::scan::ScanIssue {
        path: scope.clone(),
        kind: voon_core::scan::IssueKind::Unreadable {
            error: "临时断连".into(),
        },
    });
    index.store_partial_scan(&root, &failed).unwrap();
    fs::remove_dir_all(&removed).unwrap();
    let state = cx.new(|_| AppState::new(index, root.clone()).unwrap());
    state.update(cx, |state, cx| {
        state.retry_attention(&scope, "Unreadable", cx).unwrap()
    });
    assert_eq!(finish(&state, cx), 0);
    state.read_with(cx, |state, _| {
        assert!(!state.albums().iter().any(|album| album.path == removed));
        assert_eq!(state.albums().len(), 1);
        assert!(
            !state
                .attention_items()
                .iter()
                .any(|item| item.path == scope)
        );
    });
    let reopened = Index::open(&database).unwrap();
    assert!(reopened.album(&removed).unwrap().is_none());
    assert_eq!(
        reopened.tag_observation(&other.join("01.wav")).unwrap(),
        untouched
    );
    assert!(reopened.rename_candidates().unwrap().is_empty());
}

#[gpui::test]
fn forcing_a_merged_album_rereads_all_its_sources_across_library_roots(cx: &mut TestAppContext) {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let first_root = first.path().canonicalize().unwrap();
    let second_root = second.path().canonicalize().unwrap();
    let first_album = first_root.join("甲");
    let second_album = second_root.join("乙");
    for album in [&first_album, &second_album] {
        fs::create_dir(album).unwrap();
        write_wave(&album.join("01.wav"));
    }
    let local = tempfile::tempdir().unwrap();
    let database = local.path().join("index.sqlite3");
    let mut index = Index::open(&database).unwrap();
    for root in [&first_root, &second_root] {
        let scan = {
            let mut tags = CachedTags::new(&index, SeedTags { failed: vec![] });
            scan_root(root, &mut tags)
        };
        index.store_scan(root, &scan).unwrap();
    }
    let merged = index
        .merge_albums(
            &[first_album.clone(), second_album.clone()],
            &first_album,
            &[],
        )
        .unwrap();
    let state = cx.new(|_| AppState::new(index, first_root).unwrap());
    state.update(cx, |state, cx| {
        state.select_album(Some(merged.clone()), cx);
        state.force_selected_album_tags(cx).unwrap();
    });
    assert_eq!(finish(&state, cx), 2);

    state.read_with(cx, |state, _| {
        assert_eq!(state.album_detail().unwrap().path, merged);
        assert_eq!(state.album_detail().unwrap().tracks().count(), 2);
        assert_eq!(state.albums().len(), 1);
    });
    let index = Index::open(&database).unwrap();
    for album in [&first_album, &second_album] {
        assert!(matches!(
            index
                .tag_observation(&album.join("01.wav"))
                .unwrap()
                .unwrap()
                .result,
            TagReadResult::NoTags(_)
        ));
    }
    assert!(index.last_structure_operation().unwrap().is_some());
}

#[gpui::test]
fn forcing_a_loose_album_with_no_tags_removes_the_old_group_and_shows_attention(
    cx: &mut TestAppContext,
) {
    let library = tempfile::tempdir().unwrap();
    let root = library.path().canonicalize().unwrap();
    let audio = root.join("01.wav");
    write_wave(&audio);
    let other = root.join("02.wav");
    write_wave(&other);
    let local = tempfile::tempdir().unwrap();
    let database = local.path().join("index.sqlite3");
    let mut index = Index::open(&database).unwrap();
    let scan = {
        let mut tags = CachedTags::new(&index, LooseSeedTags);
        scan_root(&root, &mut tags)
    };
    index.store_scan(&root, &scan).unwrap();
    let old_group = index
        .albums()
        .unwrap()
        .into_iter()
        .find(|album| album.display_title() == "01")
        .unwrap()
        .path;
    let other_observation = index.tag_observation(&other).unwrap();
    write_wave(&root.join("03.wav"));
    index
        .set_user_edit(&old_group, "genre", "保留的整理")
        .unwrap();
    let state = cx.new(|_| AppState::new(index, root.clone()).unwrap());
    state.update(cx, |state, cx| {
        state.select_album(Some(old_group.clone()), cx);
        assert!(
            state
                .album_tag_refresh_notice()
                .unwrap()
                .contains("旧分组可能消失")
        );
        state.force_selected_album_tags(cx).unwrap();
    });
    assert_eq!(finish(&state, cx), 1);

    state.read_with(cx, |state, _| {
        assert!(
            state.albums().len() == 1 && state.albums()[0].display_title() == "02",
            "旧的标签合成分组不能继续冒充当前资料"
        );
        assert!(
            !state
                .attention_items()
                .iter()
                .any(|item| item.kind == "LooseUntagged" && item.explanation.contains("03.wav"))
        );
        assert!(
            state
                .attention_items()
                .iter()
                .any(|item| item.kind == "LooseUntagged" && item.explanation.contains("01.wav"))
        );
    });
    let mut index = Index::open(&database).unwrap();
    assert!(index.album(&old_group).unwrap().is_none());
    assert_eq!(index.tag_observation(&other).unwrap(), other_observation);
    assert!(
        index
            .tag_observation(&root.join("03.wav"))
            .unwrap()
            .is_none()
    );
    let scan = {
        let mut tags = CachedTags::new(&index, LooseSeedTags).with_forced_paths([audio]);
        scan_root(&root, &mut tags)
    };
    index.store_scan(&root, &scan).unwrap();
    let album = index.album(&old_group).unwrap().unwrap();
    assert_eq!(album.metadata.genre.unwrap().value, "保留的整理");
}

struct ControlledScans(
    std::sync::Mutex<
        std::collections::VecDeque<
            std::sync::mpsc::Receiver<voon_app::view_model::scanning::ScanUpdate>,
        >,
    >,
);
impl voon_app::view_model::scanning::Scanner for ControlledScans {
    fn start(
        &self,
        _: PathBuf,
        _: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> std::sync::mpsc::Receiver<voon_app::view_model::scanning::ScanUpdate> {
        self.0.lock().unwrap().pop_front().unwrap()
    }
}

#[gpui::test]
fn cancelling_a_multi_root_force_stops_the_round_and_keeps_the_accepted_album(
    cx: &mut TestAppContext,
) {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let first_root = first.path().canonicalize().unwrap();
    let second_root = second.path().canonicalize().unwrap();
    let first_album = first_root.join("甲");
    let second_album = second_root.join("乙");
    for album in [&first_album, &second_album] {
        fs::create_dir(album).unwrap();
        write_wave(&album.join("01.wav"));
    }
    let mut index = Index::open_in_memory().unwrap();
    for root in [&first_root, &second_root] {
        let scan = {
            let mut tags = CachedTags::new(&index, SeedTags { failed: vec![] });
            scan_root(root, &mut tags)
        };
        index.store_scan(root, &scan).unwrap();
    }
    let merged = index
        .merge_albums(&[first_album.clone(), second_album], &first_album, &[])
        .unwrap();
    index
        .set_user_edit(&merged, "title", "已接受的人工整理")
        .unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let (_later_sender, later_receiver) = std::sync::mpsc::channel();
    let state = cx.new(|_| {
        AppState::new(index, first_root.clone())
            .unwrap()
            .with_scanner(Box::new(ControlledScans(std::sync::Mutex::new(
                std::collections::VecDeque::from([receiver, later_receiver]),
            ))))
    });
    state.update(cx, |state, cx| {
        state.select_album(Some(merged.clone()), cx);
        state.force_selected_album_tags(cx).unwrap();
        state.cancel_scan(cx);
    });
    sender
        .send(voon_app::view_model::scanning::ScanUpdate::Finished(
            scan_root(&first_root, &mut voon_core::scan::NoTags),
            1,
        ))
        .unwrap();
    state.update(cx, |state, cx| state.poll_scan(cx));

    state.read_with(cx, |state, _| {
        assert!(matches!(state.scan_state(), ScanState::Cancelled { .. }));
        assert_eq!(
            state.album_detail().unwrap().display_title(),
            "已接受的人工整理"
        );
        assert_eq!(state.album_detail().unwrap().tracks().count(), 2);
        assert_eq!(state.albums().len(), 1);
    });
}
