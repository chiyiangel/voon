use std::time::{Duration, Instant};
use std::{fs, path::PathBuf};

use gpui::{AppContext, TestAppContext};
use voon_app::view_model::{AppState, ScanState};

#[gpui::test]
fn an_incompatible_index_stays_visible_with_a_chinese_explanation_and_keeps_its_files(
    cx: &mut TestAppContext,
) {
    let local = tempfile::tempdir().unwrap();
    let path = local.path().join("unknown.sqlite3");
    let sidecar = local.path().join("unknown.sqlite3-wal");
    fs::write(&path, b"another application's library").unwrap();
    fs::write(&sidecar, b"keep the existing sidecar").unwrap();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    let library = tempfile::tempdir().unwrap();
    let state =
        cx.new(|_| AppState::open_index(path.clone(), None, library.path().to_path_buf()).unwrap());

    state.read_with(cx, |state, _| {
        let problem = state.index_open_problem().expect("启动失败必须可见");
        assert_eq!(problem.path.as_ref(), Some(&path));
        assert!(problem.message.contains("无法识别"));
        assert!(problem.message.contains("原文件已保留"));
        assert!(problem.message.contains("新建索引重新扫描"));
        assert!(state.roots().is_empty());
        assert_eq!(state.suggested_root(), library.path());
        assert_eq!(state.scan_state(), &ScanState::Idle);
    });
    state.update(cx, |state, cx| {
        assert!(state.add_root(PathBuf::from(library.path()), cx).is_err());
    });
    assert_eq!(fs::read(&path).unwrap(), b"another application's library");
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
    assert_eq!(fs::read(&sidecar).unwrap(), b"keep the existing sidecar");
}

#[gpui::test]
fn a_user_can_keep_an_incompatible_index_create_a_separate_one_and_reopen_after_scanning(
    cx: &mut TestAppContext,
) {
    let old = tempfile::tempdir().unwrap();
    let old_path = old.path().join("old.sqlite3");
    fs::write(&old_path, b"keep this old index").unwrap();
    let old_modified = fs::metadata(&old_path).unwrap().modified().unwrap();
    let local = tempfile::tempdir().unwrap();
    let local_path = local.path().canonicalize().unwrap();
    let new_path = local_path.join("new.sqlite3");
    let selection = local_path.join("current-index-path");
    let library = tempfile::tempdir().unwrap();
    let root = library.path().canonicalize().unwrap();
    let album = root.join("新专辑");
    fs::create_dir(&album).unwrap();
    fs::write(album.join("01.dff"), b"DFF placeholder").unwrap();
    let state = cx.new(|_| {
        AppState::open_index(old_path.clone(), Some(selection.clone()), root.clone()).unwrap()
    });

    state.update(cx, |state, cx| {
        assert!(state.create_new_index(old_path.clone(), cx).is_err());
        assert!(state.index_open_problem().is_some());
        state.create_new_index(new_path.clone(), cx).unwrap();
        assert!(state.index_open_problem().is_none());
        assert_eq!(state.index_path(), Some(new_path.as_path()));
        assert!(state.index_notice().unwrap().contains("原资料库文件已保留"));
        assert_eq!(state.suggested_root(), root);
        assert!(state.roots().is_empty());
        assert_eq!(state.scan_state(), &ScanState::Idle);
        state
            .add_root(state.suggested_root().to_path_buf(), cx)
            .unwrap();
        assert_eq!(state.roots()[0].path, root);
        assert!(matches!(
            state.scan_state(),
            ScanState::Scanning { root: scanning_root, .. } if scanning_root == &root
        ));
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        state.update(cx, |state, cx| state.poll_scan(cx));
        if !state.read_with(cx, |state, _| {
            matches!(state.scan_state(), ScanState::Scanning { .. })
        }) {
            break;
        }
        assert!(Instant::now() < deadline, "新索引扫描未完成");
        std::thread::sleep(Duration::from_millis(5));
    }
    state.read_with(cx, |state, _| {
        assert_eq!(state.albums().len(), 1);
        assert!(state.error().is_none(), "{:?}", state.error());
    });

    let reopened = AppState::open_index(old_path.clone(), Some(selection), root.clone()).unwrap();
    assert!(reopened.index_open_problem().is_none());
    assert_eq!(reopened.index_path(), Some(new_path.as_path()));
    assert_eq!(reopened.albums().len(), 1);
    assert_eq!(reopened.roots()[0].path, root);
    assert_eq!(fs::read(&old_path).unwrap(), b"keep this old index");
    assert_eq!(
        fs::metadata(old_path).unwrap().modified().unwrap(),
        old_modified
    );
}

#[gpui::test]
fn failed_recovery_keeps_the_saved_unavailable_index_for_retry(cx: &mut TestAppContext) {
    let local = tempfile::tempdir().unwrap();
    let directory = local.path().canonicalize().unwrap();
    let old_path = directory.join("unavailable.sqlite3");
    let occupied = directory.join("already-exists.sqlite3");
    let selection = directory.join("current-index-path");
    fs::write(&old_path, b"keep the unavailable index").unwrap();
    fs::write(&occupied, b"keep the chosen file too").unwrap();
    let saved_selection = old_path.to_str().unwrap();
    fs::write(&selection, saved_selection).unwrap();
    let library = tempfile::tempdir().unwrap();
    let root = library.path().canonicalize().unwrap();
    let state = cx.new(|_| {
        AppState::open_index(old_path.clone(), Some(selection.clone()), root.clone()).unwrap()
    });
    let problem = state.read_with(cx, |state, _| state.index_open_problem().unwrap().clone());

    state.update(cx, |state, cx| {
        assert!(state.create_new_index(occupied.clone(), cx).is_err());
    });

    state.read_with(cx, |state, _| {
        assert_eq!(state.index_open_problem(), Some(&problem));
        assert!(state.index_path().is_none());
        assert_eq!(state.suggested_root(), root);
        assert!(state.roots().is_empty());
        assert_eq!(state.scan_state(), &ScanState::Idle);
    });
    assert_eq!(fs::read_to_string(&selection).unwrap(), saved_selection);
    assert_eq!(fs::read(&old_path).unwrap(), b"keep the unavailable index");
    assert_eq!(fs::read(&occupied).unwrap(), b"keep the chosen file too");
    let reopened = AppState::open_index(old_path.clone(), Some(selection), root).unwrap();
    assert_eq!(reopened.index_open_problem(), Some(&problem));
    assert_eq!(reopened.index_open_problem().unwrap().path, Some(old_path));
}

#[gpui::test]
fn creating_a_new_index_cannot_write_inside_the_suggested_music_library(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    let root = library.path().canonicalize().unwrap();
    let index_path = root.join("index.sqlite3");
    let state =
        cx.new(|_| AppState::new(voon_core::db::Index::open_in_memory().unwrap(), root).unwrap());

    state.update(cx, |state, cx| {
        assert!(state.create_new_index(index_path.clone(), cx).is_err());
    });

    assert!(!index_path.exists());
    assert_eq!(fs::read_dir(library.path()).unwrap().count(), 0);
}
