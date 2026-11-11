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

#[gpui::test]
fn rescanning_reads_only_audio_files_whose_stamp_changed(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let album = library.path().join("Philips Duo/CD1");
    std::fs::create_dir_all(&album).unwrap();
    std::fs::write(album.join("01.flac"), b"first fixture").unwrap();
    std::fs::write(album.join("02.flac"), b"second fixture").unwrap();
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

    std::fs::write(album.join("02.flac"), b"changed audio fixture").unwrap();
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
