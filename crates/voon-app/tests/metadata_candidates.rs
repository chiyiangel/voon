use gpui::{AppContext, TestAppContext};
use std::path::Path;
use voon_app::view_model::AppState;
use voon_core::{
    AudioFormat, FieldSource,
    db::{CachedTags, Index},
    scan::{FileStamp, TagReader, scan_root},
    tags::TagInfo,
};

struct Tags;
impl TagReader for Tags {
    fn read(&mut self, _: &Path, _: AudioFormat, _: FileStamp) -> Option<TagInfo> {
        Some(TagInfo {
            album: Some("Tag title".into()),
            year: Some(1999),
            album_artist: Some("Tag artist".into()),
            artist: Some("Tag performer".into()),
            composer: Some("Tag composer".into()),
            conductor: Some("Tag conductor".into()),
            genre: Some("Classical".into()),
            catalog_number: Some("TAG-123".into()),
            ..Default::default()
        })
    }
}

#[gpui::test]
fn all_fields_share_effective_values_with_details_and_composer_browsing(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    let album = library.path().join("Folder title (1988) [Remaster]");
    std::fs::create_dir(&album).unwrap();
    std::fs::write(album.join("01.flac"), b"fixture").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    let scan = {
        let mut tags = CachedTags::new(&index, Tags);
        scan_root(library.path(), &mut tags)
    };
    index.store_scan(library.path(), &scan).unwrap();
    let state = cx.new(|_| AppState::new(index, library.path().into()).unwrap());
    state.update(cx, |state, cx| {
        state.select_album(Some(album.clone()), cx);
        state.begin_album_edit(cx).unwrap();
        state
            .set_album_edit_field("composer", "My composer".into(), cx)
            .unwrap();
        state
            .set_album_edit_field("orchestra", "My orchestra".into(), cx)
            .unwrap();
        state.save_album_edit(cx).unwrap();
    });
    state.read_with(cx, |state, _| {
        let comparisons = state.album_metadata_comparison().unwrap();
        assert_eq!(comparisons.len(), 10);
        let entries = state.album_detail().unwrap().metadata.entries();
        for field in &comparisons {
            let effective = entries
                .iter()
                .find(|(name, _, _)| *name == field.name)
                .map(|(_, value, _)| value.as_str());
            assert_eq!(
                field.effective.as_ref().map(|value| value.value.as_str()),
                effective
            );
            if field.name != "orchestra" {
                assert!(!field.candidates.is_empty(), "missing {}", field.name);
            }
        }
        let composer = comparisons
            .iter()
            .find(|field| field.name == "composer")
            .unwrap();
        assert_eq!(composer.effective.as_ref().unwrap().value, "My composer");
        assert_eq!(composer.automatic.as_ref().unwrap().value, "Tag composer");
        assert_eq!(state.composer_options()[0].name, "My composer");
    });
}

#[gpui::test]
fn details_compare_sources_and_blank_editor_restores_automatic_without_losing_evidence(
    cx: &mut TestAppContext,
) {
    let library = tempfile::tempdir().unwrap();
    let album = library.path().join("Folder title (1988)");
    std::fs::create_dir(&album).unwrap();
    std::fs::write(album.join("01.flac"), b"fixture").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    let scan = {
        let mut tags = CachedTags::new(&index, Tags);
        scan_root(library.path(), &mut tags)
    };
    index.store_scan(library.path(), &scan).unwrap();
    let state = cx.new(|_| AppState::new(index, library.path().into()).unwrap());
    state.update(cx, |state, cx| {
        state.select_album(Some(album.clone()), cx);
        state.begin_album_edit(cx).unwrap();
        state
            .set_album_edit_field("title", "My title".into(), cx)
            .unwrap();
        state.save_album_edit(cx).unwrap();
    });
    let sources = state.read_with(cx, |state, _| {
        let fields = state.album_metadata_comparison().unwrap();
        let title = fields.iter().find(|field| field.name == "title").unwrap();
        assert_eq!(title.automatic.as_ref().unwrap().value, "Tag title");
        assert_eq!(title.user_value.as_deref(), Some("My title"));
        assert_eq!(
            title.effective.as_ref().unwrap().source,
            FieldSource::UserEdit
        );
        assert_eq!(title.candidates.len(), 2);
        title.candidates.clone()
    });
    state.update(cx, |state, cx| {
        state.begin_album_edit(cx).unwrap();
        state
            .set_album_edit_field("title", "  ".into(), cx)
            .unwrap();
        state.save_album_edit(cx).unwrap();
    });
    state.read_with(cx, |state, _| {
        let fields = state.album_metadata_comparison().unwrap();
        let title = fields.iter().find(|field| field.name == "title").unwrap();
        assert_eq!(title.effective.as_ref().unwrap().value, "Tag title");
        assert_eq!(title.user_value, None);
        assert_eq!(title.candidates, sources);
        assert_eq!(state.album_detail().unwrap().display_title(), "Tag title");
    });
}
