use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use voon_core::{
    db::Index,
    scan::{NoTags, scan_root},
};

fn music(root: &Path, album: &str) {
    fs::create_dir_all(root.join(album)).unwrap();
    fs::write(root.join(album).join("01.flac"), b"audio fixture").unwrap();
}

#[test]
fn cancelled_scan_keeps_reported_albums_on_reopen_and_never_writes_the_library() {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    music(library.path(), "甲/CD1");
    music(library.path(), "甲/CD2");
    music(library.path(), "乙");
    let files = ["甲/CD1/01.flac", "甲/CD2/01.flac", "乙/01.flac"];
    let before: Vec<_> = files
        .iter()
        .map(|path| {
            let path = library.path().join(path);
            (
                fs::read(&path).unwrap(),
                fs::metadata(path).unwrap().modified().unwrap(),
            )
        })
        .collect();
    let cancel = AtomicBool::new(false);
    let mut reported = Vec::new();
    let scan = voon_core::scan_root_with(library.path(), &mut NoTags, &cancel, &mut |event| {
        if let voon_core::scan::ScanEvent::Album(album) = event {
            reported.push(album.path.clone());
            cancel.store(true, Ordering::Relaxed);
        }
    });
    assert!(scan.cancelled);
    assert_eq!(reported.len(), 1);
    assert_eq!(
        scan.albums
            .iter()
            .map(|album| album.path.clone())
            .collect::<Vec<_>>(),
        reported
    );
    let database = local.path().join("library.sqlite3");
    Index::open(&database)
        .unwrap()
        .store_scan(library.path(), &scan)
        .unwrap();
    let reopened = Index::open(&database).unwrap();
    assert_eq!(
        reopened
            .albums()
            .unwrap()
            .iter()
            .map(|album| album.path.clone())
            .collect::<Vec<_>>(),
        reported
    );
    for (relative, (bytes, modified)) in files.iter().zip(before) {
        let path = library.path().join(relative);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(fs::metadata(path).unwrap().modified().unwrap(), modified);
    }
    assert_eq!(fs::read_dir(library.path()).unwrap().count(), 2);
}

#[test]
fn removing_one_root_hides_its_library_but_retains_user_work_when_readded() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    music(first.path(), "套装/专辑甲");
    music(second.path(), "专辑乙");
    let album = first.path().join("套装/专辑甲");
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(first.path(), &scan_root(first.path(), &mut NoTags))
        .unwrap();
    index
        .store_scan(second.path(), &scan_root(second.path(), &mut NoTags))
        .unwrap();
    index
        .set_user_edit(&album, "title", "我整理过的专辑")
        .unwrap();
    index.add_user_tag(&album, "收藏").unwrap();
    let collection = index.create_manual_collection("周末").unwrap();
    index.add_to_manual_collection(collection, &album).unwrap();

    index.remove_root(first.path()).unwrap();
    assert_eq!(index.roots().unwrap().len(), 1);
    assert_eq!(index.albums().unwrap().len(), 1);
    assert_eq!(
        index.albums().unwrap()[0].path,
        second.path().join("专辑乙")
    );
    assert!(index.collections().unwrap().is_empty());
    assert!(
        index
            .issues()
            .unwrap()
            .iter()
            .all(|(path, _, _)| !path.starts_with(first.path()))
    );

    index
        .store_scan(first.path(), &scan_root(first.path(), &mut NoTags))
        .unwrap();
    let albums = index.albums().unwrap();
    let restored = albums.iter().find(|row| row.path == album).unwrap();
    assert_eq!(restored.display_title(), "我整理过的专辑");
    assert_eq!(index.user_tags(&album).unwrap(), ["收藏"]);
    assert_eq!(index.manual_collection_albums(collection).unwrap(), [album]);
    assert_eq!(
        fs::read(first.path().join("套装/专辑甲/01.flac")).unwrap(),
        b"audio fixture"
    );
}

#[test]
fn partial_rescan_updates_found_albums_without_losing_unvisited_albums_or_proposing_renames() {
    let library = tempfile::tempdir().unwrap();
    music(library.path(), "套装/甲");
    music(library.path(), "套装/乙");
    let mut index = Index::open_in_memory().unwrap();
    let original = scan_root(library.path(), &mut NoTags);
    index.store_scan(library.path(), &original).unwrap();
    let old_path = library.path().join("套装/甲");
    index.set_user_edit(&old_path, "title", "我的甲").unwrap();
    fs::rename(&old_path, library.path().join("套装/丙")).unwrap();
    let mut partial = scan_root(library.path(), &mut NoTags);
    partial.cancelled = true;
    partial.albums.retain(|album| album.path.ends_with("丙"));
    index.store_partial_scan(library.path(), &partial).unwrap();
    let albums = index.albums().unwrap();
    assert_eq!(albums.len(), 3);
    assert!(albums.iter().any(|album| album.display_title() == "我的甲"));
    assert!(albums.iter().any(|album| album.path.ends_with("乙")));
    assert!(albums.iter().any(|album| album.path.ends_with("丙")));
    assert!(index.rename_candidates().unwrap().is_empty());
    let issues = index.issues().unwrap();
    index.store_partial_scan(library.path(), &partial).unwrap();
    assert_eq!(index.albums().unwrap().len(), 3);
    assert_eq!(index.issues().unwrap(), issues);
    // Completing a later scan must still compare against the last complete
    // library, not the new paths already shown during the partial scan.
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    let candidates = index.rename_candidates().unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].old_path, old_path);
    assert_eq!(candidates[0].new_path, library.path().join("套装/丙"));
}

#[test]
fn same_size_audio_changed_within_one_second_is_read_again() {
    use std::time::{Duration, UNIX_EPOCH};
    use voon_core::db::CachedTags;
    let library = tempfile::tempdir().unwrap();
    music(library.path(), "专辑");
    let audio = library.path().join("专辑/01.flac");
    let file = fs::OpenOptions::new().write(true).open(&audio).unwrap();
    file.set_times(
        fs::FileTimes::new().set_modified(UNIX_EPOCH + Duration::new(1_700_000_000, 100_000_000)),
    )
    .unwrap();
    let index = Index::open_in_memory().unwrap();
    let mut tags = CachedTags::new(&index, NoTags);
    scan_root(library.path(), &mut tags);
    assert_eq!(tags.reads, 1);
    file.set_times(
        fs::FileTimes::new().set_modified(UNIX_EPOCH + Duration::new(1_700_000_000, 200_000_000)),
    )
    .unwrap();
    let mut tags = CachedTags::new(&index, NoTags);
    scan_root(library.path(), &mut tags);
    assert_eq!(tags.reads, 1);
}
