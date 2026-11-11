use gpui::{AppContext, TestAppContext};
use voon_app::view_model::{AppState, Navigation};
use voon_core::{db::Index, *};

#[gpui::test]
fn composer_names_are_sorted_and_selection_filters_the_shared_album_result(
    cx: &mut TestAppContext,
) {
    let mut index = Index::open_in_memory().unwrap();
    let albums = [
        ("第一张", "莫扎特"),
        ("第二张", "巴赫"),
        ("第三张", "莫扎特"),
    ]
    .into_iter()
    .map(|(name, composer)| Album {
        source_directories: vec![],
        candidates: vec![],
        path: format!("/假库/{name}").into(),
        folder_name: name.into(),
        metadata: AlbumMetadata {
            composer: Some(Field::new(composer.into(), FieldSource::Tag)),
            ..AlbumMetadata::default()
        },
        discs: vec![],
        cover: None,
        collection: None,
        signature: name.into(),
    })
    .collect();
    index
        .store_scan(
            std::path::Path::new("/假库"),
            &ScanResult {
                albums,
                ..ScanResult::default()
            },
        )
        .unwrap();
    let state = cx.new(|_| AppState::new(index, "/建议目录".into()).unwrap());
    state.read_with(cx, |state, _| {
        let composers = state.composer_options();
        assert_eq!(
            composers
                .iter()
                .map(|option| option.name.as_str())
                .collect::<Vec<_>>(),
            ["巴赫", "莫扎特"]
        );
        assert_eq!(composers[1].count, 2);
    });
    state.update(cx, |state, cx| {
        state.select_composer(Some("莫扎特".into()), cx).unwrap()
    });
    state.read_with(cx, |state, _| {
        assert_eq!(state.navigation(), Navigation::Composers);
        assert_eq!(state.browsed_albums().len(), 2);
        assert!(
            state.browsed_albums().iter().all(|album| album
                .metadata
                .composer
                .as_ref()
                .unwrap()
                .value
                == "莫扎特")
        );
    });
    state.update(cx, |state, cx| state.navigate(Navigation::AllAlbums, cx));
    state.read_with(cx, |state, _| assert_eq!(state.browsed_albums().len(), 3));
}
