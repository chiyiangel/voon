use gpui::{AppContext, TestAppContext};
use voon_app::view_model::{AppState, facets::FacetValue};
use voon_core::{
    db::Index,
    facets::{Encoding, Language, Resolution},
    *,
};

fn fixture() -> Index {
    let rows = [
        ("甲", AudioFormat::Flac, 96_000, 24, 1983, "古典"),
        ("乙", AudioFormat::Flac, 44_100, 16, 1988, "流行"),
        ("あ", AudioFormat::Dsf, 2_822_400, 1, 2001, "古典"),
        ("English", AudioFormat::Ape, 192_000, 24, 1999, "古典"),
    ];
    let mut index = Index::open_in_memory().unwrap();
    let albums = rows
        .into_iter()
        .map(|(title, format, rate, bits, year, genre)| {
            let path = std::path::PathBuf::from(format!("/假库/{title}"));
            Album {
                source_directories: vec![],
                candidates: vec![],
                path: path.clone(),
                folder_name: title.into(),
                metadata: AlbumMetadata {
                    title: Some(Field::new(title.into(), FieldSource::Tag)),
                    year: Some(Field::new(year, FieldSource::Tag)),
                    genre: Some(Field::new(genre.into(), FieldSource::Tag)),
                    ..AlbumMetadata::default()
                },
                discs: vec![Disc {
                    number: 1,
                    path: path.clone(),
                    tracks: vec![Track {
                        number: Some(1),
                        title: None,
                        artist: None,
                        format,
                        source: TrackSource::File {
                            path: path.join("01.audio"),
                        },
                        properties: AudioProperties {
                            sample_rate: Some(rate),
                            bit_depth: Some(bits),
                            ..AudioProperties::default()
                        },
                    }],
                }],
                cover: None,
                collection: None,
                signature: title.into(),
            }
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
    index
}

#[gpui::test]
fn facets_intersect_across_dimensions_and_counts_exclude_their_own_dimension(
    cx: &mut TestAppContext,
) {
    let state = cx.new(|_| AppState::new(fixture(), "/建议目录".into()).unwrap());
    state.update(cx, |state, cx| {
        state.toggle_facet(FacetValue::Format(AudioFormat::Flac), cx);
        state.toggle_facet(FacetValue::Resolution(Resolution::HiRes), cx);
    });
    state.read_with(cx, |state, _| {
        assert_eq!(state.browsed_albums().len(), 1);
        assert_eq!(state.browsed_albums()[0].display_title(), "甲");
        let count = |value| {
            state
                .facet_options()
                .iter()
                .find(|option| option.value == value)
                .unwrap()
                .count
        };
        assert_eq!(count(FacetValue::Format(AudioFormat::Ape)), 1);
        assert_eq!(count(FacetValue::Resolution(Resolution::Cd)), 1);
        assert_eq!(count(FacetValue::Encoding(Encoding::Dsd)), 0);
    });
    state.update(cx, |state, cx| {
        state.toggle_facet(FacetValue::Format(AudioFormat::Ape), cx);
        assert_eq!(state.browsed_albums().len(), 2);
        state.toggle_facet(FacetValue::Language(Language::Chinese), cx);
        state.toggle_facet(FacetValue::Decade(1980), cx);
        state.toggle_facet(FacetValue::MinSampleRate(96_000), cx);
        state.toggle_facet(FacetValue::MinBitDepth(24), cx);
        state.toggle_facet(FacetValue::Genre("古典".into()), cx);
        assert_eq!(state.browsed_albums().len(), 1);
        state.toggle_facet(FacetValue::Encoding(Encoding::Dsd), cx);
        assert!(state.browsed_albums().is_empty());
        state.clear_facets(cx);
        assert_eq!(state.browsed_albums().len(), 4);
    });
}
