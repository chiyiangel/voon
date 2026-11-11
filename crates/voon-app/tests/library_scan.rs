use gpui::{AppContext, TestAppContext};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::AtomicBool,
        mpsc::{self, Receiver, Sender},
    },
};
use voon_app::view_model::{
    AppState, ScanState,
    scanning::{ScanUpdate, Scanner},
};
use voon_core::{
    db::Index,
    scan::{NoTags, scan_root},
};

struct ControlledScanner(Mutex<Option<Receiver<ScanUpdate>>>);
impl Scanner for ControlledScanner {
    fn start(&self, _: PathBuf, _: Arc<AtomicBool>) -> Receiver<ScanUpdate> {
        self.0.lock().unwrap().take().unwrap()
    }
}

fn scanner() -> (Box<dyn Scanner>, Sender<ScanUpdate>) {
    let (sender, receiver) = mpsc::channel();
    (
        Box::new(ControlledScanner(Mutex::new(Some(receiver)))),
        sender,
    )
}

#[gpui::test]
fn adding_a_root_streams_albums_and_persists_the_completed_scan(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    std::fs::create_dir(library.path().join("专辑")).unwrap();
    std::fs::write(library.path().join("专辑/01.flac"), b"fixture").unwrap();
    let result = scan_root(library.path(), &mut NoTags);
    let (scanner, sender) = scanner();
    let path = local.path().join("library.sqlite3");
    let state = cx.new(|_| {
        AppState::new(Index::open(&path).unwrap(), library.path().into())
            .unwrap()
            .with_scanner(scanner)
    });
    state.update(cx, |state, cx| {
        state.add_root(library.path().into(), cx).unwrap()
    });
    state.read_with(cx, |state, _| {
        assert!(matches!(
            state.scan_state(),
            ScanState::Scanning { discovered: 0, .. }
        ))
    });
    sender
        .send(ScanUpdate::Album(Box::new(result.albums[0].clone())))
        .unwrap();
    state.update(cx, |state, cx| state.poll_scan(cx));
    state.read_with(cx, |state, _| {
        assert_eq!(state.albums().len(), 1);
        assert!(matches!(
            state.scan_state(),
            ScanState::Scanning { discovered: 1, .. }
        ));
    });
    sender.send(ScanUpdate::Finished(result, 1)).unwrap();
    state.update(cx, |state, cx| state.poll_scan(cx));
    state.read_with(cx, |state, _| {
        assert!(matches!(
            state.scan_state(),
            ScanState::Done { discovered: 1, .. }
        ))
    });
    let reopened = AppState::new(Index::open(&path).unwrap(), library.path().into()).unwrap();
    assert_eq!(reopened.albums().len(), 1);
    assert_eq!(reopened.roots().len(), 1);
}

#[gpui::test]
fn cancelling_keeps_the_albums_already_found(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    std::fs::create_dir(library.path().join("专辑")).unwrap();
    std::fs::write(library.path().join("专辑/01.flac"), b"fixture").unwrap();
    let mut result = scan_root(library.path(), &mut NoTags);
    let (scanner, sender) = scanner();
    let state = cx.new(|_| {
        AppState::new(Index::open_in_memory().unwrap(), library.path().into())
            .unwrap()
            .with_scanner(scanner)
    });
    state.update(cx, |state, cx| {
        state.add_root(library.path().into(), cx).unwrap()
    });
    sender
        .send(ScanUpdate::Album(Box::new(result.albums[0].clone())))
        .unwrap();
    state.update(cx, |state, cx| {
        state.poll_scan(cx);
        state.cancel_scan(cx);
    });
    result.cancelled = true;
    sender.send(ScanUpdate::Finished(result, 1)).unwrap();
    state.update(cx, |state, cx| state.poll_scan(cx));
    state.read_with(cx, |state, _| {
        assert_eq!(state.albums().len(), 1);
        assert!(matches!(
            state.scan_state(),
            ScanState::Cancelled { discovered: 1, .. }
        ));
    });
}

#[gpui::test]
fn a_library_cannot_include_voons_writable_index(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("library.sqlite3");
    let state = cx
        .new(|_| AppState::new(Index::open(&database).unwrap(), directory.path().into()).unwrap());
    state.update(cx, |state, cx| {
        assert!(state.add_root(directory.path().into(), cx).is_err())
    });
    state.read_with(cx, |state, _| assert!(state.roots().is_empty()));
}

#[gpui::test]
fn cancellation_ignores_late_album_events_and_only_persists_completed_discoveries(
    cx: &mut TestAppContext,
) {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let root = library.path().canonicalize().unwrap();
    for name in ["已完成", "取消之后"] {
        std::fs::create_dir(root.join(name)).unwrap();
        std::fs::write(root.join(name).join("01.flac"), name).unwrap();
    }
    let result = scan_root(&root, &mut NoTags);
    let completed = result
        .albums
        .iter()
        .find(|album| album.path == root.join("已完成"))
        .unwrap()
        .clone();
    let late = result
        .albums
        .iter()
        .find(|album| album.path == root.join("取消之后"))
        .unwrap()
        .clone();
    let (scanner, sender) = scanner();
    let database = local.path().join("index.sqlite3");
    let state = cx.new(|_| {
        AppState::new(Index::open(&database).unwrap(), root.clone())
            .unwrap()
            .with_scanner(scanner)
    });
    state.update(cx, |state, cx| state.add_root(root.clone(), cx).unwrap());
    sender.send(ScanUpdate::Album(Box::new(completed))).unwrap();
    state.update(cx, |state, cx| {
        state.poll_scan(cx);
        state.cancel_scan(cx);
    });
    sender.send(ScanUpdate::Album(Box::new(late))).unwrap();
    sender.send(ScanUpdate::Finished(result, 0)).unwrap();
    state.update(cx, |state, cx| state.poll_scan(cx));
    state.read_with(cx, |state, _| {
        assert_eq!(state.albums().len(), 1);
        assert_eq!(state.albums()[0].path, root.join("已完成"));
        assert!(matches!(
            state.scan_state(),
            ScanState::Cancelled { discovered: 1, .. }
        ));
    });
    let index = Index::open(&database).unwrap();
    assert_eq!(index.albums().unwrap().len(), 1);
    assert_eq!(index.albums().unwrap()[0].path, root.join("已完成"));
}

#[gpui::test]
fn a_cancelled_task_can_switch_roots_and_old_events_cannot_replace_new_edits(
    cx: &mut TestAppContext,
) {
    struct QueuedScanner(Mutex<std::collections::VecDeque<Receiver<ScanUpdate>>>);
    impl Scanner for QueuedScanner {
        fn start(&self, _: PathBuf, _: Arc<AtomicBool>) -> Receiver<ScanUpdate> {
            self.0.lock().unwrap().pop_front().unwrap()
        }
    }
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let first = library.path().join("第一根");
    let second = library.path().join("第二根");
    for path in [first.join("已有"), second.join("将移除")] {
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("01.flac"), b"fixture").unwrap();
    }
    let database = local.path().join("index.sqlite3");
    let mut index = Index::open(&database).unwrap();
    index
        .store_scan(&first, &scan_root(&first, &mut NoTags))
        .unwrap();
    index
        .store_scan(&second, &scan_root(&second, &mut NoTags))
        .unwrap();
    std::fs::create_dir_all(first.join("新系列/已完成")).unwrap();
    std::fs::write(first.join("新系列/已完成/02.flac"), b"new").unwrap();
    let first_result = scan_root(&first, &mut NoTags);
    let discovered = first_result
        .albums
        .iter()
        .find(|album| album.path == first.join("新系列/已完成"))
        .unwrap()
        .clone();
    let (old_sender, old_receiver) = mpsc::channel();
    let (new_sender, new_receiver) = mpsc::channel();
    let state = cx.new(|_| {
        AppState::new(index, first.clone())
            .unwrap()
            .with_scanner(Box::new(QueuedScanner(Mutex::new(
                [old_receiver, new_receiver].into(),
            ))))
    });
    state.update(cx, |state, cx| state.start_scan(first.clone(), cx).unwrap());
    old_sender
        .send(ScanUpdate::Album(Box::new(discovered)))
        .unwrap();
    state.update(cx, |state, cx| {
        state.poll_scan(cx);
        state.select_album(Some(first.join("已有")), cx);
        state.begin_album_edit(cx).unwrap();
        state
            .set_album_edit_field("title", "扫描期间的新编辑".into(), cx)
            .unwrap();
        state.save_album_edit(cx).unwrap();
        state.cancel_scan(cx);
        state.start_scan(second.clone(), cx).unwrap();
    });
    assert!(
        old_sender
            .send(ScanUpdate::Finished(first_result, 0))
            .is_err()
    );
    std::fs::remove_dir_all(second.join("将移除")).unwrap();
    std::fs::create_dir(second.join("新增")).unwrap();
    std::fs::write(second.join("新增/03.flac"), b"new second").unwrap();
    new_sender
        .send(ScanUpdate::Finished(scan_root(&second, &mut NoTags), 0))
        .unwrap();
    state.update(cx, |state, cx| state.poll_scan(cx));
    state.read_with(cx, |state, _| {
        assert_eq!(state.albums().len(), 3);
        assert!(
            state
                .albums()
                .iter()
                .any(|album| album.display_title() == "扫描期间的新编辑")
        );
        assert!(
            !state
                .albums()
                .iter()
                .any(|album| album.path == second.join("将移除"))
        );
    });
    let index = Index::open(&database).unwrap();
    assert!(index.collection_tree().unwrap().iter().any(|collection| {
        collection.path == first.join("新系列")
            && collection.albums == [first.join("新系列/已完成")]
    }));
    assert!(index.rename_candidates().unwrap().is_empty());
}

#[gpui::test]
fn an_interrupted_worker_preserves_completed_discoveries_on_reopen(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let root = library.path().canonicalize().unwrap();
    std::fs::create_dir(root.join("已完成")).unwrap();
    std::fs::write(root.join("已完成/01.flac"), b"fixture").unwrap();
    let scan = scan_root(&root, &mut NoTags);
    let database = local.path().join("index.sqlite3");
    let (scanner, sender) = scanner();
    let state = cx.new(|_| {
        AppState::new(Index::open(&database).unwrap(), root.clone())
            .unwrap()
            .with_scanner(scanner)
    });
    state.update(cx, |state, cx| state.add_root(root.clone(), cx).unwrap());
    sender
        .send(ScanUpdate::Album(Box::new(scan.albums[0].clone())))
        .unwrap();
    drop(sender);
    state.update(cx, |state, cx| state.poll_scan(cx));
    assert_eq!(Index::open(&database).unwrap().albums().unwrap().len(), 1);
    state.read_with(cx, |state, _| {
        assert_eq!(state.albums().len(), 1);
        assert!(state.error().is_some());
    });
}
