use super::*;
use gpui::{AppContext, TestAppContext, VisualTestContext};
use voon_core::{AlbumMetadata, AudioFormat, AudioProperties, Disc, ScanResult, db::Index};

fn fixture(path: &str) -> Album {
    Album {
        path: path.into(),
        source_directories: vec![],
        candidates: vec![],
        folder_name: path.into(),
        cover: None,
        collection: None,
        signature: path.into(),
        metadata: AlbumMetadata::default(),
        discs: (1..=2)
            .map(|number| Disc {
                number,
                path: path.into(),
                tracks: vec![Track {
                    number: Some(1),
                    title: None,
                    artist: None,
                    format: AudioFormat::Flac,
                    source: TrackSource::File {
                        path: PathBuf::from(path).join(format!("{number}.flac")),
                    },
                    properties: AudioProperties {
                        sample_rate: Some(44100),
                        bit_depth: Some(16),
                        channels: Some(2),
                        duration_ms: Some(180000),
                    },
                }],
            })
            .collect(),
    }
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    cx.run_until_parked();
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("missing control: {selector}"));
    cx.simulate_click(bounds.center(), Default::default());
    cx.run_until_parked();
}

#[gpui::test]
fn pages_disclose_details_and_switching_album_clears_expansions(cx: &mut TestAppContext) {
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(
            std::path::Path::new("/music"),
            &ScanResult {
                albums: vec![fixture("/music/first"), fixture("/music/second")],
                ..Default::default()
            },
        )
        .unwrap();
    let state = cx.new(|_| AppState::new(index, "/music".into()).unwrap());
    state.update(cx, |state, cx| {
        state.select_album(Some("/music/first".into()), cx)
    });
    let covers = cx.new(|cx| CoverCache::new(&state, cx));
    let (detail, cx) = cx.add_window_view(|_, cx| AlbumDetail::new(state.clone(), covers, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("toggle-album-metadata").is_some());
    assert!(cx.debug_bounds("detail-track-0-0").is_none());
    assert!(cx.debug_bounds("detail-attachments").is_none());
    click(cx, "toggle-album-metadata");
    click(cx, "toggle-metadata-sources");
    click(cx, "detail-tracks");
    assert!(cx.debug_bounds("detail-track-0-0").is_some());
    assert!(cx.debug_bounds("detail-track-1-0").is_none());
    assert!(cx.debug_bounds("open-track-player-0-0").is_none());
    click(cx, "detail-track-0-0");
    assert!(cx.debug_bounds("open-track-player-0-0").is_some());
    click(cx, "detail-disc-1");
    assert!(cx.debug_bounds("detail-track-1-0").is_some());
    click(cx, "detail-files");
    assert!(cx.debug_bounds("detail-attachments").is_some());
    assert!(cx.debug_bounds("detail-force-tags").is_none());
    click(cx, "toggle-reading-status");
    assert!(cx.debug_bounds("detail-force-tags").is_some());
    click(cx, "detail-organization");
    detail.read_with(cx, |detail, _| {
        assert!(
            detail.page == DetailPage::Organization,
            "organization click did not switch page"
        )
    });
    // Page navigation preserves the selected album and its open track.
    click(cx, "detail-tracks");
    assert!(cx.debug_bounds("open-track-player-0-0").is_some());
    state.read_with(cx, |state, _| {
        assert_eq!(
            state.album_detail().unwrap().path,
            PathBuf::from("/music/first")
        )
    });
    state.update(cx, |state, cx| {
        state.select_album(Some("/music/second".into()), cx)
    });
    cx.run_until_parked();
    detail.read_with(cx, |detail, _| {
        assert!(detail.page == DetailPage::Tracks);
        assert!(!detail.sources_open && !detail.metadata_open && !detail.reading_open);
        assert!(detail.open_tracks.is_empty());
        assert_eq!(detail.open_discs, HashSet::from([0]));
    });
    // Editing remains available on every page.
    click(cx, "edit-album-information");
    state.read_with(cx, |state, _| assert!(state.album_edit_draft().is_some()));
}

#[test]
fn audio_summary_does_not_misrepresent_mixed_or_empty_albums() {
    let mut album = fixture("/music/album");
    assert_eq!(audio_summary(&album), " · FLAC · 16 bit / 44.1 kHz");
    album.discs[1].tracks[0].properties.sample_rate = Some(96000);
    assert_eq!(audio_summary(&album), " · 多种音频规格");
    album.discs.clear();
    assert_eq!(audio_summary(&album), "");
}
