use gpui::{AppContext, Entity, TestAppContext};
use voon_app::view_model::{AppState, FilterState, Navigation, ScanState, SortOrder};
use voon_core::db::Index;
use voon_core::{Album, AlbumMetadata, ScanResult};

/// Use an in-memory Index, so every slice can supply fixtures without touching
/// a Library root or opening a native window.
fn app_state(cx: &mut TestAppContext, index: Index) -> Entity<AppState> {
    cx.new(|_| AppState::new(index, "/建议的音乐目录".into()).expect("构造应用状态"))
}

#[gpui::test]
fn first_launch_is_an_empty_library_without_starting_a_scan(cx: &mut TestAppContext) {
    let state = app_state(cx, Index::open_in_memory().unwrap());

    state.read_with(cx, |state, _| {
        assert!(state.roots().is_empty());
        assert!(state.albums().is_empty());
        assert_eq!(state.navigation(), Navigation::AllAlbums);
        assert_eq!(state.scan_state(), &ScanState::Idle);
        assert!(state.selected_album().is_none());
    });
}

#[gpui::test]
fn every_navigation_destination_can_be_selected(cx: &mut TestAppContext) {
    let state = app_state(cx, Index::open_in_memory().unwrap());

    for destination in [
        Navigation::Collections,
        Navigation::Composers,
        Navigation::Facets,
        Navigation::UserTags,
        Navigation::ManualCollections,
        Navigation::NeedsAttention,
        Navigation::AllAlbums,
    ] {
        state.update(cx, |state, cx| state.navigate(destination, cx));
        state.read_with(cx, |state, _| assert_eq!(state.navigation(), destination));
    }
}

#[gpui::test]
fn an_injected_index_supplies_albums_and_selection_stays_in_the_state(cx: &mut TestAppContext) {
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(
            std::path::Path::new("/假音乐库"),
            &ScanResult {
                albums: vec![Album {
                    source_directories: vec![],
                    candidates: vec![],
                    path: "/假音乐库/夜曲".into(),
                    folder_name: "夜曲".into(),
                    discs: vec![],
                    cover: None,
                    metadata: AlbumMetadata::default(),
                    collection: None,
                    signature: "fixture".into(),
                }],
                ..ScanResult::default()
            },
        )
        .unwrap();
    let state = app_state(cx, index);

    state.update(cx, |state, cx| {
        state.select_album(Some("/假音乐库/夜曲".into()), cx)
    });
    state.read_with(cx, |state, _| {
        assert_eq!(state.roots()[0].path, std::path::Path::new("/假音乐库"));
        assert_eq!(state.albums().len(), 1);
        assert_eq!(state.selected_album().unwrap().display_title(), "夜曲");
    });

    state.update(cx, |state, cx| state.navigate(Navigation::Collections, cx));
    state.read_with(cx, |state, _| assert!(state.selected_album().is_none()));

    state.update(cx, |state, cx| {
        state.select_album(Some("/未知专辑".into()), cx)
    });
    state.read_with(cx, |state, _| assert!(state.selected_album().is_none()));
}

#[gpui::test]
fn browse_controls_are_owned_by_state_across_navigation(cx: &mut TestAppContext) {
    let state = app_state(cx, Index::open_in_memory().unwrap());

    state.update(cx, |state, cx| {
        state.set_search("巴赫".into(), cx);
        state.set_filters(
            FilterState {
                formats: vec![voon_core::AudioFormat::Flac],
                ..FilterState::default()
            },
            cx,
        );
        state.set_sort_order(SortOrder::Title, cx);
        state.navigate(Navigation::Facets, cx);
    });

    state.read_with(cx, |state, _| {
        assert_eq!(state.search(), "巴赫");
        assert_eq!(state.filters().formats, [voon_core::AudioFormat::Flac]);
        assert_eq!(state.sort_order(), SortOrder::Title);
    });
}
