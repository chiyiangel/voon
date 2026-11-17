//! Overflow *and* under-truncation checks for single-line labels.
//!
//! The first overflow tests only asserted that label *boxes* stayed inside the
//! sidebar or card. A flex item can still be the right width while
//! `text_ellipsis` keeps only a few glyphs, which is what happened after
//! 3d542b9 on macOS (about three letters and a large gap before the count).
//! These tests therefore also inspect the displayed string and how much of the
//! remaining width the shaped text actually occupies.

use gpui::{AppContext, Render, TestAppContext, VisualTestContext, div, prelude::*, px, size};
use voon_app::{
    motion,
    ui::{VoonWindow, capture_labels, ellipsize_to_width, snapshots, truncating_label},
    view_model::AppState,
};
use voon_core::{
    db::Index,
    scan::{NoTags, scan_root},
};

const LONG_COLLECTION: &str =
    "Anne-Sophie_Mutter_Herbert_Von_Karajan_Vienna_Philharmonic_Orchestra";
const SHORT_COLLECTION: &str = "Alf";
const CJK_SHORT: &str = "邓丽君淡淡幽情";
const CJK_LONG: &str = "安妮索菲穆特与赫伯特冯卡拉扬维也纳爱乐乐团音乐会全记录";
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

fn latest_snapshot(source: &str) -> voon_app::ui::LabelSnapshot {
    snapshots()
        .into_iter()
        .rev()
        .find(|snapshot| snapshot.source == source)
        .unwrap_or_else(|| {
            panic!(
                "missing label snapshot for {source:?}; got {snapshots:#?}",
                snapshots = snapshots()
            )
        })
}

fn assert_fills_remaining_width(
    source: &str,
    snapshot: &voon_app::ui::LabelSnapshot,
    min_available: f32,
) {
    assert!(
        snapshot.available > min_available,
        "{source} available width {} is smaller than the leftover flex space ({min_available})",
        snapshot.available
    );
    let slack = 12.5_f32;
    assert!(
        snapshot.available - snapshot.text_width <= slack,
        "{source} used {}px of {}px leftover; glyphs should reach the end before ellipsizing",
        snapshot.text_width,
        snapshot.available
    );
    assert!(
        snapshot.displayed.ends_with('…'),
        "{source} should ellipsize at the end, got {:?}",
        snapshot.displayed
    );
    let visible = snapshot.displayed.trim_end_matches('…');
    assert!(
        source.starts_with(visible),
        "{source} should keep a prefix, got {:?}",
        snapshot.displayed
    );
    assert!(
        visible.chars().count() > 3,
        "{source} was over-truncated to {:?}",
        snapshot.displayed
    );
}

fn assert_short_name_complete(source: &str, snapshot: &voon_app::ui::LabelSnapshot) {
    assert_eq!(
        snapshot.displayed, source,
        "short name {source} should be shown in full"
    );
    assert!(
        !snapshot.displayed.contains('…'),
        "short name {source} should not be ellipsized"
    );
    assert!(
        snapshot.text_width <= snapshot.available + 0.5,
        "short name {source} overflowed its box: text {} available {}",
        snapshot.text_width,
        snapshot.available
    );
}

#[test]
fn ellipsize_uses_measured_width_not_a_fixed_character_count() {
    let measure = |text: &str| px(text.chars().count() as f32 * 10.0);
    assert_eq!(ellipsize_to_width("Alf", px(100.), measure), "Alf");
    assert_eq!(ellipsize_to_width("ABCDEFGHIJ", px(50.), measure), "ABCD…");
    assert_eq!(
        ellipsize_to_width("ABCDEFGHIJ", px(80.), measure),
        "ABCDEFG…"
    );
    assert_eq!(
        ellipsize_to_width("邓丽君淡淡幽情专辑名", px(50.), measure),
        "邓丽君淡…"
    );
    assert_eq!(
        ellipsize_to_width("邓丽君淡淡幽情专辑名", px(90.), measure),
        "邓丽君淡淡幽情专…"
    );
}

struct LabelRow {
    name: String,
    sidebar: f32,
    depth: u8,
    count: u32,
    font_size: f32,
}

impl Render for LabelRow {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        div()
            .id("probe-sidebar")
            .debug_selector(|| "probe-sidebar".into())
            .w(px(self.sidebar))
            .max_w(px(self.sidebar))
            .min_w_0()
            .overflow_hidden()
            .px(px(10.))
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .h(px(26.))
                    .pl(px(8. + self.depth as f32 * 14.))
                    .pr(px(8.))
                    .child(div().id("expand").w(px(12.)).flex_none().child("›"))
                    .child(div().id("icon").w(px(13.)).flex_none().child("□"))
                    .child(
                        div()
                            .id("probe-label")
                            .debug_selector(|| "probe-label".into())
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_size(px(self.font_size))
                            .child(truncating_label(self.name.clone())),
                    )
                    .child(
                        div()
                            .id("probe-count")
                            .debug_selector(|| "probe-count".into())
                            .flex_none()
                            .text_size(px(11.5))
                            .child(self.count.to_string()),
                    ),
            )
    }
}

struct LaidOutRow {
    snapshot: voon_app::ui::LabelSnapshot,
    label: gpui::Bounds<gpui::Pixels>,
    count: gpui::Bounds<gpui::Pixels>,
}

fn layout_row(
    cx: &mut TestAppContext,
    name: &str,
    sidebar: f32,
    depth: u8,
    count: u32,
) -> LaidOutRow {
    let _capture = capture_labels();
    let (_view, cx) = cx.add_window_view(|_, _cx| LabelRow {
        name: name.to_string(),
        sidebar,
        depth,
        count,
        font_size: 12.5,
    });
    cx.simulate_resize(size(px(960.), px(400.)));
    cx.run_until_parked();
    let label = bounds(cx, "probe-label");
    let count_bounds = bounds(cx, "probe-count");
    LaidOutRow {
        snapshot: latest_snapshot(name),
        label,
        count: count_bounds,
    }
}

fn assert_row_uses_space_between_icon_and_count(row: &LaidOutRow, source: &str) {
    let gap = left(row.count) - right(row.label);
    assert!(
        (gap - 5.0).abs() < 1.5,
        "{source} should leave only the flex gap before the count, gap={gap}"
    );
    assert!(
        (width(row.label) - row.snapshot.available).abs() < 1.5,
        "{source} label box {} should match the width used for shaping {}",
        width(row.label),
        row.snapshot.available
    );
}

#[gpui::test]
fn truncating_label_fills_remaining_width_across_containers(cx: &mut TestAppContext) {
    let short_en = layout_row(cx, SHORT_COLLECTION, 220.0, 0, 1);
    assert_row_uses_space_between_icon_and_count(&short_en, SHORT_COLLECTION);
    assert_short_name_complete(SHORT_COLLECTION, &short_en.snapshot);

    let short_cjk = layout_row(cx, CJK_SHORT, 220.0, 0, 5);
    assert_row_uses_space_between_icon_and_count(&short_cjk, CJK_SHORT);
    assert_short_name_complete(CJK_SHORT, &short_cjk.snapshot);

    let narrow = layout_row(cx, LONG_COLLECTION, 160.0, 0, 2);
    let mid = layout_row(cx, LONG_COLLECTION, 220.0, 0, 2);
    let wide = layout_row(cx, LONG_COLLECTION, 300.0, 0, 2);
    for (label, row, min_available) in [
        ("160", &narrow, 60.0),
        ("220", &mid, 100.0),
        ("300", &wide, 160.0),
    ] {
        assert_row_uses_space_between_icon_and_count(row, &format!("{LONG_COLLECTION} at {label}"));
        assert_fills_remaining_width(LONG_COLLECTION, &row.snapshot, min_available);
    }
    assert!(
        mid.snapshot.available > narrow.snapshot.available,
        "a wider sidebar should give the name more leftover width"
    );
    assert!(
        wide.snapshot.available > mid.snapshot.available,
        "a 300px container should leave more width than 220px"
    );
    assert!(
        wide.snapshot.displayed.chars().count() > narrow.snapshot.displayed.chars().count(),
        "wider leftover space should keep more of the name, narrow={:?} wide={:?}",
        narrow.snapshot.displayed,
        wide.snapshot.displayed
    );

    let indented = layout_row(cx, LONG_COLLECTION, 220.0, 2, 2);
    assert_row_uses_space_between_icon_and_count(&indented, "indented long name");
    assert_fills_remaining_width(LONG_COLLECTION, &indented.snapshot, 80.0);
    assert!(
        indented.snapshot.available < mid.snapshot.available,
        "depth indent should reduce leftover width, depth0={} depth2={}",
        mid.snapshot.available,
        indented.snapshot.available
    );

    let one_digit = layout_row(cx, LONG_COLLECTION, 220.0, 0, 1);
    let three_digits = layout_row(cx, LONG_COLLECTION, 220.0, 0, 100);
    assert_fills_remaining_width(LONG_COLLECTION, &one_digit.snapshot, 100.0);
    assert_fills_remaining_width(LONG_COLLECTION, &three_digits.snapshot, 90.0);
    assert!(
        three_digits.snapshot.available < one_digit.snapshot.available,
        "a wider count should shrink the leftover name width, 1={} 100={}",
        one_digit.snapshot.available,
        three_digits.snapshot.available
    );
    assert!(
        width(three_digits.count) > width(one_digit.count),
        "three-digit counts should be wider than one-digit counts"
    );

    let long_cjk = layout_row(cx, CJK_LONG, 220.0, 0, 15);
    assert_row_uses_space_between_icon_and_count(&long_cjk, CJK_LONG);
    assert_fills_remaining_width(CJK_LONG, &long_cjk.snapshot, 90.0);
}

fn seed_library() -> (tempfile::TempDir, Index) {
    let library = tempfile::tempdir().unwrap();
    for album in [MOZART_TITLE, BEETHOVEN_TITLE] {
        let path = library
            .path()
            .join(LONG_COLLECTION)
            .join(album)
            .join("01.flac");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"fixture").unwrap();
    }
    for (collection, album) in [(SHORT_COLLECTION, "Album One"), (CJK_SHORT, "淡淡幽情")] {
        let path = library.path().join(collection).join(album).join("01.flac");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"fixture").unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    (library, index)
}

fn assert_long_names_stay_inside_cells_and_fill_remaining_width(cx: &mut TestAppContext) {
    let (library, index) = seed_library();
    let state = cx.new(|_| AppState::new(index, library.path().into()).unwrap());
    state.read_with(cx, |state, _| {
        let names: Vec<_> = state
            .collection_rows()
            .iter()
            .map(|row| row.name.clone())
            .collect();
        for expected in [LONG_COLLECTION, SHORT_COLLECTION, CJK_SHORT] {
            assert!(
                names.iter().any(|name| name == expected),
                "fixture collection {expected} missing from {names:?}"
            );
        }
    });

    let _capture = capture_labels();
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

    let long_name = latest_snapshot(LONG_COLLECTION);
    assert_fills_remaining_width(LONG_COLLECTION, &long_name, 100.0);
    assert_short_name_complete(SHORT_COLLECTION, &latest_snapshot(SHORT_COLLECTION));
    assert_short_name_complete(CJK_SHORT, &latest_snapshot(CJK_SHORT));
    let artist = latest_snapshot("未知艺术家");
    assert_short_name_complete("未知艺术家", &artist);
    assert!(
        artist.available > 40.0,
        "artist leftover width {} looks collapsed",
        artist.available
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
    for (name, artist_bounds) in [("Mozart", mozart_artist), ("Beethoven", beethoven_artist)] {
        assert!(
            width(artist_bounds) > 40.0,
            "{name} artist width {} looks collapsed to an ellipsis",
            width(artist_bounds)
        );
        assert!(
            right(artist_bounds)
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
    assert_long_names_stay_inside_cells_and_fill_remaining_width(cx);
}

#[gpui::test]
fn long_names_stay_inside_cells_while_motion_is_enabled(cx: &mut TestAppContext) {
    let _guard = motion::force_reduce_motion(false);
    assert_long_names_stay_inside_cells_and_fill_remaining_width(cx);
}
