use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::AtomicBool,
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};

use gpui::{AppContext, Entity, TestAppContext};
use voon_app::view_model::{
    AppState, ScanState,
    scanning::{ScanUpdate, Scanner},
};
use voon_core::{
    db::Index,
    scan::{NoTags, scan_root},
};

fn finish_scan(state: &Entity<AppState>, cx: &mut TestAppContext) -> usize {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        state.update(cx, |state, cx| state.poll_scan(cx));
        if let Some(reads) = state.read_with(cx, |state, _| match state.scan_state() {
            ScanState::Done { reads, .. } => Some(*reads),
            _ => None,
        }) {
            return reads;
        }
        assert!(Instant::now() < deadline, "扫描未完成");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn write_wave(path: &std::path::Path, samples: &[i16]) {
    let length = (samples.len() * 2) as u32;
    let mut bytes = b"RIFF".to_vec();
    bytes.extend((36 + length).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(44_100u32.to_le_bytes());
    bytes.extend(88_200u32.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(length.to_le_bytes());
    for sample in samples {
        bytes.extend(sample.to_le_bytes());
    }
    std::fs::write(path, bytes).unwrap();
}

#[gpui::test]
fn rescanning_reads_only_audio_files_whose_stamp_changed(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let album = library.path().join("Philips Duo/CD1");
    std::fs::create_dir_all(&album).unwrap();
    write_wave(&album.join("01.wav"), &[0, 0]);
    write_wave(&album.join("02.wav"), &[0, 0]);
    let database = local.path().join("library.sqlite3");
    let root = library.path().canonicalize().unwrap();
    let state =
        cx.new(|_| AppState::new(Index::open(&database).unwrap(), library.path().into()).unwrap());

    state.update(cx, |state, cx| {
        state.add_root(library.path().into(), cx).unwrap()
    });
    assert_eq!(finish_scan(&state, cx), 2);

    state.update(cx, |state, cx| state.start_scan(root.clone(), cx).unwrap());
    assert_eq!(finish_scan(&state, cx), 0);

    write_wave(&album.join("02.wav"), &[1, 2, 3, 4]);
    state.update(cx, |state, cx| state.start_scan(root, cx).unwrap());
    assert_eq!(finish_scan(&state, cx), 1);
}

struct ControlledScanner(Mutex<Option<Receiver<ScanUpdate>>>);
impl Scanner for ControlledScanner {
    fn start(&self, _: PathBuf, _: Arc<AtomicBool>) -> Receiver<ScanUpdate> {
        self.0.lock().unwrap().take().unwrap()
    }
}

#[gpui::test]
fn cancelling_a_rescan_keeps_unseen_albums_and_does_not_propose_renames(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let root = library.path().canonicalize().unwrap();
    for (name, contents) in [("旧专辑", "fixture"), ("未扫描专辑", "different fixture")] {
        std::fs::create_dir(root.join(name)).unwrap();
        std::fs::write(root.join(name).join("01.flac"), contents).unwrap();
    }
    let database = local.path().join("library.sqlite3");
    let mut index = Index::open(&database).unwrap();
    index
        .store_scan(&root, &scan_root(&root, &mut NoTags))
        .unwrap();
    let old = root.join("旧专辑");
    index.set_user_edit(&old, "title", "我的标题").unwrap();
    std::fs::rename(&old, root.join("新路径")).unwrap();
    let mut partial = scan_root(&root, &mut NoTags);
    partial
        .albums
        .retain(|album| album.path == root.join("新路径"));
    partial.cancelled = true;
    let (sender, receiver) = mpsc::channel();
    let state = cx.new(|_| {
        AppState::new(index, root.clone())
            .unwrap()
            .with_scanner(Box::new(ControlledScanner(Mutex::new(Some(receiver)))))
    });

    state.update(cx, |state, cx| state.start_scan(root.clone(), cx).unwrap());
    sender
        .send(ScanUpdate::Album(Box::new(partial.albums[0].clone())))
        .unwrap();
    state.update(cx, |state, cx| {
        state.poll_scan(cx);
        state.cancel_scan(cx);
    });
    sender.send(ScanUpdate::Finished(partial, 0)).unwrap();
    state.update(cx, |state, cx| state.poll_scan(cx));

    state.read_with(cx, |state, _| {
        assert!(matches!(
            state.scan_state(),
            ScanState::Cancelled { discovered: 1, .. }
        ));
        assert_eq!(state.albums().len(), 3);
        assert!(
            state
                .albums()
                .iter()
                .any(|album| album.path == old && album.display_title() == "我的标题")
        );
        assert!(
            state
                .albums()
                .iter()
                .any(|album| album.path == root.join("未扫描专辑"))
        );
    });
    let index = Index::open(&database).unwrap();
    assert_eq!(index.albums().unwrap().len(), 3);
    assert!(index.rename_candidates().unwrap().is_empty());
}

#[gpui::test]
fn an_unmounted_root_keeps_local_albums_and_recovers_without_rescanning(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let root = library.path().join("NAS/Music");
    std::fs::create_dir_all(root.join("夜曲")).unwrap();
    std::fs::write(root.join("夜曲/01.flac"), b"fixture").unwrap();
    let database = local.path().join("library.sqlite3");
    let mut index = Index::open(&database).unwrap();
    index
        .store_scan(&root, &scan_root(&root, &mut NoTags))
        .unwrap();
    let album = root.join("夜曲");
    let state = cx.new(|_| AppState::new(index, root.clone()).unwrap());
    std::fs::rename(
        library.path().join("NAS"),
        library.path().join("NAS-offline"),
    )
    .unwrap();

    let wait_for_availability = |missing, cx: &mut TestAppContext| {
        let deadline = Instant::now() + Duration::from_secs(7);
        loop {
            state.update(cx, |state, cx| state.poll_scan(cx));
            let is_missing = state.read_with(cx, |state, _| {
                matches!(state.scan_state(), ScanState::Missing { .. })
            });
            if is_missing == missing {
                break;
            }
            assert!(Instant::now() < deadline, "音乐库状态未更新");
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    wait_for_availability(true, cx);
    Index::open(&database)
        .unwrap()
        .set_user_edit(&album, "title", "离线编辑")
        .unwrap();
    state.update(cx, |state, cx| {
        state.reload_library(cx).unwrap();
        state.set_search("离线".into(), cx);
        state.select_album(Some(album.clone()), cx);
    });
    state.read_with(cx, |state, _| {
        assert_eq!(state.albums().len(), 1);
        assert_eq!(state.selected_album().unwrap().display_title(), "离线编辑");
        assert_eq!(state.search(), "离线");
    });

    std::fs::rename(
        library.path().join("NAS-offline"),
        library.path().join("NAS"),
    )
    .unwrap();
    wait_for_availability(false, cx);
    state.read_with(cx, |state, _| {
        assert_eq!(state.scan_state(), &ScanState::Idle);
        assert_eq!(state.albums().len(), 1);
        assert_eq!(state.selected_album().unwrap().display_title(), "离线编辑");
    });
}

#[gpui::test]
fn losing_a_root_during_rescan_keeps_the_last_index(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let root = library.path().canonicalize().unwrap();
    std::fs::create_dir(root.join("夜曲")).unwrap();
    std::fs::write(root.join("夜曲/01.flac"), b"fixture").unwrap();
    let database = local.path().join("library.sqlite3");
    let mut index = Index::open(&database).unwrap();
    index
        .store_scan(&root, &scan_root(&root, &mut NoTags))
        .unwrap();
    let (sender, receiver) = mpsc::channel();
    let state = cx.new(|_| {
        AppState::new(index, root.clone())
            .unwrap()
            .with_scanner(Box::new(ControlledScanner(Mutex::new(Some(receiver)))))
    });
    state.update(cx, |state, cx| state.start_scan(root.clone(), cx).unwrap());

    sender
        .send(ScanUpdate::Unavailable(voon_core::ScanResult::default()))
        .unwrap();
    state.update(cx, |state, cx| state.poll_scan(cx));

    state.read_with(cx, |state, _| {
        assert!(matches!(state.scan_state(), ScanState::Missing { .. }));
        assert_eq!(state.offline_roots(), &[root]);
        assert_eq!(state.albums().len(), 1);
    });
    let index = Index::open(&database).unwrap();
    assert_eq!(index.albums().unwrap().len(), 1);
    assert!(index.rename_candidates().unwrap().is_empty());
}

#[gpui::test]
fn an_empty_mountpoint_keeps_the_previous_index_until_the_user_confirms(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let root = library.path().join("mountpoint");
    std::fs::create_dir_all(root.join("专辑")).unwrap();
    std::fs::write(root.join("专辑/01.flac"), b"fixture").unwrap();
    let database = local.path().join("library.sqlite3");
    let mut index = Index::open(&database).unwrap();
    index
        .store_scan(&root, &scan_root(&root, &mut NoTags))
        .unwrap();
    index
        .set_user_edit(&root.join("专辑"), "title", "保留的编辑")
        .unwrap();
    // An unmounted volume can leave its empty mountpoint directory behind.
    std::fs::rename(root.join("专辑"), library.path().join("unmounted-album")).unwrap();
    let state = cx.new(|_| AppState::new(index, root.clone()).unwrap());
    state.update(cx, |state, cx| state.start_scan(root.clone(), cx).unwrap());
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        state.update(cx, |state, cx| state.poll_scan(cx));
        if !state.read_with(cx, |state, _| {
            matches!(state.scan_state(), ScanState::Scanning { .. })
        }) {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    state.read_with(cx, |state, _| {
        assert_eq!(state.albums().len(), 1);
        assert_eq!(state.albums()[0].display_title(), "保留的编辑");
        assert!(matches!(state.scan_state(), ScanState::Empty { .. }));
    });
    assert!(
        Index::open(&database)
            .unwrap()
            .has_scanned_albums(&root)
            .unwrap()
    );
    state.update(cx, |state, cx| state.confirm_empty_scan(cx).unwrap());
    assert!(state.read_with(cx, |state, _| state.albums().is_empty()));
    assert!(
        !Index::open(&database)
            .unwrap()
            .has_scanned_albums(&root)
            .unwrap()
    );
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
}

#[gpui::test]
fn an_incomplete_album_remains_browsable_with_a_persisted_stale_explanation(
    cx: &mut TestAppContext,
) {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let root = library.path().canonicalize().unwrap();
    for file in [
        "套装/甲/CD1/01.flac",
        "套装/甲/CD2/01.flac",
        "套装/乙/01.flac",
    ] {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"fixture").unwrap();
    }
    let album = root.join("套装/甲");
    let database = local.path().join("index.sqlite3");
    let mut index = Index::open(&database).unwrap();
    index
        .store_scan(&root, &scan_root(&root, &mut NoTags))
        .unwrap();
    index
        .set_user_edit(&album, "title", "已经整理的标题")
        .unwrap();
    let mut partial = scan_root(&root, &mut NoTags);
    partial
        .albums
        .iter_mut()
        .find(|found| found.path == album)
        .unwrap()
        .discs
        .pop();
    partial.issues.push(voon_core::scan::ScanIssue {
        path: album.join("CD2"),
        kind: voon_core::scan::IssueKind::Unreadable {
            error: "权限不足".into(),
        },
    });
    let (sender, receiver) = mpsc::channel();
    let state = cx.new(|_| {
        AppState::new(index, root.clone())
            .unwrap()
            .with_scanner(Box::new(ControlledScanner(Mutex::new(Some(receiver)))))
    });
    state.update(cx, |state, cx| state.start_scan(root.clone(), cx).unwrap());
    sender.send(ScanUpdate::Finished(partial, 0)).unwrap();
    state.update(cx, |state, cx| {
        state.poll_scan(cx);
        state.select_album(Some(album.clone()), cx);
        state.set_search("已经整理".into(), cx);
    });
    state.read_with(cx, |state, _| {
        assert_eq!(state.album_detail().unwrap().tracks().count(), 2);
        let issue = state
            .attention_items()
            .iter()
            .find(|item| item.path == album.join("CD2"))
            .unwrap();
        assert_eq!(issue.title, "本次读取不完整");
        assert!(issue.explanation.contains("保留上次完整资料"));
        assert!(issue.explanation.contains("目录列表"));
        assert!(issue.explanation.contains("权限不足"));
        assert_eq!(state.albums().len(), 2);
    });
    let reopened = Index::open(&database).unwrap();
    assert_eq!(reopened.album(&album).unwrap().unwrap().tracks().count(), 2);
    assert!(reopened.attention_items().unwrap().iter().any(|item| {
        item.title == "本次读取不完整" && item.album_path.as_ref() == Some(&album)
    }));
}

#[gpui::test]
fn retrying_a_first_incomplete_location_publishes_the_recovered_album(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let root = library.path().canonicalize().unwrap();
    let album = root.join("首次发现");
    std::fs::create_dir(&album).unwrap();
    std::fs::write(album.join("image.flac"), b"fixture").unwrap();
    let cue = album.join("image.cue");
    std::fs::write(&cue, "FILE \"image.flac\" WAVE\n").unwrap();
    let mut index = Index::open(&local.path().join("index.sqlite3")).unwrap();
    index
        .store_scan(&root, &scan_root(&root, &mut NoTags))
        .unwrap();
    let state = cx.new(|_| AppState::new(index, root.clone()).unwrap());
    assert!(state.read_with(cx, |state, _| state.albums().is_empty()));
    std::fs::write(&cue, "FILE \"image.flac\" WAVE\n TRACK 01 AUDIO\n INDEX 01 00:00:00\n TRACK 02 AUDIO\n INDEX 01 03:00:00\n").unwrap();
    state.update(cx, |state, cx| {
        state
            .retry_attention(&album, "CueWithoutTracks", cx)
            .unwrap()
    });
    finish_scan(&state, cx);
    state.update(cx, |state, cx| state.select_album(Some(album.clone()), cx));
    state.read_with(cx, |state, _| {
        assert_eq!(state.albums().len(), 1);
        assert_eq!(state.album_detail().unwrap().tracks().count(), 2);
        assert!(
            !state
                .attention_items()
                .iter()
                .any(|item| item.kind == "CueWithoutTracks")
        );
    });
}

#[gpui::test]
fn failed_album_observation_is_saved_as_attention_instead_of_an_empty_root_confirmation(
    cx: &mut TestAppContext,
) {
    let library = tempfile::tempdir().unwrap();
    let root = library.path().canonicalize().unwrap();
    let album = root.join("甲");
    std::fs::create_dir(&album).unwrap();
    std::fs::write(album.join("01.flac"), b"fixture").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(&root, &scan_root(&root, &mut NoTags))
        .unwrap();
    let (sender, receiver) = mpsc::channel();
    let state = cx.new(|_| {
        AppState::new(index, root.clone())
            .unwrap()
            .with_scanner(Box::new(ControlledScanner(Mutex::new(Some(receiver)))))
    });
    state.update(cx, |state, cx| state.start_scan(root.clone(), cx).unwrap());
    let mut failed = voon_core::ScanResult::default();
    failed.issues.push(voon_core::scan::ScanIssue {
        path: album.clone(),
        kind: voon_core::scan::IssueKind::Unreadable {
            error: "网络读取失败".into(),
        },
    });
    sender.send(ScanUpdate::Finished(failed, 0)).unwrap();
    state.update(cx, |state, cx| state.poll_scan(cx));
    state.read_with(cx, |state, _| {
        assert!(!matches!(state.scan_state(), ScanState::Empty { .. }));
        assert_eq!(state.albums().len(), 1);
        assert!(
            state
                .album_scan_issues(&album)
                .iter()
                .any(|item| item.retains_previous)
        );
    });
}

#[gpui::test]
fn an_empty_root_after_a_cancelled_first_scan_still_requires_confirmation(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let root = library.path().canonicalize().unwrap();
    let album = root.join("已完成");
    std::fs::create_dir(&album).unwrap();
    std::fs::write(album.join("01.flac"), b"fixture").unwrap();
    let database = local.path().join("index.sqlite3");
    let mut index = Index::open(&database).unwrap();
    let mut partial = scan_root(&root, &mut NoTags);
    partial.cancelled = true;
    index.store_partial_scan(&root, &partial).unwrap();
    std::fs::remove_dir_all(&album).unwrap();
    let (sender, receiver) = mpsc::channel();
    let state = cx.new(|_| {
        AppState::new(index, root.clone())
            .unwrap()
            .with_scanner(Box::new(ControlledScanner(Mutex::new(Some(receiver)))))
    });
    state.update(cx, |state, cx| state.start_scan(root.clone(), cx).unwrap());
    sender
        .send(ScanUpdate::Finished(scan_root(&root, &mut NoTags), 0))
        .unwrap();
    state.update(cx, |state, cx| state.poll_scan(cx));
    state.read_with(cx, |state, _| {
        assert_eq!(state.albums().len(), 1);
        assert!(matches!(state.scan_state(), ScanState::Empty { .. }));
    });
    assert_eq!(Index::open(&database).unwrap().albums().unwrap().len(), 1);
    state.update(cx, |state, cx| state.confirm_empty_scan(cx).unwrap());
    assert!(Index::open(&database).unwrap().albums().unwrap().is_empty());
}
