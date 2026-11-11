use voon_core::{
    db::Index,
    scan::{NoTags, scan_root},
};

#[test]
fn collection_tree_keeps_nested_parents_and_includes_descendant_albums() {
    let library = tempfile::tempdir().unwrap();
    for relative in [
        "系列/套装/第一张/CD1/01.flac",
        "系列/套装/第二张/01.flac",
        "系列/独立专辑/01.flac",
        "其他专辑/01.flac",
    ] {
        let path = library.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"fake audio").unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();

    let tree = index.collection_tree().unwrap();
    assert_eq!(tree.len(), 2);
    let series = tree
        .iter()
        .find(|collection| collection.name == "系列")
        .unwrap();
    let boxed = tree
        .iter()
        .find(|collection| collection.name == "套装")
        .unwrap();
    assert!(series.parent.is_none());
    assert_eq!(boxed.parent.as_ref(), Some(&series.path));
    assert_eq!(series.albums.len(), 3);
    assert_eq!(boxed.albums.len(), 2);
    assert!(
        boxed
            .albums
            .contains(&library.path().join("系列/套装/第一张"))
    );
    assert!(!series.albums.contains(&library.path().join("其他专辑")));
}
