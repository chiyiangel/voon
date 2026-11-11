use gpui::{AppContext, TestAppContext};
use voon_app::view_model::{AppState, Navigation};
use voon_core::{
    db::Index,
    scan::{NoTags, scan_root},
};

#[gpui::test]
fn expanding_and_selecting_a_collection_scopes_the_shared_browse_result(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    for relative in [
        "系列/套装/第一张/01.flac",
        "系列/套装/第二张/01.flac",
        "系列/独立专辑/01.flac",
        "其他专辑/01.flac",
    ] {
        let path = library.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"fixture").unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    let state = cx.new(|_| AppState::new(index, library.path().into()).unwrap());
    let series = library.path().join("系列");
    let boxed = series.join("套装");

    state.read_with(cx, |state, _| assert_eq!(state.collection_rows().len(), 1));
    state.update(cx, |state, cx| state.toggle_collection(&series, cx));
    state.read_with(cx, |state, _| {
        let rows = state.collection_rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].depth, 1);
        assert_eq!(rows[1].album_count, 2);
    });
    state.update(cx, |state, cx| {
        state.select_collection(Some(series.clone()), cx).unwrap()
    });
    state.read_with(cx, |state, _| assert_eq!(state.browsed_albums().len(), 3));
    state.update(cx, |state, cx| {
        state.select_collection(Some(boxed), cx).unwrap()
    });
    state.read_with(cx, |state, _| {
        assert_eq!(state.browsed_albums().len(), 2);
        assert!(
            state
                .browsed_albums()
                .iter()
                .all(|album| album.path.starts_with(series.join("套装")))
        );
    });
    state.update(cx, |state, cx| state.navigate(Navigation::AllAlbums, cx));
    state.read_with(cx, |state, _| assert_eq!(state.browsed_albums().len(), 4));
}
