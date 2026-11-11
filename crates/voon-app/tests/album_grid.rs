use gpui::{AppContext, TestAppContext};
use voon_app::view_model::AppState;
use voon_app::view_model::grid::{CoverCache, CoverStatus};
use voon_core::{Album, AlbumMetadata, ScanResult, db::Index};

#[gpui::test]
async fn directory_cover_is_loaded_in_background_and_remains_cached(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("cover.png");
    image::RgbaImage::from_pixel(2, 2, image::Rgba([200, 40, 10, 255]))
        .save(&path)
        .unwrap();
    let cover = voon_core::Cover {
        source: voon_core::CoverSource::File(path.clone()),
    };
    let cache = cx.new(|_| CoverCache::default());
    cache.update(cx, |cache, cx| {
        cache.request(cover.clone(), cx);
    });
    cx.condition(&cache, |cache, _| {
        matches!(cache.status(&cover), Some(CoverStatus::Ready(_)))
    })
    .await;
    std::fs::remove_file(path).unwrap();
    cache.update(cx, |cache, cx| {
        assert!(
            matches!(cache.request(cover.clone(), cx), CoverStatus::Ready(_)),
            "已解码封面应保留在缓存中"
        );
    });
}

#[gpui::test]
async fn library_refresh_retries_failed_covers_and_decodes_updated_images(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("cover.png");
    let cover = voon_core::Cover {
        source: voon_core::CoverSource::File(path.clone()),
    };
    let state = cx
        .new(|_| AppState::new(Index::open_in_memory().unwrap(), directory.path().into()).unwrap());
    let cache = cx.new(|cx| CoverCache::new(&state, cx));
    cache.update(cx, |cache, cx| {
        cache.request(cover.clone(), cx);
    });
    cx.condition(&cache, |cache, _| {
        matches!(cache.status(&cover), Some(CoverStatus::Unavailable))
    })
    .await;

    image::RgbaImage::from_pixel(2, 2, image::Rgba([200, 40, 10, 255]))
        .save(&path)
        .unwrap();
    state.update(cx, |state, cx| state.reload_library(cx).unwrap());
    cx.condition(&cache, |cache, _| cache.status(&cover).is_none())
        .await;
    cache.update(cx, |cache, cx| {
        cache.request(cover.clone(), cx);
    });
    cx.condition(&cache, |cache, _| {
        matches!(cache.status(&cover), Some(CoverStatus::Ready(_)))
    })
    .await;

    // Replacing the same path with undecodable content must retire the Ready
    // result too. This observes fresh decoding without inspecting pixels.
    std::fs::write(path, b"not an image anymore").unwrap();
    state.update(cx, |state, cx| state.reload_library(cx).unwrap());
    cx.condition(&cache, |cache, _| cache.status(&cover).is_none())
        .await;
    cache.update(cx, |cache, cx| {
        cache.request(cover.clone(), cx);
    });
    cx.condition(&cache, |cache, _| {
        matches!(cache.status(&cover), Some(CoverStatus::Unavailable))
    })
    .await;
}

#[gpui::test]
fn grid_exposes_every_indexed_album_in_responsive_rows_and_selects_by_path(
    cx: &mut TestAppContext,
) {
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(
            std::path::Path::new("/假库"),
            &ScanResult {
                albums: (0..1003)
                    .map(|number| Album {
                        source_directories: vec![],
                        candidates: vec![],
                        path: format!("/假库/{number:04}").into(),
                        folder_name: format!("专辑{number:04}"),
                        discs: vec![],
                        cover: None,
                        metadata: AlbumMetadata::default(),
                        collection: None,
                        signature: format!("{number}"),
                    })
                    .collect(),
                ..ScanResult::default()
            },
        )
        .unwrap();
    let state = cx.new(|_| AppState::new(index, "/建议目录".into()).unwrap());
    state.update(cx, |state, cx| {
        state.set_sort_order(voon_app::view_model::SortOrder::Title, cx)
    });

    state.read_with(cx, |state, _| {
        let expected: Vec<std::path::PathBuf> = (0..1003)
            .map(|number| format!("/假库/{number:04}").into())
            .collect();
        for width in [40., 440., 720., 1280.] {
            let layout = state.grid_layout(width);
            let paths: Vec<_> = (0..layout.rows)
                .flat_map(|row| state.grid_row(row, layout))
                .map(|album| album.path.clone())
                .collect();
            assert_eq!(
                paths, expected,
                "改变可用宽度仍可按排序顺序访问每张专辑，且不重不漏"
            );
        }
    });

    state.update(cx, |state, cx| {
        state.select_album(Some("/假库/0500".into()), cx)
    });
    state.read_with(cx, |state, _| {
        assert_eq!(state.selected_album().unwrap().folder_name, "专辑0500")
    });
}
