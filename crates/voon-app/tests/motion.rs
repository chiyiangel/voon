use std::time::Duration;

use gpui::{AppContext, Context, TestAppContext};
use voon_app::motion::{self, Linger};

struct Host {
    live: Option<&'static str>,
    linger: Linger<&'static str>,
}

impl Host {
    fn visible(&self) -> Option<(&'static str, bool)> {
        self.linger
            .present(self.live.as_ref())
            .map(|(value, closing)| (*value, closing))
    }

    fn open(&mut self, value: &'static str) {
        self.linger.cancel();
        self.live = Some(value);
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        let Some(value) = self.live.take() else {
            return;
        };
        if let Some((generation, duration)) = self.linger.begin(value, motion::overlay()) {
            motion::schedule(cx, duration, move |this, cx| {
                this.linger.finish(generation);
                cx.notify();
            });
        }
        cx.notify();
    }
}

#[gpui::test]
fn reduced_motion_makes_every_token_zero(_cx: &mut TestAppContext) {
    let _guard = motion::force_reduce_motion(true);
    assert!(motion::reduce_motion());
    for duration in [
        motion::overlay(),
        motion::menu(),
        motion::banner(),
        motion::crossfade(),
        motion::detail(),
        motion::chip(),
        motion::selection(),
        motion::progress(),
        motion::new_card(),
    ] {
        assert_eq!(duration, Duration::ZERO);
    }
}

#[gpui::test]
fn motion_tokens_stay_positive_when_reduced_motion_is_off(_cx: &mut TestAppContext) {
    let _guard = motion::force_reduce_motion(false);
    assert!(!motion::reduce_motion());
    assert_eq!(motion::overlay(), motion::OVERLAY);
    assert_eq!(motion::new_card(), motion::NEW_CARD);
    assert!(motion::overlay() > Duration::ZERO);
}

#[gpui::test]
fn closing_keeps_content_mounted_until_the_duration_elapses(cx: &mut TestAppContext) {
    let _guard = motion::force_reduce_motion(false);
    let host = cx.new(|_| Host {
        live: None,
        linger: Linger::default(),
    });
    host.update(cx, |host, _| host.open("dialog"));
    host.read_with(cx, |host, _| {
        assert_eq!(host.visible(), Some(("dialog", false)))
    });

    host.update(cx, |host, cx| host.close(cx));
    host.read_with(cx, |host, _| {
        assert_eq!(host.visible(), Some(("dialog", true)));
        assert!(host.linger.is_closing());
    });

    cx.executor()
        .advance_clock(motion::overlay() - Duration::from_millis(20));
    cx.run_until_parked();
    host.read_with(cx, |host, _| {
        assert_eq!(host.visible(), Some(("dialog", true)))
    });

    cx.executor().advance_clock(Duration::from_millis(20));
    cx.run_until_parked();
    host.read_with(cx, |host, _| {
        assert_eq!(host.visible(), None);
        assert!(!host.linger.is_closing());
    });
}

#[gpui::test]
fn reduced_motion_removes_content_without_lingering(cx: &mut TestAppContext) {
    let _guard = motion::force_reduce_motion(true);
    let host = cx.new(|_| Host {
        live: None,
        linger: Linger::default(),
    });
    host.update(cx, |host, _| host.open("dialog"));
    host.update(cx, |host, cx| host.close(cx));
    cx.run_until_parked();
    host.read_with(cx, |host, _| {
        assert_eq!(host.visible(), None);
        assert!(!host.linger.is_closing());
    });
}

#[gpui::test]
fn opening_again_cancels_an_in_flight_exit(cx: &mut TestAppContext) {
    let _guard = motion::force_reduce_motion(false);
    let host = cx.new(|_| Host {
        live: None,
        linger: Linger::default(),
    });
    host.update(cx, |host, cx| {
        host.open("first");
        host.close(cx);
        host.open("second");
    });
    host.read_with(cx, |host, _| {
        assert_eq!(host.visible(), Some(("second", false)))
    });
    cx.executor().advance_clock(motion::overlay());
    cx.run_until_parked();
    host.read_with(cx, |host, _| {
        assert_eq!(host.visible(), Some(("second", false)))
    });
}

use std::path::Path;
use voon_app::view_model::AppState;
use voon_core::{Album, AlbumMetadata, Field, FieldSource, ScanResult, db::Index};

fn editable_state(cx: &mut TestAppContext) -> gpui::Entity<AppState> {
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(
            Path::new("/假音乐库"),
            &ScanResult {
                albums: vec![Album {
                    source_directories: vec![],
                    candidates: vec![],
                    path: "/假音乐库/夜曲".into(),
                    folder_name: "夜曲".into(),
                    discs: vec![],
                    cover: None,
                    collection: None,
                    signature: "夜曲".into(),
                    metadata: AlbumMetadata {
                        title: Some(Field::new("夜曲".into(), FieldSource::Tag)),
                        ..AlbumMetadata::default()
                    },
                }],
                ..ScanResult::default()
            },
        )
        .unwrap();
    let state = cx.new(|_| AppState::new(index, "/建议目录".into()).unwrap());
    state.update(cx, |state, cx| {
        state.select_album(Some("/假音乐库/夜曲".into()), cx);
        state.begin_album_edit(cx).unwrap();
    });
    state
}

#[gpui::test]
fn cancelling_an_edit_keeps_the_sheet_visible_until_the_overlay_finishes(cx: &mut TestAppContext) {
    let _guard = motion::force_reduce_motion(false);
    let state = editable_state(cx);
    state.update(cx, |state, cx| state.cancel_album_edit(cx));
    state.read_with(cx, |state, _| {
        assert!(state.album_edit_draft().is_none());
        assert!(state.album_edit_is_closing());
        assert!(state.album_edit_view().is_some_and(|(_, closing)| closing));
    });
    cx.executor().advance_clock(motion::overlay());
    cx.run_until_parked();
    state.read_with(cx, |state, _| {
        assert!(state.album_edit_view().is_none());
        assert!(!state.album_edit_is_closing());
    });
}

#[gpui::test]
fn reduced_motion_clears_the_edit_sheet_without_a_closing_phase(cx: &mut TestAppContext) {
    let _guard = motion::force_reduce_motion(true);
    let state = editable_state(cx);
    state.update(cx, |state, cx| state.cancel_album_edit(cx));
    cx.run_until_parked();
    state.read_with(cx, |state, _| {
        assert!(state.album_edit_draft().is_none());
        assert!(state.album_edit_view().is_none());
        assert!(!state.album_edit_is_closing());
    });
}
