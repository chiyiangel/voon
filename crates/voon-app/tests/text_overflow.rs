use gpui::{AppContext, TestAppContext, VisualTestContext, px, size};
use voon_app::{motion, ui::VoonWindow, view_model::AppState};
use voon_core::{
    db::Index,
    scan::{NoTags, scan_root},
};

const LONG_COLLECTION: &str =
    "Anne-Sophie_Mutter_Herbert_Von_Karajan_Vienna_Philharmonic_Orchestra";
const MOZART_TITLE: &str = "Complete Mozart. The Piano Concertos";
const BEETHOVEN_TITLE: &str = "Herbert von Karajan – Beethoven Symphonies";
const SIDEBAR_WIDTH: f32 = 220.0;

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> gpui::Bounds<gpui::Pixels> {
    cx.run_until_parked();
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("missing bounds: {selector}"))
}

fn right(value: gpui::Bounds<gpui::Pixels>) -> f32 {
    f32::from(value.right())
}

fn left(value: gpui::Bounds<gpui::Pixels>) -> f32 {
    f32::from(value.left())
}

fn width(value: gpui::Bounds<gpui::Pixels>) -> f32 {
    f32::from(value.size.width)
}

fn assert_long_names_stay_inside_cells(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    for album in [MOZART_TITLE, BEETHOVEN_TITLE] {
        let path = library
            .path()
            .join(LONG_COLLECTION)
            .join(album)
            .join("01.flac");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"fixture").unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    let state = cx.new(|_| AppState::new(index, library.path().into()).unwrap());
    state.read_with(cx, |state, _| {
        assert!(
            state
                .collection_rows()
                .iter()
                .any(|row| row.name == LONG_COLLECTION),
            "fixture collection should appear in the sidebar"
        );
        let titles: Vec<_> = state
            .browsed_albums()
            .iter()
            .map(|album| album.display_title().to_string())
            .collect();
        assert!(
            titles.iter().any(|title| title == MOZART_TITLE),
            "{titles:?}"
        );
        assert!(
            titles.iter().any(|title| title == BEETHOVEN_TITLE),
            "{titles:?}"
        );
    });

    let (_, cx) = cx.add_window_view(|_, cx| VoonWindow::new(state, cx));
    // 960px leaves a two-column grid after the 220 + 340 side columns.
    cx.simulate_resize(size(px(960.), px(800.)));
    cx.run_until_parked();

    let sidebar = bounds(cx, "library-sidebar");
    assert!(
        (width(sidebar) - SIDEBAR_WIDTH).abs() < 2.0,
        "sidebar width was {}, expected {SIDEBAR_WIDTH}",
        width(sidebar)
    );

    let collection = bounds(
        cx,
        "collection-label:Anne-Sophie_Mutter_Herbert_Von_Karajan_Vienna_Philharmonic_Orchestra",
    );
    assert!(
        right(collection) <= right(sidebar) + 0.5,
        "collection label right {} crossed sidebar right {}",
        right(collection),
        right(sidebar)
    );

    let mozart_card = bounds(cx, "album-card:Complete Mozart. The Piano Concertos");
    let beethoven_card = bounds(cx, "album-card:Herbert von Karajan – Beethoven Symphonies");
    let mozart_title = bounds(cx, "album-title:Complete Mozart. The Piano Concertos");
    let beethoven_title = bounds(cx, "album-title:Herbert von Karajan – Beethoven Symphonies");
    let mozart_artist = bounds(cx, "album-artist:Complete Mozart. The Piano Concertos");
    let beethoven_artist = bounds(
        cx,
        "album-artist:Herbert von Karajan – Beethoven Symphonies",
    );
    // "未知艺术家" must remain visible; a collapsed truncate pass would render only an ellipsis.
    for (name, artist) in [("Mozart", mozart_artist), ("Beethoven", beethoven_artist)] {
        assert!(
            width(artist) > 40.0,
            "{name} artist width {} looks collapsed to an ellipsis",
            width(artist)
        );
        assert!(
            right(artist)
                <= right(if name == "Mozart" {
                    mozart_card
                } else {
                    beethoven_card
                }) + 0.5,
            "{name} artist crossed its card"
        );
    }

    assert!(
        right(mozart_title) <= right(mozart_card) + 0.5,
        "Mozart title right {} crossed its card right {}",
        right(mozart_title),
        right(mozart_card)
    );
    assert!(
        right(beethoven_title) <= right(beethoven_card) + 0.5,
        "Beethoven title right {} crossed its card right {}",
        right(beethoven_title),
        right(beethoven_card)
    );

    let (left_card, left_title, right_card, right_title) =
        if left(mozart_card) <= left(beethoven_card) {
            (mozart_card, mozart_title, beethoven_card, beethoven_title)
        } else {
            (beethoven_card, beethoven_title, mozart_card, mozart_title)
        };
    assert!(
        left(right_card) - left(left_card) > 40.0,
        "expected two side-by-side cards, left={} right={}",
        left(left_card),
        left(right_card)
    );
    assert!(
        right(left_title) <= left(right_card) + 0.5,
        "left card title right {} entered the adjacent card at {}",
        right(left_title),
        left(right_card)
    );
    assert!(
        left(right_title) >= right(left_card) - 0.5,
        "right card title left {} entered the previous card ending at {}",
        left(right_title),
        right(left_card)
    );
}

#[gpui::test]
fn long_names_stay_inside_the_sidebar_and_album_cards(cx: &mut TestAppContext) {
    let _guard = motion::force_reduce_motion(true);
    assert_long_names_stay_inside_cells(cx);
}

#[gpui::test]
fn long_names_stay_inside_cells_while_motion_is_enabled(cx: &mut TestAppContext) {
    let _guard = motion::force_reduce_motion(false);
    assert_long_names_stay_inside_cells(cx);
}
