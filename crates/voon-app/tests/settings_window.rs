use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
};

use gpui::{AppContext, TestAppContext, VisualTestContext};
use voon_app::{
    application,
    ui::{SettingsPage, SettingsWindow},
    view_model::{
        AppState, ScanState,
        scanning::{ScanUpdate, Scanner},
    },
};
use voon_core::{
    db::Index,
    scan::{NoTags, scan_root},
};

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    cx.run_until_parked();
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("missing control: {selector}"));
    cx.simulate_click(bounds.center(), Default::default());
    cx.run_until_parked();
}

#[gpui::test]
fn sidebar_navigation_preserves_selection_and_drafts_and_protects_new_index(
    cx: &mut TestAppContext,
) {
    let music = tempfile::tempdir().unwrap();
    let root = music.path().canonicalize().unwrap();
    let album = root.join("Album");
    std::fs::create_dir(&album).unwrap();
    std::fs::write(album.join("01.dff"), b"fixture").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(&root, &scan_root(&root, &mut NoTags))
        .unwrap();
    let state = cx.new(|_| AppState::new(index, root).unwrap());
    state.update(cx, |state, cx| {
        state.select_album(Some(album.clone()), cx);
        state.begin_album_edit(cx).unwrap();
        state
            .set_album_edit_field("title", "未提交的编辑".into(), cx)
            .unwrap();
    });
    let (_, cx) = cx.add_window_view(|window, cx| SettingsWindow::new(state.clone(), window, cx));
    click(cx, "settings-scanning");
    assert!(cx.debug_bounds("force-album-tags").is_some());
    click(cx, "settings-ignore");
    assert!(cx.debug_bounds("add-ignore-rule-0").is_some());
    click(cx, "settings-local-data");
    click(cx, "create-separate-index");
    assert!(
        cx.debug_bounds("confirm-new-index").is_none(),
        "an open edit must disable index creation"
    );
    click(cx, "settings-directories");
    state.read_with(cx, |state, _| {
        assert_eq!(state.selected_album().unwrap().path, album);
        assert_eq!(
            state
                .album_edit_draft()
                .unwrap()
                .field("title")
                .unwrap()
                .value(),
            "未提交的编辑"
        );
    });
}

#[gpui::test]
fn new_index_requires_confirmation_then_switches_without_overwriting_the_old_file(
    cx: &mut TestAppContext,
) {
    let local = tempfile::tempdir().unwrap();
    let music = tempfile::tempdir().unwrap();
    let old_path = local.path().canonicalize().unwrap().join("old.sqlite3");
    let new_path = old_path.with_file_name("new.sqlite3");
    let state =
        cx.new(|_| AppState::new(Index::open(&old_path).unwrap(), music.path().into()).unwrap());
    let (_, cx) = cx.add_window_view(|window, cx| SettingsWindow::new(state.clone(), window, cx));
    click(cx, "settings-local-data");
    click(cx, "create-separate-index");
    assert!(cx.debug_bounds("confirm-new-index").is_some());
    click(cx, "cancel-new-index");
    assert!(!new_path.exists());
    state.read_with(cx, |state, _| {
        assert_eq!(state.index_path(), Some(old_path.as_path()))
    });
    click(cx, "create-separate-index");
    click(cx, "confirm-new-index");
    cx.simulate_new_path_selection(|_| Some(new_path.clone()));
    cx.run_until_parked();
    state.read_with(cx, |state, _| {
        assert_eq!(
            state.index_path(),
            Some(new_path.canonicalize().unwrap().as_path())
        )
    });
    assert!(old_path.exists());
    assert!(new_path.exists());
}

struct PendingScanner {
    receiver: Mutex<Option<Receiver<ScanUpdate>>>,
    cancellation: Arc<Mutex<Option<Arc<AtomicBool>>>>,
}
impl Scanner for PendingScanner {
    fn start(&self, _: PathBuf, cancel: Arc<AtomicBool>) -> Receiver<ScanUpdate> {
        *self.cancellation.lock().unwrap() = Some(cancel);
        self.receiver.lock().unwrap().take().unwrap()
    }
}

#[gpui::test]
fn scanning_from_settings_blocks_removal_and_closing_settings_keeps_the_scan_alive(
    cx: &mut TestAppContext,
) {
    let music = tempfile::tempdir().unwrap();
    let root = music.path().canonicalize().unwrap();
    let index = Index::open_in_memory().unwrap();
    index.add_root(&root).unwrap();
    let (_sender, receiver) = mpsc::channel();
    let cancellation = Arc::new(Mutex::new(None));
    let state = cx.new(|_| {
        AppState::new(index, root)
            .unwrap()
            .with_scanner(Box::new(PendingScanner {
                receiver: Mutex::new(Some(receiver)),
                cancellation: cancellation.clone(),
            }))
    });
    let (_, cx) = cx.add_window_view(|window, cx| SettingsWindow::new(state.clone(), window, cx));
    click(cx, "settings-scanning");
    click(cx, "settings-rescan-root-0");
    state.read_with(cx, |state, _| {
        assert!(matches!(state.scan_state(), ScanState::Scanning { .. }))
    });
    click(cx, "settings-directories");
    click(cx, "remove-root-0");
    state.read_with(cx, |state, _| {
        assert!(state.pending_root_removal().is_none())
    });
    click(cx, "close-settings");
    assert!(
        !cancellation
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .load(Ordering::Relaxed)
    );
    state.read_with(cx, |state, _| {
        assert!(matches!(state.scan_state(), ScanState::Scanning { .. }))
    });
}

#[gpui::test]
fn settings_shortcuts_reuse_the_window_and_open_the_requested_category(cx: &mut TestAppContext) {
    let state =
        cx.new(|_| AppState::new(Index::open_in_memory().unwrap(), "/music".into()).unwrap());
    cx.update(|cx| application::open_settings(state.clone(), cx));
    cx.run_until_parked();
    let window = cx.windows()[0];
    let mut visual = VisualTestContext::from_window(window, cx);
    assert!(visual.debug_bounds("add-library-root").is_some());
    cx.update(|cx| application::open_settings_page(state.clone(), SettingsPage::LocalData, cx));
    cx.run_until_parked();
    assert!(cx.windows() == [window]);
    assert!(visual.debug_bounds("create-separate-index").is_some());
    cx.update(|cx| application::open_settings(state.clone(), cx));
    cx.run_until_parked();
    assert!(cx.windows() == [window]);
    assert!(visual.debug_bounds("create-separate-index").is_some());
}
