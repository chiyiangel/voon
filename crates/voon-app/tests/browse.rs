use gpui::{AppContext, TestAppContext};
use voon_app::view_model::{
    AppState, FilterState, SortOrder,
    browse::{SortDirection, ViewMode},
};
use voon_core::{
    Album, AlbumMetadata, AudioFormat, AudioProperties, Disc, Field, FieldSource, ScanResult,
    Track, TrackSource, db::Index,
};

fn album(folder: &str, title: &str, artist: &str, composer: &str, catalog: &str) -> Album {
    Album {
        source_directories: vec![],
        path: format!("/假音乐库/{folder}").into(),
        folder_name: folder.into(),
        metadata: AlbumMetadata {
            title: Some(Field::new(title.into(), FieldSource::Tag)),
            album_artist: Some(Field::new(artist.into(), FieldSource::Tag)),
            composer: Some(Field::new(composer.into(), FieldSource::Tag)),
            catalog_number: Some(Field::new(catalog.into(), FieldSource::Tag)),
            ..AlbumMetadata::default()
        },
        discs: vec![],
        cover: None,
        collection: None,
        signature: String::new(),
    }
}

fn index() -> Index {
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(
            std::path::Path::new("/假音乐库"),
            &ScanResult {
                albums: vec![
                    album("Deng", "淡淡幽情", "邓丽君", "梁弘志", "TACL-001"),
                    album("Miyuki", "寒水魚", "中島みゆき", "中島みゆき", "PCCA-002"),
                    album(
                        "Miles",
                        "Kind of Blue",
                        "Miles Davis",
                        "Bill Evans",
                        "CS-8163",
                    ),
                ],
                ..ScanResult::default()
            },
        )
        .unwrap();
    index
}

#[gpui::test]
fn switching_view_preserves_search_filters_and_the_selected_album(cx: &mut TestAppContext) {
    let state = cx.new(|_| AppState::new(index(), "/假音乐库".into()).unwrap());
    state.update(cx, |state, cx| {
        state.select_album(Some("/假音乐库/Deng".into()), cx);
        state.set_search("淡淡".into(), cx);
        state.set_filters(FilterState::default(), cx);
        state.set_view_mode(ViewMode::List, cx);
    });
    state.read_with(cx, |state, _| {
        assert_eq!(state.view_mode(), ViewMode::List);
        assert_eq!(state.search(), "淡淡");
        assert_eq!(
            state
                .browsed_albums()
                .iter()
                .map(|album| album.display_title())
                .collect::<Vec<_>>(),
            ["淡淡幽情"]
        );
        assert_eq!(state.selected_album().unwrap().display_title(), "淡淡幽情");
    });
    state.update(cx, |state, cx| state.set_view_mode(ViewMode::Grid, cx));
    state.read_with(cx, |state, _| {
        assert_eq!(state.selected_album().unwrap().display_title(), "淡淡幽情")
    });
}

#[gpui::test]
fn clicking_a_column_sorts_albums_and_clicking_it_again_reverses_the_order(
    cx: &mut TestAppContext,
) {
    let state = cx.new(|_| AppState::new(index(), "/假音乐库".into()).unwrap());
    state.update(cx, |state, cx| state.toggle_sort(SortOrder::Title, cx));
    state.read_with(cx, |state, _| {
        assert_eq!(state.sort_direction(), SortDirection::Ascending);
        assert_eq!(
            state
                .browsed_albums()
                .iter()
                .map(|album| album.display_title())
                .collect::<Vec<_>>(),
            ["Kind of Blue", "寒水魚", "淡淡幽情"]
        );
    });
    state.update(cx, |state, cx| state.toggle_sort(SortOrder::Title, cx));
    state.read_with(cx, |state, _| {
        assert_eq!(state.sort_direction(), SortDirection::Descending);
        assert_eq!(
            state
                .browsed_albums()
                .iter()
                .map(|album| album.display_title())
                .collect::<Vec<_>>(),
            ["淡淡幽情", "寒水魚", "Kind of Blue"]
        );
    });
}

#[gpui::test]
fn search_matches_title_artist_composer_and_catalog_in_chinese_japanese_and_english(
    cx: &mut TestAppContext,
) {
    let state = cx.new(|_| AppState::new(index(), "/假音乐库".into()).unwrap());
    for (query, expected) in [
        ("淡淡", "淡淡幽情"),
        ("中島", "寒水魚"),
        ("寒水", "寒水魚"),
        ("MILES", "Kind of Blue"),
        ("bill EVANS", "Kind of Blue"),
        (" pcca-002 ", "寒水魚"),
    ] {
        state.update(cx, |state, cx| state.set_search(query.into(), cx));
        state.read_with(cx, |state, _| {
            assert_eq!(
                state
                    .browsed_albums()
                    .iter()
                    .map(|album| album.display_title())
                    .collect::<Vec<_>>(),
                [expected]
            )
        });
    }
    state.update(cx, |state, cx| state.set_search("找不到的专辑".into(), cx));
    state.read_with(cx, |state, _| assert!(state.browsed_albums().is_empty()));
}

#[gpui::test]
fn all_eight_list_columns_sort_using_album_values(cx: &mut TestAppContext) {
    let mut albums = vec![
        album("Alpha", "Alpha", "Zed", "Y", "A"),
        album("Bravo", "Bravo", "Alice", "Z", "B"),
        album("Charlie", "Charlie", "Mike", "A", "C"),
    ];
    for (position, (year, format, rate, bits, discs, tracks, collection)) in [
        (2001, AudioFormat::WavPack, 96_000, 24, 1, 3, "Z"),
        (1990, AudioFormat::Ape, 44_100, 16, 2, 1, "A"),
        (2010, AudioFormat::Flac, 192_000, 24, 1, 1, "M"),
    ]
    .into_iter()
    .enumerate()
    {
        let album = &mut albums[position];
        album.metadata.year = Some(Field::new(year, FieldSource::Tag));
        album.collection = Some(format!("/假音乐库/{collection}").into());
        album.discs = (1..=discs)
            .map(|number| Disc {
                number,
                path: album.path.join(format!("CD{number}")),
                tracks: (1..=tracks)
                    .map(|track| Track {
                        number: Some(track),
                        title: None,
                        artist: None,
                        format,
                        source: TrackSource::File {
                            path: album.path.join(format!("CD{number}/{track}.flac")),
                        },
                        properties: AudioProperties {
                            sample_rate: Some(rate),
                            bit_depth: Some(bits),
                            ..AudioProperties::default()
                        },
                    })
                    .collect(),
            })
            .collect();
    }
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(
            std::path::Path::new("/假音乐库"),
            &ScanResult {
                albums,
                ..ScanResult::default()
            },
        )
        .unwrap();
    let state = cx.new(|_| AppState::new(index, "/假音乐库".into()).unwrap());
    for (column, expected) in [
        (SortOrder::Title, ["Alpha", "Bravo", "Charlie"]),
        (SortOrder::Artist, ["Bravo", "Charlie", "Alpha"]),
        (SortOrder::Composer, ["Charlie", "Alpha", "Bravo"]),
        (SortOrder::Year, ["Bravo", "Alpha", "Charlie"]),
        (SortOrder::Format, ["Bravo", "Charlie", "Alpha"]),
        (SortOrder::Resolution, ["Bravo", "Alpha", "Charlie"]),
        (SortOrder::DiscTracks, ["Charlie", "Alpha", "Bravo"]),
        (SortOrder::Collection, ["Bravo", "Charlie", "Alpha"]),
    ] {
        state.update(cx, |state, cx| state.toggle_sort(column, cx));
        state.read_with(cx, |state, _| {
            assert_eq!(
                state
                    .browsed_albums()
                    .iter()
                    .map(|album| album.display_title())
                    .collect::<Vec<_>>(),
                expected,
                "{column:?}"
            )
        });
        state.update(cx, |state, cx| state.toggle_sort(column, cx));
        state.read_with(cx, |state, _| {
            assert_eq!(
                state
                    .browsed_albums()
                    .iter()
                    .map(|album| album.display_title())
                    .collect::<Vec<_>>(),
                expected.into_iter().rev().collect::<Vec<_>>(),
                "{column:?} descending"
            )
        });
    }
}
