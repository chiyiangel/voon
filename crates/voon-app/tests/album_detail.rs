use std::path::{Path, PathBuf};
use std::{
    sync::{
        Arc, Mutex,
        atomic::AtomicBool,
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};

use gpui::{AppContext, TestAppContext};
use voon_app::platform::{PathAction, SystemOpener};
use voon_app::view_model::{
    AppState, Navigation,
    scanning::{ScanUpdate, Scanner},
};
use voon_core::{
    Album, AlbumMetadata, AudioFormat, AudioProperties, Disc, Field, FieldSource, ScanResult,
    Track, TrackSource, db::Index,
};

#[gpui::test]
fn reopened_cue_detail_keeps_each_performer_and_describes_opening_the_whole_image(
    cx: &mut TestAppContext,
) {
    let library = tempfile::tempdir().unwrap();
    let path = library.path().join("双演奏者");
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("disc.dff"), b"DFF placeholder").unwrap();
    std::fs::write(
        path.join("disc.cue"),
        "TITLE \"双演奏者\"\nFILE \"disc.dff\" WAVE\n TRACK 01 AUDIO\n  PERFORMER \"演奏者甲\"\n  INDEX 01 00:00:00\n TRACK 02 AUDIO\n  PERFORMER \"演奏者乙\"\n  INDEX 01 03:00:00\n",
    )
    .unwrap();
    let storage = tempfile::tempdir().unwrap();
    let database = storage.path().join("index.sqlite");
    {
        let mut index = Index::open(&database).unwrap();
        index
            .store_scan(
                library.path(),
                &voon_core::scan::scan_root(library.path(), &mut voon_core::scan::DiskTags),
            )
            .unwrap();
    }
    let state =
        cx.new(|_| AppState::new(Index::open(&database).unwrap(), library.path().into()).unwrap());
    state.update(cx, |state, cx| state.select_album(Some(path.clone()), cx));
    state.read_with(cx, |state, _| {
        let tracks = &state.album_detail().unwrap().discs[0].tracks;
        assert_eq!(tracks[0].properties.duration_ms, Some(180_000));
        assert_eq!(tracks[1].properties.duration_ms, None);
        assert_eq!(tracks[0].artist.as_ref().unwrap().value, "演奏者甲");
        assert_eq!(tracks[1].artist.as_ref().unwrap().value, "演奏者乙");
        assert_eq!(state.album_player_label(), Some("用默认播放器打开整个映像"));
        assert_eq!(
            state.track_player_label(0, 1),
            Some("用默认播放器打开整个映像")
        );
    });
}

fn album(path: &str, title: &str) -> Album {
    Album {
        source_directories: vec![],
        path: path.into(),
        folder_name: title.into(),
        cover: None,
        collection: None,
        signature: title.into(),
        metadata: AlbumMetadata {
            title: Some(Field::new(title.into(), FieldSource::Cue)),
            ..AlbumMetadata::default()
        },
        discs: vec![Disc {
            number: 1,
            path: Path::new(path).join("CD1"),
            tracks: vec![Track {
                number: Some(1),
                title: Some(Field::new("第一乐章".into(), FieldSource::Cue)),
                artist: None,
                format: AudioFormat::Ape,
                source: TrackSource::Virtual {
                    image: Path::new(path).join("CD1/CDImage.ape"),
                    cue: Path::new(path).join("CD1/program.cue"),
                    start: 0,
                    end: Some(13_500),
                },
                properties: AudioProperties {
                    sample_rate: Some(44_100),
                    bit_depth: Some(16),
                    channels: Some(2),
                    duration_ms: Some(180_000),
                },
            }],
        }],
    }
}

#[gpui::test]
fn selecting_an_album_loads_its_detail_and_changing_selection_replaces_it(cx: &mut TestAppContext) {
    let mut index = Index::open_in_memory().unwrap();
    let first = PathBuf::from("/假音乐库/贝多芬");
    let second = PathBuf::from("/假音乐库/莫扎特");
    index
        .store_scan(
            Path::new("/假音乐库"),
            &ScanResult {
                albums: vec![
                    album(first.to_str().unwrap(), "贝多芬"),
                    album(second.to_str().unwrap(), "莫扎特"),
                ],
                ..ScanResult::default()
            },
        )
        .unwrap();
    let state = cx.new(|_| AppState::new(index, "/建议目录".into()).unwrap());

    state.update(cx, |state, cx| state.select_album(Some(first.clone()), cx));
    state.read_with(cx, |state, _| {
        let detail = state.album_detail().unwrap();
        assert_eq!(detail.path, first);
        assert_eq!(detail.discs.len(), 1);
        assert_eq!(
            detail.discs[0].tracks[0].title.as_ref().unwrap().value,
            "第一乐章"
        );
        assert!(matches!(
            detail.discs[0].tracks[0].source,
            TrackSource::Virtual {
                end: Some(13_500),
                ..
            }
        ));
    });
    state.update(cx, |state, cx| state.select_album(Some(second.clone()), cx));
    state.read_with(cx, |state, _| {
        assert_eq!(state.album_detail().unwrap().path, second)
    });
    state.update(cx, |state, cx| state.navigate(Navigation::Collections, cx));
    state.read_with(cx, |state, _| assert!(state.album_detail().is_none()));
}

struct RecordingOpener(Arc<Mutex<Vec<(PathBuf, PathAction)>>>);
impl SystemOpener for RecordingOpener {
    fn open(&self, path: &Path, action: PathAction) -> std::io::Result<()> {
        self.0.lock().unwrap().push((path.to_path_buf(), action));
        Ok(())
    }
}

#[gpui::test]
fn attachment_images_can_be_viewed_and_the_album_can_be_revealed(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    let root = library.path().canonicalize().unwrap();
    let path = root.join("Philips Duo");
    for directory in ["Booklet/pages", "Booklet/private", "CD1", "另一专辑"] {
        std::fs::create_dir_all(path.join(directory)).unwrap();
    }
    for name in [
        "cover.jpg",
        "Booklet/pages/01.PNG",
        "CD1/folder.jpg",
        "Booklet/private/private.jpg",
        "Booklet/.hidden.jpg",
        "另一专辑/ignore.jpg",
    ] {
        std::fs::write(path.join(name), b"fixture").unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(
            &root,
            &ScanResult {
                albums: vec![album(path.to_str().unwrap(), "Philips Duo")],
                ..ScanResult::default()
            },
        )
        .unwrap();
    index
        .add_ignored_path(&root, &path.join("Booklet/private"))
        .unwrap();
    let opened = Arc::new(Mutex::new(Vec::new()));
    let opener = Arc::new(RecordingOpener(opened.clone()));
    let state = cx.new(|_| {
        AppState::new(index, root.clone())
            .unwrap()
            .with_system_opener(opener)
    });
    state.update(cx, |state, cx| state.select_album(Some(path.clone()), cx));
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        state.update(cx, |state, cx| state.poll_scan(cx));
        if !state.read_with(cx, |state, _| state.attachment_images_loading()) {
            break;
        }
        assert!(Instant::now() < deadline, "附件图片未加载");
        std::thread::sleep(Duration::from_millis(5));
    }
    state.read_with(cx, |state, _| {
        assert_eq!(
            state.attachment_images(),
            &[
                path.join("Booklet/pages/01.PNG"),
                path.join("CD1/folder.jpg"),
                path.join("cover.jpg")
            ]
        );
        assert!(state.attachment_images_error().is_none());
    });
    state.update(cx, |state, cx| {
        state
            .open_attachment_image(path.join("Booklet/pages/01.PNG"), cx)
            .unwrap();
        state.reveal_selected_album(cx).unwrap();
    });
    while state.read_with(cx, |state, _| state.file_operations_pending()) {
        state.update(cx, |state, cx| state.poll_scan(cx));
        assert!(Instant::now() < deadline, "系统文件操作未结束");
        std::thread::sleep(Duration::from_millis(5));
    }
    let opened = opened.lock().unwrap();
    assert_eq!(opened.len(), 2);
    assert!(opened.contains(&(path.join("Booklet/pages/01.PNG"), PathAction::Open)));
    assert!(opened.contains(&(path, PathAction::Reveal)));
}

struct ControlledScanner(Mutex<Option<Receiver<ScanUpdate>>>);

#[gpui::test]
fn merged_album_images_respect_ignore_rules_in_every_source_root(cx: &mut TestAppContext) {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let first_root = first.path().canonicalize().unwrap();
    let second_root = second.path().canonicalize().unwrap();
    let first_album = first_root.join("第一张");
    let second_album = second_root.join("第二张");
    std::fs::create_dir_all(&first_album).unwrap();
    std::fs::create_dir_all(second_album.join("Booklet/private")).unwrap();
    let front = first_album.join("cover.jpg");
    let booklet = second_album.join("Booklet/front.jpg");
    let private = second_album.join("Booklet/private/private.jpg");
    for path in [&front, &booklet, &private] {
        std::fs::write(path, b"image fixture").unwrap();
    }
    let mut merged = album(first_album.to_str().unwrap(), "本地合并");
    merged.path = first_root.join(".voon-structure/merged");
    merged.source_directories = vec![first_album.clone(), second_album.clone()];
    merged.discs.clear();
    let path = merged.path.clone();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(
            &first_root,
            &ScanResult {
                albums: vec![merged],
                ..ScanResult::default()
            },
        )
        .unwrap();
    index.add_root(&second_root).unwrap();
    index
        .add_ignored_path(&second_root, &second_album.join("Booklet/private"))
        .unwrap();
    let opened = Arc::new(Mutex::new(Vec::new()));
    let opener = Arc::new(RecordingOpener(opened.clone()));
    let state = cx.new(|_| {
        AppState::new(index, first_root)
            .unwrap()
            .with_system_opener(opener)
    });
    state.update(cx, |state, cx| {
        state.select_album(Some(path), cx);
        state.begin_cover_picker(cx).unwrap();
        state.reveal_selected_album(cx).unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        state.update(cx, |state, cx| state.poll_scan(cx));
        if state.read_with(cx, |state, _| {
            !state.attachment_images_loading()
                && !state.cover_picker().unwrap().loading
                && !state.file_operations_pending()
        }) {
            break;
        }
        assert!(Instant::now() < deadline, "合并来源图片未加载");
        std::thread::sleep(Duration::from_millis(5));
    }
    state.read_with(cx, |state, _| {
        assert_eq!(state.attachment_images().len(), 2);
        assert!(state.attachment_images().contains(&front));
        assert!(state.attachment_images().contains(&booklet));
        let candidates = &state.cover_picker().unwrap().candidates;
        assert_eq!(candidates.len(), 2);
        assert!(
            !candidates
                .iter()
                .any(|candidate| candidate.cover.source.path() == private)
        );
    });
    assert_eq!(
        *opened.lock().unwrap(),
        vec![(first_album, PathAction::Reveal)]
    );
}

impl Scanner for ControlledScanner {
    fn start(&self, _: PathBuf, _: Arc<AtomicBool>) -> Receiver<ScanUpdate> {
        self.0.lock().unwrap().take().unwrap()
    }
}

#[gpui::test]
fn a_new_album_can_be_inspected_before_the_first_scan_finishes(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    let root = library.path().canonicalize().unwrap();
    let path = root.join("贝多芬");
    let (sender, receiver) = mpsc::channel();
    let state = cx.new(|_| {
        AppState::new(Index::open_in_memory().unwrap(), root.clone())
            .unwrap()
            .with_scanner(Box::new(ControlledScanner(Mutex::new(Some(receiver)))))
    });
    state.update(cx, |state, cx| state.add_root(root, cx).unwrap());
    sender
        .send(ScanUpdate::Album(Box::new(album(
            path.to_str().unwrap(),
            "贝多芬",
        ))))
        .unwrap();
    state.update(cx, |state, cx| {
        state.poll_scan(cx);
        state.select_album(Some(path.clone()), cx);
    });
    state.read_with(cx, |state, _| {
        assert_eq!(state.album_detail().unwrap().path, path);
        assert_eq!(state.album_detail().unwrap().tracks().count(), 1);
    });
}

#[gpui::test]
fn a_structural_album_reveals_a_real_source_directory(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    let root = library.path().canonicalize().unwrap();
    let first = root.join("第一张");
    let second = root.join("第二张");
    for path in [&first, &second] {
        std::fs::create_dir(path).unwrap();
        std::fs::write(path.join("01.flac"), b"audio fixture").unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(
            &root,
            &voon_core::scan::scan_root(&root, &mut voon_core::scan::NoTags),
        )
        .unwrap();
    let merged = index
        .merge_albums(&[first, second], &root.join("第一张"), &[])
        .unwrap();
    let opened = Arc::new(Mutex::new(Vec::new()));
    let opener = Arc::new(RecordingOpener(opened.clone()));
    let state = cx.new(|_| {
        AppState::new(index, root)
            .unwrap()
            .with_system_opener(opener)
    });
    state.update(cx, |state, cx| {
        state.select_album(Some(merged), cx);
        state.reveal_selected_album(cx).unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    while state.read_with(cx, |state, _| state.file_operations_pending()) {
        state.update(cx, |state, cx| state.poll_scan(cx));
        assert!(Instant::now() < deadline, "显示目录操作未结束");
        std::thread::sleep(Duration::from_millis(5));
    }
    let opened = opened.lock().unwrap();
    assert_eq!(opened.len(), 1);
    assert_eq!(opened[0].1, PathAction::Reveal);
    assert!(
        opened[0].0.is_dir(),
        "显示目录必须使用真实来源，收到：{}",
        opened[0].0.display()
    );
}
