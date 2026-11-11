use std::{
    path::Path,
    time::{Duration, Instant},
};

use gpui::{AppContext, Entity, TestAppContext};
use voon_app::view_model::{AppState, ScanState};
use voon_core::{
    db::Index,
    scan::{NoTags, scan_root},
};

fn finish_scan(state: &Entity<AppState>, cx: &mut TestAppContext) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        state.update(cx, |state, cx| state.poll_scan(cx));
        if state.read_with(cx, |state, _| {
            !matches!(state.scan_state(), ScanState::Scanning { .. })
        }) {
            break;
        }
        assert!(Instant::now() < deadline, "扫描未完成");
        std::thread::sleep(Duration::from_millis(5));
    }
    state.read_with(cx, |state, _| {
        assert!(state.error().is_none(), "{:?}", state.error());
        assert!(matches!(state.scan_state(), ScanState::Done { .. }));
    });
}

#[gpui::test]
fn ignoring_a_subfolder_updates_albums_after_rescan_and_can_be_undone(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let root = library.path().canonicalize().unwrap();
    for relative in [
        "Philips Duo/Schubert/CD1/01.flac",
        "Alfred_Brendel/Mozart/Disc 1/01.wv",
    ] {
        let file = root.join(relative);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, b"fixture").unwrap();
    }
    let database = local.path().join("library.sqlite3");
    let mut index = Index::open(&database).unwrap();
    index
        .store_scan(&root, &scan_root(&root, &mut NoTags))
        .unwrap();
    let state = cx.new(|_| AppState::new(index, root.clone()).unwrap());

    state.update(cx, |state, cx| {
        state
            .add_ignored_path(&root, root.join("Philips Duo"), cx)
            .unwrap()
    });
    state.read_with(cx, |state, _| {
        assert_eq!(state.albums().len(), 2, "规则在下次扫描生效");
        assert_eq!(
            state.ignored_paths(&root).unwrap(),
            vec![Path::new("Philips Duo")]
        );
    });
    state.update(cx, |state, cx| state.start_scan(root.clone(), cx).unwrap());
    finish_scan(&state, cx);
    state.read_with(cx, |state, _| {
        assert_eq!(
            state
                .albums()
                .iter()
                .map(|album| album.folder_name.as_str())
                .collect::<Vec<_>>(),
            ["Mozart"]
        );
    });

    state.update(cx, |state, cx| {
        state
            .remove_ignored_path(&root, Path::new("Philips Duo"), cx)
            .unwrap();
        state.start_scan(root.clone(), cx).unwrap();
    });
    finish_scan(&state, cx);
    state.read_with(cx, |state, _| assert_eq!(state.albums().len(), 2));
}

#[gpui::test]
fn folder_selection_rejects_the_root_files_and_folders_outside_the_library(
    cx: &mut TestAppContext,
) {
    let library = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let root = library.path().canonicalize().unwrap();
    std::fs::write(root.join("01.flac"), b"fixture").unwrap();
    let index = Index::open_in_memory().unwrap();
    index.add_root(&root).unwrap();
    let state = cx.new(|_| AppState::new(index, root.clone()).unwrap());
    for path in [
        root.clone(),
        root.join("01.flac"),
        outside.path().to_path_buf(),
        root.join("Album/../Temporary"),
    ] {
        state.update(cx, |state, cx| {
            assert!(state.add_ignored_path(&root, path, cx).is_err())
        });
    }
    state.read_with(cx, |state, _| {
        assert!(state.ignored_paths(&root).unwrap().is_empty())
    });
}

#[gpui::test]
fn a_saved_rule_can_be_removed_while_its_library_is_offline(cx: &mut TestAppContext) {
    let root = Path::new("/未挂载的音乐库");
    let index = Index::open_in_memory().unwrap();
    index.add_root(root).unwrap();
    index
        .add_ignored_path(root, Path::new("Temporary"))
        .unwrap();
    let state = cx.new(|_| AppState::new(index, root.into()).unwrap());
    state.update(cx, |state, cx| {
        state
            .remove_ignored_path(root, Path::new("Temporary"), cx)
            .unwrap()
    });
    state.read_with(cx, |state, _| {
        assert!(state.ignored_paths(root).unwrap().is_empty())
    });
}
