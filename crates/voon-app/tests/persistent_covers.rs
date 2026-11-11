use gpui::{AppContext, TestAppContext};
use std::path::{Path, PathBuf};
use voon_app::view_model::{AppState, grid::CoverStatus};
use voon_core::{Album, AlbumMetadata, Cover, CoverSource, ScanResult, db::Index};

fn png(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        width,
        height,
        image::Rgba(color),
    ))
    .write_to(&mut bytes, image::ImageFormat::Png)
    .unwrap();
    bytes.into_inner()
}

fn dsf_with_two_covers(path: &Path) {
    use id3::TagLike;
    let mut tag = id3::Tag::new();
    for (description, picture_type, data) in [
        (
            "正面",
            id3::frame::PictureType::CoverFront,
            png(400, 200, [200, 40, 10, 255]),
        ),
        (
            "背面",
            id3::frame::PictureType::CoverBack,
            png(200, 400, [10, 40, 200, 255]),
        ),
    ] {
        tag.add_frame(id3::frame::Picture {
            mime_type: "image/png".into(),
            picture_type,
            description: description.into(),
            data,
        });
    }
    let mut bytes = vec![0; 80];
    bytes[..4].copy_from_slice(b"DSD ");
    bytes[20..28].copy_from_slice(&80u64.to_le_bytes());
    bytes[28..32].copy_from_slice(b"fmt ");
    tag.write_to(&mut bytes, id3::Version::Id3v24).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn store_album(index: &mut Index, root: &Path, name: &str, cover: Cover) -> PathBuf {
    let path = root.join(name);
    index
        .store_scan(
            root,
            &ScanResult {
                albums: vec![Album {
                    source_directories: vec![],
                    path: path.clone(),
                    folder_name: name.into(),
                    discs: vec![],
                    cover: Some(cover),
                    metadata: AlbumMetadata::default(),
                    collection: None,
                    signature: name.into(),
                }],
                ..ScanResult::default()
            },
        )
        .unwrap();
    path
}

#[gpui::test]
async fn automatic_and_manually_selected_embedded_pictures_reopen_offline(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("music");
    let album_path = root.join("Album");
    std::fs::create_dir_all(&album_path).unwrap();
    let source = album_path.join("image.dsf");
    dsf_with_two_covers(&source);
    let automatic = Cover {
        source: CoverSource::Embedded(source.clone()),
    };
    let chosen = Cover {
        source: CoverSource::EmbeddedPicture {
            path: source,
            index: 1,
        },
    };
    let database = directory.path().join("library.sqlite3");
    let mut index = Index::open(&database).unwrap();
    let mut scan = voon_core::scan::scan_root(&root, &mut voon_core::scan::NoTags);
    scan.albums[0].cover = Some(automatic.clone());
    index.store_scan(&root, &scan).unwrap();
    let state = cx.new(|_| AppState::new(index, root.clone()).unwrap());
    state.update(cx, |state, cx| {
        state.request_cover_thumbnail(automatic.clone(), cx);
    });
    cx.condition(&state, |state, _| {
        matches!(
            state.cover_thumbnail(&automatic),
            Some(CoverStatus::Ready(_))
        )
    })
    .await;
    state.read_with(cx, |state, _| {
        let image = image::open(state.cover_thumbnail_path(&automatic).unwrap()).unwrap();
        assert_eq!((image.width(), image.height()), (256, 128));
    });
    state.update(cx, |state, cx| {
        state.select_album(Some(album_path.clone()), cx);
        state.begin_cover_picker(cx).unwrap();
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        state.update(cx, |state, cx| state.poll_scan(cx));
        if !state.read_with(cx, |state, _| state.cover_picker().unwrap().loading) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "真实内嵌封面候选未完成"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    state.update(cx, |state, cx| {
        state
            .select_cover_candidate(Some(chosen.clone()), cx)
            .unwrap();
        state.apply_cover_choice(cx).unwrap();
        state.request_cover_thumbnail(chosen.clone(), cx);
    });
    cx.condition(&state, |state, _| {
        matches!(state.cover_thumbnail(&chosen), Some(CoverStatus::Ready(_)))
    })
    .await;
    drop(state);
    std::fs::rename(&root, directory.path().join("unmounted")).unwrap();
    let reopened =
        cx.new(|_| AppState::new(Index::open(&database).unwrap(), root.clone()).unwrap());
    reopened.update(cx, |state, cx| {
        state.select_album(Some(album_path), cx);
        assert_eq!(state.album_detail().unwrap().cover, Some(chosen.clone()));
        state.request_cover_thumbnail(chosen.clone(), cx);
    });
    cx.condition(&reopened, |state, _| {
        matches!(state.cover_thumbnail(&chosen), Some(CoverStatus::Ready(_)))
    })
    .await;
    reopened.read_with(cx, |state, _| {
        let image = image::open(state.cover_thumbnail_path(&chosen).unwrap()).unwrap();
        assert_eq!((image.width(), image.height()), (128, 256));
    });
}

#[gpui::test]
async fn requested_cover_reopens_from_a_local_thumbnail_after_source_goes_offline(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("music");
    std::fs::create_dir(&root).unwrap();
    let source = root.join("cover.png");
    image::RgbaImage::from_pixel(800, 400, image::Rgba([200, 40, 10, 255]))
        .save(&source)
        .unwrap();
    let original = std::fs::read(&source).unwrap();
    let modified = std::fs::metadata(&source).unwrap().modified().unwrap();
    let cover = Cover {
        source: CoverSource::File(source.clone()),
    };
    let database = directory.path().join("library.sqlite3");
    let mut index = Index::open(&database).unwrap();
    let album = store_album(&mut index, &root, "Album", cover.clone());
    let state = cx.new(|_| AppState::new(index, root.clone()).unwrap());
    state.update(cx, |state, cx| {
        state.request_cover_thumbnail(cover.clone(), cx);
    });
    cx.condition(&state, |state, _| {
        matches!(state.cover_thumbnail(&cover), Some(CoverStatus::Ready(_)))
    })
    .await;
    let asset = state.read_with(cx, |state, _| {
        state.cover_thumbnail_path(&cover).unwrap().to_path_buf()
    });
    assert!(!asset.starts_with(&root));
    let thumbnail = image::open(&asset).unwrap();
    assert_eq!((thumbnail.width(), thumbnail.height()), (256, 128));
    assert_eq!(std::fs::read(&source).unwrap(), original);
    assert_eq!(
        std::fs::metadata(&source).unwrap().modified().unwrap(),
        modified
    );
    drop(state);
    std::fs::rename(&root, directory.path().join("unmounted")).unwrap();

    let reopened =
        cx.new(|_| AppState::new(Index::open(&database).unwrap(), root.clone()).unwrap());
    reopened.update(cx, |state, cx| {
        state.select_album(Some(album), cx);
        assert_eq!(state.selected_album().unwrap().cover, Some(cover.clone()));
        assert_eq!(state.album_detail().unwrap().cover, Some(cover.clone()));
        state.request_cover_thumbnail(cover.clone(), cx);
    });
    cx.condition(&reopened, |state, _| {
        matches!(state.cover_thumbnail(&cover), Some(CoverStatus::Ready(_)))
    })
    .await;
    reopened.read_with(cx, |state, _| {
        assert_eq!(state.cover_thumbnail_path(&cover), Some(asset.as_path()))
    });
}

#[gpui::test]
async fn confirmed_source_changes_replace_the_thumbnail_and_missing_sources_keep_it(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("music");
    std::fs::create_dir(&root).unwrap();
    let source = root.join("cover.png");
    std::fs::write(&source, png(400, 200, [200, 40, 10, 255])).unwrap();
    let cover = Cover {
        source: CoverSource::File(source.clone()),
    };
    let database = directory.path().join("library.sqlite3");
    let mut index = Index::open(&database).unwrap();
    store_album(&mut index, &root, "Album", cover.clone());
    let state = cx.new(|_| AppState::new(index, root).unwrap());
    state.update(cx, |state, cx| {
        state.request_cover_thumbnail(cover.clone(), cx);
    });
    cx.condition(&state, |state, _| {
        matches!(state.cover_thumbnail(&cover), Some(CoverStatus::Ready(_)))
    })
    .await;
    std::fs::write(&source, png(200, 400, [10, 40, 200, 255])).unwrap();
    state.update(cx, |state, cx| {
        state.reload_library(cx).unwrap();
        state.request_cover_thumbnail(cover.clone(), cx);
    });
    cx.condition(&state, |state, _| {
        matches!(state.cover_thumbnail(&cover), Some(CoverStatus::Ready(_)))
    })
    .await;
    state.read_with(cx, |state, _| {
        let image = image::open(state.cover_thumbnail_path(&cover).unwrap()).unwrap();
        assert_eq!((image.width(), image.height()), (128, 256));
    });
    std::fs::remove_file(source).unwrap();
    state.update(cx, |state, cx| {
        state.reload_library(cx).unwrap();
        state.request_cover_thumbnail(cover.clone(), cx);
    });
    cx.condition(&state, |state, _| {
        matches!(state.cover_thumbnail(&cover), Some(CoverStatus::Ready(_)))
    })
    .await;
    state.read_with(cx, |state, _| {
        let image = image::open(state.cover_thumbnail_path(&cover).unwrap()).unwrap();
        assert_eq!((image.width(), image.height()), (128, 256));
    });
}

#[gpui::test]
async fn a_corrupt_local_thumbnail_does_not_clear_selection_or_other_albums(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let music = directory.path().join("music");
    let database = directory.path().join("library.sqlite3");
    let mut index = Index::open(&database).unwrap();
    let mut covers = Vec::new();
    let mut albums = Vec::new();
    for name in ["A", "B"] {
        let root = music.join(name);
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("cover.png");
        std::fs::write(&source, png(30, 20, [200, 40, 10, 255])).unwrap();
        let cover = Cover {
            source: CoverSource::File(source),
        };
        albums.push(store_album(&mut index, &root, "Album", cover.clone()));
        covers.push(cover);
    }
    index.set_user_cover(&albums[0], &covers[0]).unwrap();
    let state = cx.new(|_| AppState::new(index, music.clone()).unwrap());
    state.update(cx, |state, cx| {
        for cover in &covers {
            state.request_cover_thumbnail(cover.clone(), cx);
        }
    });
    cx.condition(&state, |state, _| {
        covers
            .iter()
            .all(|cover| matches!(state.cover_thumbnail(cover), Some(CoverStatus::Ready(_))))
    })
    .await;
    let bad_asset = state.read_with(cx, |state, _| {
        state
            .cover_thumbnail_path(&covers[0])
            .unwrap()
            .to_path_buf()
    });
    drop(state);
    // A damaged/replaced asset may still be a decodable image. It must not
    // silently become this Album's current Cover while its source is offline.
    std::fs::write(bad_asset, png(20, 30, [10, 40, 200, 255])).unwrap();
    std::fs::rename(&music, directory.path().join("unmounted")).unwrap();
    let reopened = cx.new(|_| AppState::new(Index::open(&database).unwrap(), music).unwrap());
    reopened.update(cx, |state, cx| {
        state.select_album(Some(albums[0].clone()), cx);
        for cover in &covers {
            state.request_cover_thumbnail(cover.clone(), cx);
        }
    });
    cx.condition(&reopened, |state, _| {
        matches!(
            state.cover_thumbnail(&covers[0]),
            Some(CoverStatus::Unavailable)
        ) && matches!(
            state.cover_thumbnail(&covers[1]),
            Some(CoverStatus::Ready(_))
        )
    })
    .await;
    reopened.read_with(cx, |state, _| {
        assert_eq!(state.albums().len(), 2);
        assert_eq!(state.selected_album().unwrap().path, albums[0]);
        assert_eq!(state.album_detail().unwrap().cover, Some(covers[0].clone()));
        assert!(state.cover_thumbnail_path(&covers[0]).is_none());
        assert!(image::open(state.cover_thumbnail_path(&covers[1]).unwrap()).is_ok());
    });
}

#[gpui::test]
async fn superseded_background_work_cannot_publish_over_a_new_cover_choice(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("music");
    std::fs::create_dir(&root).unwrap();
    let old_source = root.join("cover.png");
    let new_source = root.join("chosen.png");
    std::fs::write(&old_source, png(400, 200, [200, 40, 10, 255])).unwrap();
    std::fs::write(&new_source, png(200, 400, [10, 40, 200, 255])).unwrap();
    let old = Cover {
        source: CoverSource::File(old_source),
    };
    let chosen = Cover {
        source: CoverSource::File(new_source),
    };
    let database = directory.path().join("library.sqlite3");
    let mut index = Index::open(&database).unwrap();
    let album = store_album(&mut index, &root, "Album", old.clone());
    let state = cx.new(|_| AppState::new(index, root).unwrap());
    state.update(cx, |state, cx| {
        state.request_cover_thumbnail(old.clone(), cx);
    });
    // Before the GPUI background executor runs, a newer accepted Cover retires
    // the queued old request. Both real image jobs are then allowed to finish.
    Index::open(&database)
        .unwrap()
        .set_user_cover(&album, &chosen)
        .unwrap();
    state.update(cx, |state, cx| {
        state.reload_library(cx).unwrap();
        state.select_album(Some(album), cx);
        state.request_cover_thumbnail(chosen.clone(), cx);
    });
    cx.condition(&state, |state, _| {
        matches!(state.cover_thumbnail(&chosen), Some(CoverStatus::Ready(_)))
    })
    .await;
    cx.background_executor.run_until_parked();
    state.read_with(cx, |state, _| {
        assert_eq!(state.selected_album().unwrap().cover, Some(chosen.clone()));
        assert_eq!(state.album_detail().unwrap().cover, Some(chosen.clone()));
        assert!(state.cover_thumbnail(&old).is_none());
        let image = image::open(state.cover_thumbnail_path(&chosen).unwrap()).unwrap();
        assert_eq!((image.width(), image.height()), (128, 256));
    });
}

#[gpui::test]
async fn thumbnail_write_failure_never_claims_ready_or_discards_the_album(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("music");
    std::fs::create_dir(&root).unwrap();
    let source = root.join("cover.png");
    std::fs::write(&source, png(30, 20, [200, 40, 10, 255])).unwrap();
    let cover = Cover {
        source: CoverSource::File(source),
    };
    let database = directory.path().join("library.sqlite3");
    let mut index = Index::open(&database).unwrap();
    let album = store_album(&mut index, &root, "Album", cover.clone());
    let state = cx.new(|_| AppState::new(index, root).unwrap());
    let cache_directory = state.read_with(cx, |state, _| {
        state.cover_thumbnail_directory().unwrap().to_path_buf()
    });
    std::fs::write(cache_directory, b"not a writable directory").unwrap();
    state.update(cx, |state, cx| {
        state.select_album(Some(album), cx);
        state.request_cover_thumbnail(cover.clone(), cx);
    });
    cx.condition(&state, |state, _| {
        matches!(
            state.cover_thumbnail(&cover),
            Some(CoverStatus::Unavailable)
        )
    })
    .await;
    state.read_with(cx, |state, _| {
        assert!(state.cover_thumbnail_path(&cover).is_none());
        assert_eq!(state.albums().len(), 1);
        assert_eq!(state.album_detail().unwrap().cover, Some(cover.clone()));
        assert!(matches!(state.cover_thumbnail_state(&cover), Some(voon_app::view_model::cover_thumbnails::ThumbnailState::Failed(message)) if message.contains("本地封面目录")));
    });
}

#[gpui::test]
async fn decoded_image_eviction_keeps_requested_thumbnails_available_after_reopen(
    cx: &mut TestAppContext,
) {
    use voon_app::view_model::cover_thumbnails::ThumbnailState;
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("music");
    std::fs::create_dir(&root).unwrap();
    let mut covers = Vec::new();
    let mut albums = Vec::new();
    for number in 0..260 {
        let source = root.join(format!("{number}.png"));
        std::fs::write(&source, png(4, 2, [200, 40, 10, 255])).unwrap();
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
        for cover in &covers {
            state.request_cover_thumbnail(cover.clone(), cx);
        }
    });
    cx.condition(&state, |state, _| {
        covers
            .iter()
            .all(|cover| state.cover_thumbnail_state(cover) == Some(&ThumbnailState::Ready))
    })
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
    drop(state);
    std::fs::rename(&root, directory.path().join("unmounted")).unwrap();
    let reopened = cx.new(|_| AppState::new(Index::open(&database).unwrap(), root).unwrap());
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
