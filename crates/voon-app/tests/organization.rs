use gpui::{AppContext, TestAppContext};
use voon_app::view_model::{
    AppState, Navigation, SortOrder,
    organization::{OrganizationDialog, OrganizationScope},
};
use voon_core::{Album, AlbumMetadata, ScanResult, db::Index};

fn index() -> Index {
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(
            std::path::Path::new("/假音乐库"),
            &ScanResult {
                albums: ["Bach", "Mozart", "Schubert"]
                    .into_iter()
                    .map(|title| Album {
                        source_directories: vec![],
                        candidates: vec![],
                        path: format!("/假音乐库/{title}").into(),
                        folder_name: title.into(),
                        discs: vec![],
                        cover: None,
                        metadata: AlbumMetadata::default(),
                        collection: None,
                        signature: title.into(),
                    })
                    .collect(),
                ..ScanResult::default()
            },
        )
        .unwrap();
    index
}

#[gpui::test]
fn multi_selected_albums_can_be_tagged_and_browsed_by_that_tag(cx: &mut TestAppContext) {
    let state = cx.new(|_| AppState::new(index(), "/假音乐库".into()).unwrap());
    state.update(cx, |state, cx| {
        state.click_album("/假音乐库/Bach".into(), false, false, cx);
        state.click_album("/假音乐库/Schubert".into(), true, false, cx);
        state.set_tag_on_selection("周末名演", true, cx).unwrap();
        let tag = state.user_tag_catalog()[0].id;
        state
            .select_organization(Some(OrganizationScope::Tag(tag)), cx)
            .unwrap();
    });
    state.read_with(cx, |state, _| {
        assert!(state.selected_album_paths().is_empty());
        let mut titles: Vec<_> = state
            .browsed_albums()
            .iter()
            .map(|album| album.display_title())
            .collect();
        titles.sort();
        assert_eq!(titles, ["Bach", "Schubert"]);
    });
    state.update(cx, |state, cx| {
        state.click_album("/假音乐库/Bach".into(), false, false, cx);
        state.click_album("/假音乐库/Schubert".into(), true, false, cx);
        state.set_tag_on_selection("周末名演", false, cx).unwrap()
    });
    state.read_with(cx, |state, _| assert!(state.browsed_albums().is_empty()));
}

#[gpui::test]
fn navigation_clears_primary_multi_selection_and_detail_together(cx: &mut TestAppContext) {
    let index = index();
    let tag = index.create_user_tag("测试").unwrap();
    let collection = index.create_manual_collection("测试合集").unwrap();
    let state = cx.new(|_| AppState::new(index, "/假音乐库".into()).unwrap());
    for destination in [
        Navigation::Composers,
        Navigation::AllAlbums,
        Navigation::Collections,
        Navigation::Facets,
        Navigation::UserTags,
        Navigation::ManualCollections,
    ] {
        state.update(cx, |state, cx| {
            state.click_album("/假音乐库/Bach".into(), false, false, cx);
            state.click_album("/假音乐库/Schubert".into(), true, false, cx);
            state.navigate(destination, cx);
        });
        state.read_with(cx, |state, _| {
            assert!(state.selected_album().is_none(), "{destination:?}");
            assert!(state.selected_album_paths().is_empty(), "{destination:?}");
            assert!(state.album_detail().is_none(), "{destination:?}");
        });
    }
    for scope in 0..4 {
        state.update(cx, |state, cx| {
            state.click_album("/假音乐库/Bach".into(), false, false, cx);
            state.click_album("/假音乐库/Schubert".into(), true, false, cx);
            match scope {
                0 => state.select_collection(None, cx).unwrap(),
                1 => state.select_composer(None, cx).unwrap(),
                2 => state
                    .select_organization(Some(OrganizationScope::Tag(tag)), cx)
                    .unwrap(),
                _ => state
                    .select_organization(Some(OrganizationScope::ManualCollection(collection)), cx)
                    .unwrap(),
            }
        });
        state.read_with(cx, |state, _| {
            assert!(state.selected_album().is_none(), "scope {scope}");
            assert!(state.selected_album_paths().is_empty(), "scope {scope}");
            assert!(state.album_detail().is_none(), "scope {scope}");
        });
    }
}

#[gpui::test]
fn manual_collection_reordering_is_reflected_by_the_shared_browse_query(cx: &mut TestAppContext) {
    let state = cx.new(|_| AppState::new(index(), "/假音乐库".into()).unwrap());
    state.update(cx, |state, cx| {
        state.click_album("/假音乐库/Bach".into(), false, false, cx);
        state.click_album("/假音乐库/Schubert".into(), true, false, cx);
        let collection = state.create_manual_collection("周末", cx).unwrap();
        state
            .set_selected_manual_collection(collection, true, cx)
            .unwrap();
        state
            .select_organization(Some(OrganizationScope::ManualCollection(collection)), cx)
            .unwrap();
        state
            .move_manual_album(
                collection,
                std::path::Path::new("/假音乐库/Schubert"),
                -1,
                cx,
            )
            .unwrap();
    });
    state.read_with(cx, |state, _| {
        assert_eq!(
            state
                .browsed_albums()
                .iter()
                .map(|album| album.display_title())
                .collect::<Vec<_>>(),
            ["Schubert", "Bach"]
        )
    });
    state.update(cx, |state, cx| {
        state
            .open_organization_dialog(
                OrganizationDialog::Delete(state.organization_scope().unwrap()),
                cx,
            )
            .unwrap();
        state.confirm_organization_dialog(cx).unwrap();
    });
    state.read_with(cx, |state, _| {
        assert_eq!(state.sort_order(), SortOrder::RecentlyAdded);
        assert_eq!(state.browsed_albums().len(), 3);
    });
}

#[gpui::test]
fn shift_selection_uses_current_browse_order_and_modifier_click_can_remove_one_album(
    cx: &mut TestAppContext,
) {
    let state = cx.new(|_| AppState::new(index(), "/假音乐库".into()).unwrap());
    state.update(cx, |state, cx| {
        state.set_sort_order(SortOrder::Title, cx);
        state.click_album("/假音乐库/Bach".into(), false, false, cx);
        state.click_album("/假音乐库/Schubert".into(), false, true, cx);
    });
    state.read_with(cx, |state, _| {
        assert_eq!(state.selected_album_paths().len(), 3)
    });
    state.update(cx, |state, cx| {
        state.click_album("/假音乐库/Mozart".into(), true, false, cx)
    });
    state.read_with(cx, |state, _| {
        assert_eq!(
            state.selected_album_paths(),
            [
                std::path::Path::new("/假音乐库/Bach"),
                std::path::Path::new("/假音乐库/Schubert")
            ]
        )
    });
}

#[gpui::test]
fn renaming_and_deleting_the_current_tag_refreshes_navigation_and_albums(cx: &mut TestAppContext) {
    let state = cx.new(|_| AppState::new(index(), "/假音乐库".into()).unwrap());
    state.update(cx, |state, cx| {
        state.click_album("/假音乐库/Bach".into(), false, false, cx);
        state.set_tag_on_selection("旧标签", true, cx).unwrap();
        let scope = OrganizationScope::Tag(state.user_tag_catalog()[0].id);
        state.select_organization(Some(scope), cx).unwrap();
        state
            .open_organization_dialog(OrganizationDialog::Rename(scope), cx)
            .unwrap();
        state.set_organization_draft("新标签".into(), cx);
        state.confirm_organization_dialog(cx).unwrap();
    });
    state.read_with(cx, |state, _| {
        assert_eq!(state.user_tag_catalog()[0].name, "新标签");
        assert_eq!(state.browsed_albums().len(), 1);
    });
    state.update(cx, |state, cx| {
        state
            .open_organization_dialog(
                OrganizationDialog::Delete(state.organization_scope().unwrap()),
                cx,
            )
            .unwrap();
        state.confirm_organization_dialog(cx).unwrap();
    });
    state.read_with(cx, |state, _| {
        assert!(state.user_tag_catalog().is_empty());
        assert!(state.organization_scope().is_none());
        assert_eq!(state.browsed_albums().len(), 3);
    });
}
