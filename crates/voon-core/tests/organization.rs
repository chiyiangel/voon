use std::path::{Path, PathBuf};

use voon_core::{
    db::Index,
    scan::{NoTags, scan_root},
};

fn library() -> tempfile::TempDir {
    let library = tempfile::tempdir().unwrap();
    for file in [
        "Philips Duo/Schubert/CD1/01.flac",
        "Alfred_Brendel/Mozart/Disc 1/01.wv",
    ] {
        let file = library.path().join(file);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, b"fixture").unwrap();
    }
    library
}

fn album_paths(root: &Path) -> Vec<PathBuf> {
    vec![
        root.join("Philips Duo/Schubert"),
        root.join("Alfred_Brendel/Mozart"),
    ]
}

#[test]
fn tags_can_be_applied_in_bulk_renamed_and_deleted_without_duplicate_membership() {
    let library = library();
    let root = library.path();
    let albums = album_paths(root);
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    index
        .set_user_tag_for_albums(&albums, " 喜欢的演奏 ", true)
        .unwrap();
    index
        .set_user_tag_for_albums(&albums, "喜欢的演奏", true)
        .unwrap();
    assert_eq!(index.user_tags(&albums[0]).unwrap(), ["喜欢的演奏"]);
    assert_eq!(index.user_tag_catalog().unwrap()[0].albums.len(), 2);
    index.rename_user_tag("喜欢的演奏", "古典名演").unwrap();
    assert_eq!(index.user_tags(&albums[1]).unwrap(), ["古典名演"]);
    index
        .set_user_tag_for_albums(&albums[..1], "古典名演", false)
        .unwrap();
    assert!(index.user_tags(&albums[0]).unwrap().is_empty());
    index.delete_user_tag("古典名演").unwrap();
    assert!(index.user_tag_catalog().unwrap().is_empty());
    assert!(index.add_user_tag(&albums[0], " \t ").is_err());
}

#[test]
fn manual_collections_preserve_order_and_membership_across_rescan_and_reopening() {
    let library = library();
    let local = tempfile::tempdir().unwrap();
    let root = library.path();
    let albums = album_paths(root);
    let database = local.path().join("library.sqlite3");
    let mut index = Index::open(&database).unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    let collection = index.create_manual_collection(" 周末 ").unwrap();
    assert_eq!(index.create_manual_collection("周末").unwrap(), collection);
    index
        .set_manual_collection_for_albums(collection, &albums, true)
        .unwrap();
    index
        .set_manual_collection_for_albums(collection, &albums, true)
        .unwrap();
    let reordered = vec![albums[1].clone(), albums[0].clone()];
    index
        .set_manual_collection_order(collection, &reordered)
        .unwrap();
    assert!(
        index
            .set_manual_collection_order(collection, &[albums[0].clone(), albums[0].clone()])
            .is_err()
    );
    assert!(
        index
            .set_manual_collection_order(collection, &albums[..1])
            .is_err()
    );
    assert_eq!(
        index.manual_collection_albums(collection).unwrap(),
        reordered
    );
    index.rename_manual_collection(collection, "晨间").unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    drop(index);

    let mut index = Index::open(&database).unwrap();
    let catalog = index.manual_collections().unwrap();
    assert_eq!(catalog[0].name, "晨间");
    assert_eq!(catalog[0].albums, reordered);
    index
        .set_manual_collection_for_albums(collection, &albums[..1], false)
        .unwrap();
    assert_eq!(
        index.manual_collection_albums(collection).unwrap(),
        vec![albums[1].clone()]
    );
    index.delete_manual_collection(collection).unwrap();
    assert!(index.manual_collections().unwrap().is_empty());
    assert!(index.create_manual_collection(" \n ").is_err());
}

fn files(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = std::collections::BTreeMap::new();
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    std::fs::read(path).unwrap(),
                );
            }
        }
    }
    files
}

#[test]
fn local_organization_survives_rescans_without_creating_or_changing_library_files() {
    let library = library();
    let local = tempfile::tempdir().unwrap();
    let root = library.path();
    let before = files(root);
    let albums = album_paths(root);
    let database = local.path().join("library.sqlite3");
    let mut index = Index::open(&database).unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    index
        .set_user_tag_for_albums(&albums, "古典名演", true)
        .unwrap();
    let collection = index.create_manual_collection("周末").unwrap();
    index
        .set_manual_collection_for_albums(collection, &albums, true)
        .unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    drop(index);
    let index = Index::open(&database).unwrap();
    assert_eq!(index.user_tags(&albums[0]).unwrap(), ["古典名演"]);
    assert_eq!(index.manual_collection_albums(collection).unwrap(), albums);
    assert_eq!(files(root), before);
}

#[test]
fn conflicting_renames_and_invalid_orders_leave_local_organization_intact() {
    let index = Index::open_in_memory().unwrap();
    index
        .add_user_tag(Path::new("/假音乐库/Album"), "已有标签")
        .unwrap();
    index.create_user_tag("另一个标签").unwrap();
    assert!(index.rename_user_tag("已有标签", "另一个标签").is_err());
    assert_eq!(
        index.user_tags(Path::new("/假音乐库/Album")).unwrap(),
        ["已有标签"]
    );
    let collection = index.create_manual_collection("已有合集").unwrap();
    index.create_manual_collection("另一个合集").unwrap();
    assert!(
        index
            .rename_manual_collection(collection, "另一个合集")
            .is_err()
    );
    assert_eq!(
        index
            .manual_collections()
            .unwrap()
            .iter()
            .find(|item| item.id == collection)
            .unwrap()
            .name,
        "已有合集"
    );
}
