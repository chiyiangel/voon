use gpui::{AppContext, TestAppContext};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use voon_app::{
    platform::{PathAction, SystemOpener},
    view_model::AppState,
};
use voon_core::{
    db::Index,
    scan::{NoTags, scan_root},
};

struct RecordingOpener(Arc<Mutex<Vec<PathBuf>>>);
impl SystemOpener for RecordingOpener {
    fn open(&self, path: &Path, action: PathAction) -> std::io::Result<()> {
        assert_eq!(action, PathAction::Open);
        self.0.lock().unwrap().push(path.into());
        Ok(())
    }
}

#[gpui::test]
fn album_and_virtual_track_open_the_whole_disc_image(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    let album = library.path().join("专辑");
    fs::create_dir(&album).unwrap();
    fs::write(album.join("disc.flac"), b"fixture").unwrap();
    fs::write(album.join("disc.cue"), "FILE \"disc.flac\" WAVE\n TRACK 01 AUDIO\n INDEX 01 00:00:00\n TRACK 02 AUDIO\n INDEX 01 03:00:00\n").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    let opened = Arc::new(Mutex::new(Vec::new()));
    let state = cx.new(|_| {
        AppState::new(index, library.path().into())
            .unwrap()
            .with_system_opener(Arc::new(RecordingOpener(opened.clone())))
    });
    state.update(cx, |state, cx| {
        state.select_album(Some(album.clone()), cx);
        state.open_album_in_player(cx).unwrap();
        state.open_track_in_player(&album, 0, 1, cx).unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    while opened.lock().unwrap().len() < 2 {
        assert!(Instant::now() < deadline, "系统打开器未收到请求");
        state.update(cx, |state, cx| state.poll_scan(cx));
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(*opened.lock().unwrap(), vec![album.join("disc.flac"); 2]);
}
