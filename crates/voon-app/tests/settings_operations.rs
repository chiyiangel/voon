use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::AtomicBool,
        mpsc::{self, Receiver},
    },
};

use gpui::{AppContext, TestAppContext};
use voon_app::view_model::{
    AppState, ScanState,
    scanning::{ScanUpdate, Scanner},
};
use voon_core::db::Index;
use voon_core::scan::{NoTags, scan_root};

struct PendingScanner(Mutex<Option<Receiver<ScanUpdate>>>);

impl Scanner for PendingScanner {
    fn start(&self, _: PathBuf, _: Arc<AtomicBool>) -> Receiver<ScanUpdate> {
        self.0.lock().unwrap().take().unwrap()
    }
}

#[gpui::test]
fn a_removal_confirmed_after_another_window_started_scanning_keeps_both_roots(
    cx: &mut TestAppContext,
) {
    let music = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let root = music.path().canonicalize().unwrap();
    let other_root = other.path().canonicalize().unwrap();
    let index = Index::open_in_memory().unwrap();
    index.add_root(&root).unwrap();
    index.add_root(&other_root).unwrap();
    let (_sender, receiver) = mpsc::channel();
    let state = cx.new(|_| {
        AppState::new(index, root.clone())
            .unwrap()
            .with_scanner(Box::new(PendingScanner(Mutex::new(Some(receiver)))))
    });

    state.update(cx, |state, cx| {
        state.request_root_removal(Some(other_root.clone()), cx);
        state.start_scan(root.clone(), cx).unwrap();
        let error = state
            .confirm_root_removal(cx)
            .expect_err("扫描中不能移除目录");
        assert!(error.to_string().contains("扫描"));
        assert_eq!(state.roots().len(), 2);
        assert!(matches!(state.scan_state(), ScanState::Scanning { .. }));
        assert_eq!(state.pending_root_removal(), Some(other_root.as_path()));
    });
}

#[gpui::test]
fn creating_an_index_preserves_an_album_draft_until_the_user_saves_or_cancels_it(
    cx: &mut TestAppContext,
) {
    let music = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let root = music.path().canonicalize().unwrap();
    let album = root.join("Album");
    std::fs::create_dir(&album).unwrap();
    std::fs::write(album.join("01.dff"), b"fixture").unwrap();
    let old_path = local.path().canonicalize().unwrap().join("old.sqlite3");
    let new_path = old_path.with_file_name("new.sqlite3");
    let mut index = Index::open(&old_path).unwrap();
    index
        .store_scan(&root, &scan_root(&root, &mut NoTags))
        .unwrap();
    let state = cx.new(|_| AppState::new(index, root).unwrap());

    state.update(cx, |state, cx| {
        state.select_album(Some(album), cx);
        state.begin_album_edit(cx).unwrap();
        state
            .set_album_edit_field("title", "未提交标题".into(), cx)
            .unwrap();
        let error = state
            .create_new_index(new_path.clone(), cx)
            .expect_err("不能丢弃编辑");
        assert!(error.to_string().contains("保存或取消"));
        assert_eq!(state.index_path(), Some(old_path.as_path()));
        assert_eq!(
            state
                .album_edit_draft()
                .unwrap()
                .fields()
                .iter()
                .find(|f| f.name() == "title")
                .unwrap()
                .value(),
            "未提交标题"
        );
        assert!(!new_path.exists());
        state.save_album_edit(cx).unwrap();
        state.create_new_index(new_path.clone(), cx).unwrap();
        assert_eq!(state.index_path(), Some(new_path.as_path()));
    });
}

#[gpui::test]
fn creating_an_index_preserves_a_cover_draft_until_the_user_cancels_it(cx: &mut TestAppContext) {
    let music = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let root = music.path().canonicalize().unwrap();
    let album = root.join("Album");
    std::fs::create_dir(&album).unwrap();
    std::fs::write(album.join("01.dff"), b"fixture").unwrap();
    let old_path = local.path().canonicalize().unwrap().join("old.sqlite3");
    let new_path = old_path.with_file_name("new.sqlite3");
    let mut index = Index::open(&old_path).unwrap();
    index
        .store_scan(&root, &scan_root(&root, &mut NoTags))
        .unwrap();
    let state = cx.new(|_| AppState::new(index, root).unwrap());

    state.update(cx, |state, cx| {
        state.select_album(Some(album.clone()), cx);
        state.begin_cover_picker(cx).unwrap();
        let error = state
            .create_new_index(new_path.clone(), cx)
            .expect_err("不能丢弃封面选择");
        assert!(error.to_string().contains("保存或取消"));
        assert_eq!(state.index_path(), Some(old_path.as_path()));
        assert_eq!(state.cover_picker().unwrap().album_path, album);
        assert!(!new_path.exists());
        state.cancel_cover_picker(cx);
        state.create_new_index(new_path.clone(), cx).unwrap();
        assert_eq!(state.index_path(), Some(new_path.as_path()));
    });
}

#[gpui::test]
fn finishing_a_scan_does_not_hide_a_settings_operation_error(cx: &mut TestAppContext) {
    let music = tempfile::tempdir().unwrap();
    let root = music.path().canonicalize().unwrap();
    let index = Index::open_in_memory().unwrap();
    index.add_root(&root).unwrap();
    let (sender, receiver) = mpsc::channel();
    let state = cx.new(|_| {
        AppState::new(index, root.clone())
            .unwrap()
            .with_scanner(Box::new(PendingScanner(Mutex::new(Some(receiver)))))
    });
    state.update(cx, |state, cx| {
        state.report_settings_error("无法选择索引位置", cx);
        state.start_scan(root, cx).unwrap();
    });
    sender
        .send(ScanUpdate::Finished(voon_core::ScanResult::default(), 0))
        .unwrap();
    state.update(cx, |state, cx| {
        state.poll_scan(cx);
        assert!(matches!(state.scan_state(), ScanState::Done { .. }));
        assert_eq!(state.settings_error(), Some("无法选择索引位置"));
        assert!(state.error().is_none());
        state.clear_settings_error(cx);
        assert!(state.settings_error().is_none());
    });
}

#[gpui::test]
fn a_late_folder_choice_and_new_index_stay_blocked_until_cancellation_is_consumed(
    cx: &mut TestAppContext,
) {
    let music = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let root = music.path().canonicalize().unwrap();
    let index = Index::open_in_memory().unwrap();
    index.add_root(&root).unwrap();
    let (sender, receiver) = mpsc::channel();
    let state = cx.new(|_| {
        AppState::new(index, root.clone())
            .unwrap()
            .with_scanner(Box::new(PendingScanner(Mutex::new(Some(receiver)))))
    });
    let new_path = local.path().join("new.sqlite3");
    state.update(cx, |state, cx| {
        state.start_scan(root, cx).unwrap();
        state.cancel_scan(cx);
        assert!(state.add_root(other.path().into(), cx).is_err());
        assert!(state.create_new_index(new_path.clone(), cx).is_err());
        assert_eq!(state.roots().len(), 1);
        assert!(!new_path.exists());
    });
    sender
        .send(ScanUpdate::Finished(voon_core::ScanResult::default(), 0))
        .unwrap();
    state.update(cx, |state, cx| {
        state.poll_scan(cx);
        assert!(state.root_changes_blocked_reason().is_none());
        assert!(state.new_index_blocked_reason().is_none());
    });
}

#[gpui::test]
fn ignore_rules_saved_during_a_scan_apply_only_to_the_next_scan(cx: &mut TestAppContext) {
    let music = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let root = music.path().canonicalize().unwrap();
    for album in ["Ignored", "Kept"] {
        std::fs::create_dir(root.join(album)).unwrap();
        std::fs::write(root.join(album).join("01.dff"), b"fixture").unwrap();
    }
    let scan = scan_root(&root, &mut NoTags);
    let database = local.path().join("library.sqlite3");
    let index = Index::open(&database).unwrap();
    index.add_root(&root).unwrap();
    let (sender, receiver) = mpsc::channel();
    let state = cx.new(|_| {
        AppState::new(index, root.clone())
            .unwrap()
            .with_scanner(Box::new(PendingScanner(Mutex::new(Some(receiver)))))
    });
    state.update(cx, |state, cx| {
        state.start_scan(root.clone(), cx).unwrap();
        state
            .add_ignored_path(&root, root.join("Ignored"), cx)
            .unwrap();
        assert_eq!(
            state.ignored_paths(&root).unwrap(),
            vec![PathBuf::from("Ignored")]
        );
        assert!(matches!(state.scan_state(), ScanState::Scanning { .. }));
    });
    sender.send(ScanUpdate::Finished(scan, 0)).unwrap();
    state.update(cx, |state, cx| {
        state.poll_scan(cx);
        assert_eq!(state.albums().len(), 2);
    });
    let reopened =
        cx.new(|_| AppState::new(Index::open(&database).unwrap(), root.clone()).unwrap());
    reopened.update(cx, |state, cx| state.start_scan(root.clone(), cx).unwrap());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        reopened.update(cx, |state, cx| state.poll_scan(cx));
        if !reopened.read_with(cx, |state, _| {
            matches!(state.scan_state(), ScanState::Scanning { .. })
        }) {
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    reopened.read_with(cx, |state, _| {
        assert_eq!(state.albums().len(), 1);
        assert_eq!(state.albums()[0].path, root.join("Kept"));
        assert!(state.error().is_none());
    });
}
