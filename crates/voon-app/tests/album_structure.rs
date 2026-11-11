use gpui::{AppContext, TestAppContext};
use voon_app::view_model::AppState;
use voon_core::{
    db::{Index, TrackRef},
    scan::{NoTags, scan_root},
};

#[gpui::test]
fn rescanning_a_structured_root_does_not_flash_physical_album_previews(cx: &mut TestAppContext) {
    use std::sync::{
        Arc, Mutex,
        atomic::AtomicBool,
        mpsc::{self, Receiver},
    };
    use voon_app::view_model::scanning::{ScanUpdate, Scanner};
    struct Controlled(Mutex<Option<Receiver<ScanUpdate>>>);
    impl Scanner for Controlled {
        fn start(&self, _: std::path::PathBuf, _: Arc<AtomicBool>) -> Receiver<ScanUpdate> {
            self.0.lock().unwrap().take().unwrap()
        }
    }
    let library = tempfile::tempdir().unwrap();
    let first = library.path().join("甲");
    let second = library.path().join("乙");
    for path in [&first, &second] {
        std::fs::create_dir(path).unwrap();
        std::fs::write(path.join("01.flac"), b"fixture").unwrap();
    }
    let scan = scan_root(library.path(), &mut NoTags);
    let mut index = Index::open_in_memory().unwrap();
    index.store_scan(library.path(), &scan).unwrap();
    let merged = index
        .merge_albums(&[first.clone(), second], &first, &[])
        .unwrap();
    let (sender, receiver) = mpsc::channel();
    let state = cx.new(|_| {
        AppState::new(index, library.path().into())
            .unwrap()
            .with_scanner(Box::new(Controlled(Mutex::new(Some(receiver)))))
    });
    state.update(cx, |state, cx| {
        state.select_album(Some(merged.clone()), cx);
        state.start_scan(library.path().into(), cx).unwrap();
    });
    sender
        .send(ScanUpdate::Album(Box::new(scan.albums[0].clone())))
        .unwrap();
    state.update(cx, |state, cx| {
        state.poll_scan(cx);
        assert_eq!(state.albums().len(), 1);
        assert_eq!(state.selected_album().unwrap().path, merged);
        assert!(state.begin_split(cx).is_err());
    });
    sender
        .send(ScanUpdate::Unavailable(voon_core::ScanResult::default()))
        .unwrap();
    state.update(cx, |state, cx| {
        state.poll_scan(cx);
        assert_eq!(state.albums().len(), 1);
        assert_eq!(state.selected_album().unwrap().path, merged);
        state.begin_structure_undo(cx).unwrap();
        state.confirm_structure(cx).unwrap();
        assert_eq!(state.albums().len(), 2);
    });
}

#[gpui::test]
fn structure_confirmation_updates_selection_and_undo_restores_the_library(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    let first = library.path().join("第一张");
    let second = library.path().join("第二张");
    for path in [&first, &second] {
        std::fs::create_dir(path).unwrap();
        std::fs::write(path.join("01.flac"), b"fixture").unwrap();
    }
    std::fs::write(first.join("02.flac"), b"second fixture").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    let state = cx.new(|_| AppState::new(index, library.path().into()).unwrap());
    state.update(cx, |state, cx| {
        state.click_album(first.clone(), false, false, cx);
        state.click_album(second.clone(), true, false, cx);
        state.begin_merge(cx).unwrap();
        state.cancel_structure(cx);
        assert_eq!(state.albums().len(), 2);
        state.begin_merge(cx).unwrap();
        state.choose_merge_primary(second.clone(), cx).unwrap();
        state.move_merge_disc(1, -1, cx).unwrap();
        state.confirm_structure(cx).unwrap();
        assert_eq!(state.albums().len(), 1);
        let detail = state.album_detail().unwrap();
        assert_eq!(detail.path, state.selected_album().unwrap().path);
        assert_eq!(detail.display_title(), "第二张");
        assert_eq!(
            detail.discs[0].tracks[0].source.audio_path(),
            &second.join("01.flac")
        );
        state.begin_structure_undo(cx).unwrap();
        state.confirm_structure(cx).unwrap();
        assert_eq!(state.albums().len(), 2);

        state.select_album(Some(first.clone()), cx);
        state.begin_split(cx).unwrap();
        state.set_split_by_tracks(true, cx).unwrap();
        state
            .toggle_split_track(TrackRef { disc: 0, track: 0 }, cx)
            .unwrap();
        state.confirm_structure(cx).unwrap();
        assert_eq!(state.albums().len(), 3);
        assert!(state.selected_album().is_some());
        state.begin_structure_undo(cx).unwrap();
        state.confirm_structure(cx).unwrap();
        assert_eq!(state.albums().len(), 2);
        assert!(state.structure_draft().is_none());
    });
}
