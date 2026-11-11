use std::fs;
use voon_core::{
    db::Index,
    scan::{NoTags, scan_root},
};

#[test]
fn completing_a_folder_only_title_resolves_attention_until_the_override_is_cleared() {
    let library = tempfile::tempdir().unwrap();
    let album = library.path().join("仅有目录标题");
    fs::create_dir(&album).unwrap();
    fs::write(album.join("01.flac"), b"fixture").unwrap();
    let scan = scan_root(library.path(), &mut NoTags);
    let mut index = Index::open_in_memory().unwrap();
    index.store_scan(library.path(), &scan).unwrap();
    assert!(
        index
            .attention_items()
            .unwrap()
            .iter()
            .any(|item| item.kind == "TitleFromFolderOnly")
    );
    index.set_user_edit(&album, "title", "补全的标题").unwrap();
    index.store_scan(library.path(), &scan).unwrap();
    assert!(
        !index
            .attention_items()
            .unwrap()
            .iter()
            .any(|item| item.kind == "TitleFromFolderOnly")
    );
    index.clear_user_edit(&album, "title").unwrap();
    assert!(
        index
            .attention_items()
            .unwrap()
            .iter()
            .any(|item| item.kind == "TitleFromFolderOnly")
    );
}

#[test]
fn a_physical_issue_links_to_the_local_merge_result() {
    let library = tempfile::tempdir().unwrap();
    let first = library.path().join("第一张");
    let second = library.path().join("第二张");
    for path in [&first, &second] {
        fs::create_dir(path).unwrap();
        fs::write(path.join("01.flac"), b"fixture").unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    fs::write(
        first.join("broken.cue"),
        "FILE \"missing.wav\" WAVE\n TRACK 01 AUDIO\n INDEX 01 00:00:00\n",
    )
    .unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    let merged = index
        .merge_albums(&[first.clone(), second], &first, &[])
        .unwrap();
    let items = index.attention_items().unwrap();
    let issue = items
        .iter()
        .find(|item| item.kind == "CueTargetMissing")
        .unwrap();
    assert_eq!(issue.album_path.as_ref(), Some(&merged));
    index
        .set_user_edit(&merged, "title", "已确认的标题")
        .unwrap();
    assert!(
        !index
            .attention_items()
            .unwrap()
            .iter()
            .any(|item| item.kind == "TitleFromFolderOnly")
    );
}

#[test]
fn indexed_attention_explains_cue_failures_and_links_the_related_album() {
    let library = tempfile::tempdir().unwrap();
    let album = library.path().join("专辑");
    fs::create_dir(&album).unwrap();
    fs::write(album.join("01.flac"), b"fixture").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    fs::write(
        album.join("broken.cue"),
        "FILE \"missing.wav\" WAVE\n TRACK 01 AUDIO\n INDEX 01 00:00:00\n",
    )
    .unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    let items = index.attention_items().unwrap();
    let broken = items
        .iter()
        .find(|item| item.kind == "CueTargetMissing")
        .unwrap();
    assert_eq!(broken.album_path.as_deref(), Some(album.as_path()));
    assert!(broken.explanation.contains("找不到"));
    assert!(broken.explanation.contains("broken.cue"));
    assert!(broken.explanation.contains("missing.wav"));
    fs::remove_file(album.join("broken.cue")).unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    assert!(
        !index
            .attention_items()
            .unwrap()
            .iter()
            .any(|item| item.kind == "CueTargetMissing")
    );
}

#[test]
fn every_attention_kind_is_queryable_after_real_scans() {
    use std::collections::BTreeSet;
    let library = tempfile::tempdir().unwrap();
    for folder in ["外层", "外层/内层", "坏分轨"] {
        fs::create_dir_all(library.path().join(folder)).unwrap();
        fs::write(
            library.path().join(folder).join("01.flac"),
            folder.as_bytes(),
        )
        .unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    fs::rename(
        library.path().join("外层/内层"),
        library.path().join("外层/改名后"),
    )
    .unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    fs::write(library.path().join("散音频.flac"), b"loose").unwrap();
    fs::write(
        library.path().join("坏分轨/empty.cue"),
        "TITLE \"空分轨\"\nFILE \"01.flac\" WAVE\n",
    )
    .unwrap();
    fs::write(
        library.path().join("坏分轨/broken.cue"),
        "FILE \"missing.wav\" WAVE\n TRACK 01 AUDIO\n INDEX 01 00:00:00\n",
    )
    .unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    let unavailable = library.path().join("不存在的根目录");
    index
        .store_scan(&unavailable, &scan_root(&unavailable, &mut NoTags))
        .unwrap();
    let items = index.attention_items().unwrap();
    let kinds: BTreeSet<_> = items.iter().map(|item| item.kind.as_str()).collect();
    assert_eq!(
        kinds,
        BTreeSet::from([
            "CueTargetMissing",
            "CueWithoutTracks",
            "NestedAlbum",
            "Unreadable",
            "TitleFromFolderOnly",
            "LooseUntagged",
            "PossibleRename"
        ])
    );
    assert!(
        items
            .iter()
            .all(|item| !item.title.is_empty() && !item.explanation.is_empty())
    );
}

#[test]
fn an_expired_rename_cannot_move_user_data_to_an_unrelated_album() {
    let library = tempfile::tempdir().unwrap();
    for folder in ["甲", "乙"] {
        fs::create_dir(library.path().join(folder)).unwrap();
        fs::write(
            library.path().join(folder).join("01.flac"),
            folder.as_bytes(),
        )
        .unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    let first = library.path().join("甲");
    let second = library.path().join("乙");
    index.set_user_edit(&first, "title", "专属标题").unwrap();
    assert!(index.confirm_rename(&first, &second).is_err());
    assert_eq!(
        index.album(&first).unwrap().unwrap().display_title(),
        "专属标题"
    );
    assert_eq!(index.album(&second).unwrap().unwrap().display_title(), "乙");
}
