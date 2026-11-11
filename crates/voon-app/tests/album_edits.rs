use gpui::{AppContext, TestAppContext};
use std::path::Path;
use voon_app::view_model::AppState;
use voon_core::{Album, AlbumMetadata, Field, FieldSource, ScanResult, db::Index};

fn fixture(title: &str) -> Album {
    Album {
        source_directories: vec![],
        candidates: vec![],
        path: Path::new("/假音乐库").join(title),
        folder_name: title.into(),
        discs: vec![],
        cover: None,
        collection: None,
        signature: title.into(),
        metadata: AlbumMetadata {
            title: Some(Field::new(title.into(), FieldSource::Tag)),
            year: Some(Field::new(1999, FieldSource::Tag)),
            ..AlbumMetadata::default()
        },
    }
}

#[gpui::test]
fn saving_a_draft_updates_detail_sources_and_restoring_uses_the_automatic_value(
    cx: &mut TestAppContext,
) {
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(
            Path::new("/假音乐库"),
            &ScanResult {
                albums: vec![fixture("自动标题")],
                ..ScanResult::default()
            },
        )
        .unwrap();
    let state = cx.new(|_| AppState::new(index, "/建议目录".into()).unwrap());
    state.update(cx, |state, cx| {
        state.select_album(Some("/假音乐库/自动标题".into()), cx);
        state.begin_album_edit(cx).unwrap();
        state
            .set_album_edit_field("title", "我的标题".into(), cx)
            .unwrap();
        state.save_album_edit(cx).unwrap();
    });
    state.read_with(cx, |state, _| {
        assert!(state.album_edit_draft().is_none());
        let title = state
            .album_detail()
            .unwrap()
            .metadata
            .title
            .as_ref()
            .unwrap();
        assert_eq!(title.value, "我的标题");
        assert_eq!(title.source, FieldSource::UserEdit);
    });
    state.update(cx, |state, cx| {
        state.begin_album_edit(cx).unwrap();
        state.restore_album_edit_field("title", cx).unwrap();
        state.save_album_edit(cx).unwrap();
    });
    state.read_with(cx, |state, _| {
        let title = state
            .album_detail()
            .unwrap()
            .metadata
            .title
            .as_ref()
            .unwrap();
        assert_eq!(title.value, "自动标题");
        assert_eq!(title.source, FieldSource::Tag);
    });
}

#[gpui::test]
fn invalid_years_stay_in_the_draft_and_switching_albums_discards_unsaved_edits(
    cx: &mut TestAppContext,
) {
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(
            Path::new("/假音乐库"),
            &ScanResult {
                albums: vec![fixture("第一张"), fixture("第二张")],
                ..ScanResult::default()
            },
        )
        .unwrap();
    let state = cx.new(|_| AppState::new(index, "/建议目录".into()).unwrap());
    state.update(cx, |state, cx| {
        state.select_album(Some("/假音乐库/第一张".into()), cx);
        state.begin_album_edit(cx).unwrap();
        state
            .set_album_edit_field("title", "不应保存".into(), cx)
            .unwrap();
        state
            .set_album_edit_field("year", "20xx".into(), cx)
            .unwrap();
        assert!(state.save_album_edit(cx).is_err());
    });
    state.read_with(cx, |state, _| {
        assert!(state.album_edit_draft().unwrap().error().is_some());
        assert_eq!(state.album_detail().unwrap().display_title(), "第一张");
    });
    state.update(cx, |state, cx| {
        state.select_album(Some("/假音乐库/第二张".into()), cx)
    });
    state.read_with(cx, |state, _| {
        assert!(state.album_edit_draft().is_none());
        assert_eq!(state.album_detail().unwrap().display_title(), "第二张");
    });
    state.update(cx, |state, cx| {
        assert!(state.save_album_edit(cx).is_err());
        state.select_album(Some("/假音乐库/第一张".into()), cx);
    });
    state.read_with(cx, |state, _| {
        assert_eq!(state.album_detail().unwrap().display_title(), "第一张")
    });
}
