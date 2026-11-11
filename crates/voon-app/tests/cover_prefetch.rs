use gpui::{AppContext, TestAppContext};
use voon_app::view_model::{
    AppState, ScanState, cover_thumbnails::ThumbnailState, grid::CoverStatus,
};
use voon_core::{Album, AlbumMetadata, Cover, CoverSource, ScanResult, db::Index};

#[gpui::test]
async fn accepted_disk_scans_prefetch_covers_and_removed_roots_retire_old_work(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("music");
    let mut sources = Vec::new();
    for name in ["A", "B"] {
        let album = root.join(name);
        std::fs::create_dir_all(&album).unwrap();
        let audio = album.join("01.dff");
        std::fs::write(&audio, b"unsupported DFF fixture").unwrap();
        let image = album.join("cover.png");
        image::RgbaImage::from_pixel(20, 10, image::Rgba([200, 40, 10, 255]))
            .save(&image)
            .unwrap();
        sources.extend([audio, image]);
    }
    let original: Vec<_> = sources
        .iter()
        .map(|path| {
            (
                std::fs::read(path).unwrap(),
                std::fs::metadata(path).unwrap().modified().unwrap(),
            )
        })
        .collect();
    let database = directory.path().join("library.sqlite3");
    let state = cx.new(|_| AppState::new(Index::open(&database).unwrap(), root.clone()).unwrap());
    state.update(cx, |state, cx| state.add_root(root.clone(), cx).unwrap());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        state.update(cx, |state, cx| state.poll_scan(cx));
        if state.read_with(cx, |state, _| {
            matches!(state.scan_state(), ScanState::Done { discovered: 2, .. })
        }) {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "真实扫描未接受专辑");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    cx.condition(&state, |state, _| state.cover_cache_progress().ready == 2)
        .await;
    for (path, (bytes, modified)) in sources.iter().zip(original) {
        assert_eq!(std::fs::read(path).unwrap(), bytes);
        assert_eq!(
            std::fs::metadata(path).unwrap().modified().unwrap(),
            modified
        );
    }
    let accepted_root = state.read_with(cx, |state, _| state.roots()[0].path.clone());
    state.update(cx, |state, cx| {
        state.reload_library(cx).unwrap();
        state.request_root_removal(Some(accepted_root), cx);
        state.confirm_root_removal(cx).unwrap();
    });
    cx.background_executor.run_until_parked();
    state.read_with(cx, |state, _| {
        assert_eq!(state.cover_cache_progress().total, 0);
        assert!(state.albums().is_empty());
        assert!(state.roots().is_empty());
    });
}

#[gpui::test]
async fn loading_the_library_prefetches_unbrowsed_automatic_and_selected_covers(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("music");
    std::fs::create_dir(&root).unwrap();
    let automatic_path = root.join("automatic.png");
    let selected_path = root.join("selected.png");
    for (path, dimensions) in [(&automatic_path, (400, 200)), (&selected_path, (200, 400))] {
        image::RgbaImage::from_pixel(dimensions.0, dimensions.1, image::Rgba([200, 40, 10, 255]))
            .save(path)
            .unwrap();
    }
    let automatic = Cover {
        source: CoverSource::File(automatic_path),
    };
    let selected = Cover {
        source: CoverSource::File(selected_path),
    };
    let database = directory.path().join("library.sqlite3");
    let mut index = Index::open(&database).unwrap();
    index
        .store_scan(
            &root,
            &ScanResult {
                albums: ["Automatic", "Selected", "No cover"]
                    .into_iter()
                    .map(|name| Album {
                        source_directories: vec![],
                        path: root.join(name),
                        folder_name: name.into(),
                        discs: vec![],
                        cover: (name != "No cover").then(|| automatic.clone()),
                        metadata: AlbumMetadata::default(),
                        collection: None,
                        signature: name.into(),
                    })
                    .collect(),
                ..ScanResult::default()
            },
        )
        .unwrap();
    index
        .set_user_cover(&root.join("Selected"), &selected)
        .unwrap();
    let state = cx.new(|_| AppState::new(index, root.clone()).unwrap());
    state.read_with(cx, |state, _| {
        let progress = state.cover_cache_progress();
        assert_eq!(
            (progress.total, progress.pending, progress.no_cover),
            (3, 2, 1)
        );
        assert_eq!(
            state.album_cover_state(&root.join("No cover")),
            Some(ThumbnailState::NoCover)
        );
    });
    // poll_scan is the same lifecycle tick used by VoonWindow. No viewport or
    // individual Cover request is made before both assets become ready.
    state.update(cx, |state, cx| state.poll_scan(cx));
    cx.condition(&state, |state, _| state.cover_cache_progress().ready == 2)
        .await;
    state.read_with(cx, |state, _| {
        let progress = state.cover_cache_progress();
        assert_eq!(
            (progress.pending, progress.loading, progress.failed),
            (0, 0, 0)
        );
        assert_eq!(state.scan_state(), &ScanState::Idle);
        assert!(state.selected_album().is_none());
        assert!(state.cover_thumbnail_path(&automatic).unwrap().is_file());
        assert!(state.cover_thumbnail_path(&selected).unwrap().is_file());
    });
    drop(state);
    std::fs::rename(&root, directory.path().join("unmounted")).unwrap();
    let reopened =
        cx.new(|_| AppState::new(Index::open(&database).unwrap(), root.clone()).unwrap());
    reopened.update(cx, |state, cx| state.poll_scan(cx));
    cx.condition(&reopened, |state, _| {
        state.cover_cache_progress().ready == 2
    })
    .await;
    reopened.update(cx, |state, cx| {
        state.select_album(Some(root.join("Selected")), cx);
        assert_eq!(
            state.selected_album().unwrap().cover,
            Some(selected.clone())
        );
        assert_eq!(state.album_detail().unwrap().cover, Some(selected.clone()));
        state.request_cover_thumbnail(selected.clone(), cx);
    });
    cx.condition(&reopened, |state, _| {
        matches!(
            state.cover_thumbnail(&selected),
            Some(CoverStatus::Ready(_))
        )
    })
    .await;
    reopened.read_with(cx, |state, _| {
        let image = image::open(state.cover_thumbnail_path(&selected).unwrap()).unwrap();
        assert_eq!((image.width(), image.height()), (128, 256));
    });
}

#[gpui::test]
async fn failed_cover_prefetch_can_be_retried_without_rescanning_the_library(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("music");
    std::fs::create_dir(&root).unwrap();
    let good_path = root.join("good.png");
    let bad_path = root.join("bad.png");
    image::RgbaImage::from_pixel(20, 10, image::Rgba([200, 40, 10, 255]))
        .save(&good_path)
        .unwrap();
    std::fs::write(&bad_path, b"temporarily damaged image").unwrap();
    let good = Cover {
        source: CoverSource::File(good_path),
    };
    let bad = Cover {
        source: CoverSource::File(bad_path.clone()),
    };
    let database = directory.path().join("library.sqlite3");
    let mut index = Index::open(&database).unwrap();
    index
        .store_scan(
            &root,
            &ScanResult {
                albums: [("Good", &good), ("Bad", &bad)]
                    .into_iter()
                    .map(|(name, cover)| Album {
                        source_directories: vec![],
                        path: root.join(name),
                        folder_name: name.into(),
                        discs: vec![],
                        cover: Some(cover.clone()),
                        metadata: AlbumMetadata::default(),
                        collection: None,
                        signature: name.into(),
                    })
                    .collect(),
                ..ScanResult::default()
            },
        )
        .unwrap();
    let state = cx.new(|_| AppState::new(index, root.clone()).unwrap());
    state.update(cx, |state, cx| state.poll_scan(cx));
    cx.condition(&state, |state, _| {
        let progress = state.cover_cache_progress();
        progress.ready == 1 && progress.failed == 1
    })
    .await;
    let good_asset = state.read_with(cx, |state, _| {
        state.cover_thumbnail_path(&good).unwrap().to_path_buf()
    });
    let good_stamp = std::fs::metadata(&good_asset).unwrap().modified().unwrap();
    state.read_with(cx, |state, _| {
        assert!(matches!(state.album_cover_state(&root.join("Bad")), Some(ThumbnailState::Failed(reason)) if reason.contains("无法解码") && reason.contains("bad.png")));
        assert_eq!(state.scan_state(), &ScanState::Idle);
    });
    image::RgbaImage::from_pixel(10, 20, image::Rgba([10, 40, 200, 255]))
        .save(&bad_path)
        .unwrap();
    state.update(cx, |state, cx| state.retry_failed_covers(cx));
    cx.condition(&state, |state, _| state.cover_cache_progress().ready == 2)
        .await;
    state.read_with(cx, |state, _| {
        let progress = state.cover_cache_progress();
        assert_eq!(
            (progress.failed, progress.pending, progress.loading),
            (0, 0, 0)
        );
        assert_eq!(state.scan_state(), &ScanState::Idle);
        assert_eq!(state.albums().len(), 2);
        assert_eq!(
            state.cover_thumbnail_path(&good),
            Some(good_asset.as_path())
        );
        assert!(state.cover_thumbnail_path(&bad).unwrap().is_file());
    });
    assert_eq!(
        std::fs::metadata(good_asset).unwrap().modified().unwrap(),
        good_stamp
    );
}

#[gpui::test]
async fn whole_library_prefetch_is_bounded_and_memory_eviction_does_not_reset_asset_progress(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("music");
    std::fs::create_dir(&root).unwrap();
    let mut covers = Vec::new();
    let mut albums = Vec::new();
    for number in 0..260 {
        let source = root.join(format!("{number}.png"));
        image::RgbaImage::from_pixel(4, 2, image::Rgba([200, 40, 10, 255]))
            .save(&source)
            .unwrap();
        let cover = Cover {
            source: CoverSource::File(source),
        };
        albums.push(Album {
            source_directories: vec![],
            path: root.join(number.to_string()),
            folder_name: number.to_string(),
            discs: vec![],
            cover: Some(cover.clone()),
            metadata: AlbumMetadata::default(),
            collection: None,
            signature: number.to_string(),
        });
        covers.push(cover);
    }
    let database = directory.path().join("library.sqlite3");
    let mut index = Index::open(&database).unwrap();
    index
        .store_scan(
            &root,
            &ScanResult {
                albums,
                ..ScanResult::default()
            },
        )
        .unwrap();
    let state = cx.new(|_| AppState::new(index, root.clone()).unwrap());
    state.update(cx, |state, cx| {
        state.poll_scan(cx);
        let progress = state.cover_cache_progress();
        assert_eq!((progress.loading, progress.pending), (4, 256));
    });
    cx.condition(&state, |state, _| state.cover_cache_progress().ready == 260)
        .await;
    state.read_with(cx, |state, _| {
        assert!(
            covers
                .iter()
                .filter(|cover| matches!(state.cover_thumbnail(cover), Some(CoverStatus::Ready(_))))
                .count()
                <= 256
        );
        assert!(
            covers
                .iter()
                .all(|cover| state.cover_thumbnail_path(cover).unwrap().is_file())
        );
    });
    state.update(cx, |state, cx| {
        state.request_cover_thumbnail(covers[0].clone(), cx);
        assert_eq!(
            state.cover_cache_progress().ready,
            260,
            "重新解码本地图片不应把持久资产改为未缓存"
        );
        assert!(state.cover_thumbnail_path(&covers[0]).unwrap().is_file());
    });
    cx.condition(&state, |state, _| {
        matches!(
            state.cover_thumbnail(&covers[0]),
            Some(CoverStatus::Ready(_))
        )
    })
    .await;
    drop(state);
    std::fs::rename(&root, directory.path().join("unmounted")).unwrap();
    let reopened = cx.new(|_| AppState::new(Index::open(&database).unwrap(), root).unwrap());
    reopened.update(cx, |state, cx| state.poll_scan(cx));
    cx.condition(&reopened, |state, _| {
        state.cover_cache_progress().ready == 260
    })
    .await;
    reopened.update(cx, |state, cx| {
        state.request_cover_thumbnail(covers[0].clone(), cx);
    });
    cx.condition(&reopened, |state, _| {
        matches!(
            state.cover_thumbnail(&covers[0]),
            Some(CoverStatus::Ready(_))
        )
    })
    .await;
}
