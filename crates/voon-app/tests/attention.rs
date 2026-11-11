use gpui::{AppContext, TestAppContext};
use std::fs;
use voon_app::view_model::{AppState, Navigation};
use voon_core::{
    db::Index,
    scan::{NoTags, scan_root},
};

#[gpui::test]
fn attention_items_link_to_album_details_or_expose_the_ungrouped_file(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    fs::create_dir(library.path().join("专辑")).unwrap();
    fs::write(library.path().join("专辑/01.flac"), b"fixture").unwrap();
    fs::write(library.path().join("散音频.flac"), b"fixture").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    let state = cx.new(|_| AppState::new(index, library.path().into()).unwrap());
    let items = state.read_with(cx, |state, _| state.attention_items().to_vec());
    assert_eq!(items.len(), 2);
    state.update(cx, |state, cx| {
        state.navigate(Navigation::NeedsAttention, cx)
    });
    let album = items
        .iter()
        .find(|item| item.kind == "TitleFromFolderOnly")
        .unwrap();
    state.update(cx, |state, cx| {
        state.open_attention(&album.path, &album.kind, cx)
    });
    state.read_with(cx, |state, _| {
        assert_eq!(
            state.album_detail().unwrap().path,
            library.path().join("专辑")
        )
    });
    let loose = items
        .iter()
        .find(|item| item.kind == "LooseUntagged")
        .unwrap();
    state.update(cx, |state, cx| {
        state.open_attention(&loose.path, &loose.kind, cx)
    });
    state.read_with(cx, |state, _| {
        assert!(state.selected_album().is_none());
        assert_eq!(state.attention_path(), Some(loose.path.as_path()));
    });
}

#[gpui::test]
fn confirming_a_rename_removes_the_candidate_and_refreshes_the_new_album(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    let old = library.path().join("旧路径");
    let new = library.path().join("新路径");
    fs::create_dir(&old).unwrap();
    fs::write(old.join("01.flac"), b"fixture").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    index.set_user_edit(&old, "title", "整理后的标题").unwrap();
    fs::rename(&old, &new).unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    let state = cx.new(|_| AppState::new(index, library.path().into()).unwrap());
    let before = state.read_with(cx, |state, _| state.attention_items().len());
    state.update(cx, |state, cx| {
        state.confirm_rename(&old, &new, cx).unwrap()
    });
    state.read_with(cx, |state, _| {
        assert!(state.attention_items().len() < before);
        // Migrating a completed title resolves both the rename candidate and
        // the destination's folder-only title warning.
        assert!(
            !state
                .attention_items()
                .iter()
                .any(|item| matches!(item.kind.as_str(), "PossibleRename" | "TitleFromFolderOnly"))
        );
        assert_eq!(state.album_detail().unwrap().path, new);
        assert_eq!(
            state.album_detail().unwrap().display_title(),
            "整理后的标题"
        );
    });
}

#[gpui::test]
fn tag_failure_is_visible_after_reopen_and_links_to_the_preserved_album(cx: &mut TestAppContext) {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let album = library.path().join("专辑");
    fs::create_dir(&album).unwrap();
    let audio = album.join("01.flac");
    fs::write(&audio, b"invalid FLAC").unwrap();
    let database = local.path().join("index.sqlite3");
    let mut index = Index::open(&database).unwrap();
    let scan = {
        let mut tags = voon_core::db::CachedTags::new(&index, voon_core::scan::DiskTags);
        scan_root(library.path(), &mut tags)
    };
    index.store_scan(library.path(), &scan).unwrap();
    drop(index);
    let state =
        cx.new(|_| AppState::new(Index::open(&database).unwrap(), library.path().into()).unwrap());
    let item = state.read_with(cx, |state, _| {
        state
            .attention_items()
            .iter()
            .find(|item| item.kind == "TagParseFailure")
            .unwrap()
            .clone()
    });
    assert_eq!(item.path, audio);
    assert!(item.explanation.contains("标签读取"));
    assert!(item.explanation.contains("尚无成功资料"));
    state.update(cx, |state, cx| {
        state.open_attention(&item.path, &item.kind, cx)
    });
    state.read_with(cx, |state, _| {
        assert_eq!(state.album_detail().unwrap().path, album);
        assert_eq!(state.album_detail().unwrap().tracks().count(), 1);
    });
}

#[gpui::test]
fn each_split_detail_has_stale_status_even_when_attention_navigation_targets_another_album(
    cx: &mut TestAppContext,
) {
    let library = tempfile::tempdir().unwrap();
    let root = library.path();
    let collection = root.join("系列");
    let album = collection.join("套装");
    for path in [album.join("CD1"), album.join("CD2")] {
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("01.flac"), b"fixture").unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    let outputs = index
        .split_album(&album, voon_core::db::SplitMode::Discs)
        .unwrap();
    let mut failed = voon_core::ScanResult::default();
    failed.issues.push(voon_core::scan::ScanIssue {
        path: collection.clone(),
        kind: voon_core::scan::IssueKind::Unreadable {
            error: "临时断连".into(),
        },
    });
    index.store_partial_scan(root, &failed).unwrap();
    let state = cx.new(|_| AppState::new(index, root.into()).unwrap());
    for output in outputs {
        state.update(cx, |state, cx| state.select_album(Some(output.clone()), cx));
        state.read_with(cx, |state, _| {
            assert_eq!(state.album_scan_issues(&output).len(), 1);
            assert!(state.album_scan_issues(&output)[0].retains_previous);
            let fields = state.album_metadata_comparison().unwrap();
            let title = fields.iter().find(|field| field.name == "title").unwrap();
            assert!(title.candidates.iter().any(|candidate| matches!(
                candidate.status,
                voon_core::SourceStatus::Unconfirmed { .. }
            )));
        });
    }
}
