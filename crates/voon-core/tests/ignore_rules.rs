use std::path::Path;

use voon_core::db::Index;
use voon_core::scan::{NoTags, ScanOptions, scan_root_with_options};

fn scan(root: &Path, index: &Index) -> voon_core::ScanResult {
    scan_root_with_options(
        root,
        &mut NoTags,
        &std::sync::atomic::AtomicBool::new(false),
        &mut |_| {},
        &ScanOptions {
            ignored_paths: index.ignored_paths(root).unwrap(),
        },
    )
}

fn nas_library() -> tempfile::TempDir {
    let library = tempfile::tempdir().unwrap();
    for relative in [
        "Philips Duo/Schubert/CD1/01.flac",
        "Alfred_Brendel/Mozart/Disc 1/01.wv",
    ] {
        let file = library.path().join(relative);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, b"audio fixture").unwrap();
    }
    library
}

#[test]
fn ignored_subfolders_are_saved_per_library_root_and_survive_reopening() {
    let local = tempfile::tempdir().unwrap();
    let database = local.path().join("library.sqlite3");
    let root = Path::new("/Volumes/NAS1/Music");
    let other = Path::new("/Volumes/NAS2/Music");
    let index = Index::open(&database).unwrap();
    index.add_root(root).unwrap();
    index.add_root(other).unwrap();
    index
        .add_ignored_path(root, Path::new("Philips Duo/临时整理"))
        .unwrap();
    index
        .add_ignored_path(root, &root.join("Philips Duo/临时整理"))
        .unwrap();
    drop(index);

    let index = Index::open(&database).unwrap();
    assert_eq!(
        index.ignored_paths(root).unwrap(),
        vec![Path::new("Philips Duo/临时整理")]
    );
    assert!(index.ignored_paths(other).unwrap().is_empty());
    index
        .remove_ignored_path(root, Path::new("Philips Duo/临时整理"))
        .unwrap();
    assert!(index.ignored_paths(root).unwrap().is_empty());
}

#[test]
fn ignore_rules_cannot_exclude_the_root_or_escape_to_another_root() {
    let index = Index::open_in_memory().unwrap();
    let root = Path::new("/Volumes/NAS1/Music");
    index.add_root(root).unwrap();
    for invalid in [
        "",
        ".",
        "..",
        "../Music2",
        "Philips Duo/../Kept",
        "/Volumes/NAS1/Music2",
        "/Volumes/NAS1/Music",
    ] {
        assert!(
            index.add_ignored_path(root, Path::new(invalid)).is_err(),
            "accepted {invalid:?}"
        );
    }
    assert!(index.ignored_paths(root).unwrap().is_empty());
}

#[test]
fn ignoring_a_collection_removes_its_albums_on_rescan_and_removing_the_rule_restores_them() {
    let library = nas_library();
    let local = tempfile::tempdir().unwrap();
    let database = local.path().join("library.sqlite3");
    let root = library.path();
    let mut index = Index::open(&database).unwrap();
    index.store_scan(root, &scan(root, &index)).unwrap();
    assert_eq!(index.albums().unwrap().len(), 2);
    index
        .add_ignored_path(root, Path::new("Philips Duo"))
        .unwrap();
    drop(index);

    let mut index = Index::open(&database).unwrap();
    index.store_scan(root, &scan(root, &index)).unwrap();
    assert_eq!(
        index
            .albums()
            .unwrap()
            .iter()
            .map(|album| album.folder_name.as_str())
            .collect::<Vec<_>>(),
        ["Mozart"]
    );
    assert_eq!(
        index
            .collections()
            .unwrap()
            .iter()
            .map(|(_, name)| name.as_str())
            .collect::<Vec<_>>(),
        ["Alfred_Brendel"]
    );
    index
        .remove_ignored_path(root, Path::new("Philips Duo"))
        .unwrap();
    index.store_scan(root, &scan(root, &index)).unwrap();
    assert_eq!(index.albums().unwrap().len(), 2);
    assert_eq!(index.collections().unwrap().len(), 2);
}

#[test]
fn ignored_disc_and_attachment_subtrees_supply_neither_tracks_nor_covers() {
    let library = nas_library();
    let root = library.path();
    let album_path = root.join("Philips Duo/Schubert");
    for relative in ["CD2/01.flac", "Booklet/front.jpg"] {
        let file = album_path.join(relative);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, b"fixture").unwrap();
    }
    let index = Index::open_in_memory().unwrap();
    let before = scan(root, &index);
    let before = before
        .albums
        .iter()
        .find(|album| album.path == album_path)
        .unwrap();
    assert_eq!(before.discs.len(), 2);
    assert!(before.cover.is_some());

    index
        .add_ignored_path(root, &album_path.join("CD2"))
        .unwrap();
    index
        .add_ignored_path(root, &album_path.join("Booklet"))
        .unwrap();
    let after = scan(root, &index);
    let album = after
        .albums
        .iter()
        .find(|album| album.path == album_path)
        .unwrap();
    assert_eq!(album.discs.len(), 1);
    assert_eq!(album.discs[0].number, 1);
    assert!(album.cover.is_none());
    assert!(
        after
            .stamps
            .iter()
            .all(|(path, _)| !path.starts_with(album_path.join("CD2")))
    );
}

#[test]
fn options_cannot_escape_the_root_and_subtree_names_have_component_boundaries() {
    let library = nas_library();
    let root = library.path();
    std::fs::create_dir_all(root.join("Philips Duo Bonus/Bach")).unwrap();
    std::fs::write(root.join("Philips Duo Bonus/Bach/01.flac"), b"fixture").unwrap();
    let options = ScanOptions {
        ignored_paths: vec![
            "Philips Duo".into(),
            "".into(),
            ".".into(),
            "../Alfred_Brendel".into(),
            "/Alfred_Brendel".into(),
            root.to_path_buf(),
        ],
    };
    let result = scan_root_with_options(
        root,
        &mut NoTags,
        &std::sync::atomic::AtomicBool::new(false),
        &mut |_| {},
        &options,
    );
    assert_eq!(
        result
            .albums
            .iter()
            .map(|album| album.folder_name.as_str())
            .collect::<Vec<_>>(),
        ["Mozart", "Bach"]
    );
}
