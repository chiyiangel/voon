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
