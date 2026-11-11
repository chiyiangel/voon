use std::{fs, path::Path};

use voon_core::{
    db::Index,
    scan::{NoTags, scan_root},
};

fn audio(root: &Path, relative: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, relative.as_bytes()).unwrap();
}

#[cfg(unix)]
struct DeniedDirectory(std::path::PathBuf, fs::Permissions);

#[cfg(unix)]
impl DeniedDirectory {
    fn new(path: &Path) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let restore = Self(path.into(), fs::metadata(path).unwrap().permissions());
        fs::set_permissions(path, fs::Permissions::from_mode(0)).unwrap();
        assert_eq!(
            fs::read_dir(path).unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied,
            "this test must exercise a real directory read denial"
        );
        restore
    }
}

#[cfg(unix)]
impl Drop for DeniedDirectory {
    fn drop(&mut self) {
        fs::set_permissions(&self.0, self.1.clone()).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn unreadable_disc_preserves_the_complete_album_while_independent_albums_converge() {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let root = library.path();
    for file in [
        "套装/甲/CD1/01.flac",
        "套装/甲/CD2/01.flac",
        "套装/乙/旧曲.flac",
        "套装/已移除/01.flac",
    ] {
        audio(root, file);
    }
    let album = root.join("套装/甲");
    let database = local.path().join("index.sqlite3");
    let mut index = Index::open(&database).unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    index
        .set_user_edit(&album, "title", "我的完整专辑")
        .unwrap();
    index.add_user_tag(&album, "收藏").unwrap();
    let manual = index.create_manual_collection("精选").unwrap();
    index.add_to_manual_collection(manual, &album).unwrap();

    fs::remove_file(root.join("套装/乙/旧曲.flac")).unwrap();
    audio(root, "套装/乙/新曲.flac");
    fs::remove_dir_all(root.join("套装/已移除")).unwrap();
    audio(root, "套装/新增/01.flac");
    let denied = DeniedDirectory::new(&album.join("CD2"));
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    drop(index);

    let mut index = Index::open(&database).unwrap();
    let saved = index.album(&album).unwrap().unwrap();
    assert_eq!(saved.discs.len(), 2);
    assert_eq!(saved.tracks().count(), 2);
    assert_eq!(saved.display_title(), "我的完整专辑");
    assert_eq!(index.user_tags(&album).unwrap(), ["收藏"]);
    assert_eq!(
        index.manual_collection_albums(manual).unwrap(),
        [album.clone()]
    );
    assert_eq!(index.albums().unwrap().len(), 3);
    assert!(index.album(&root.join("套装/已移除")).unwrap().is_none());
    let updated = index.album(&root.join("套装/乙")).unwrap().unwrap();
    assert_eq!(
        updated.tracks().next().unwrap().source.audio_path(),
        &root.join("套装/乙/新曲.flac")
    );
    assert_eq!(index.collection_tree().unwrap()[0].albums.len(), 3);
    assert!(index.attention_items().unwrap().iter().any(|item| {
        item.path == album.join("CD2") && item.album_path.as_ref() == Some(&album)
    }));

    drop(denied);
    audio(root, "套装/甲/CD2/02.flac");
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    assert_eq!(index.album(&album).unwrap().unwrap().tracks().count(), 3);
    assert!(
        !index
            .attention_items()
            .unwrap()
            .iter()
            .any(|item| { item.path == album.join("CD2") })
    );
}

#[cfg(unix)]
#[test]
fn first_discovery_with_an_unreadable_cue_is_attention_without_a_partial_album() {
    use std::os::unix::fs::PermissionsExt;
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    audio(library.path(), "首次发现/image.flac");
    let cue = library.path().join("首次发现/image.cue");
    fs::write(&cue, "FILE \"image.flac\" WAVE\n TRACK 01 AUDIO\n INDEX 01 00:00:00\n TRACK 02 AUDIO\n INDEX 01 03:00:00\n").unwrap();
    let restore = DeniedDirectory(cue.clone(), fs::metadata(&cue).unwrap().permissions());
    fs::set_permissions(&cue, fs::Permissions::from_mode(0)).unwrap();
    assert_eq!(
        fs::read(&cue).unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    let mut reported = Vec::new();
    let scan = voon_core::scan_root_with(
        library.path(),
        &mut NoTags,
        &std::sync::atomic::AtomicBool::new(false),
        &mut |event| {
            if let voon_core::scan::ScanEvent::Album(album) = event {
                reported.push(album.path.clone());
            }
        },
    );
    assert!(scan.albums.is_empty());
    assert!(reported.is_empty());
    let database = local.path().join("index.sqlite3");
    Index::open(&database)
        .unwrap()
        .store_scan(library.path(), &scan)
        .unwrap();
    let index = Index::open(&database).unwrap();
    assert!(index.albums().unwrap().is_empty());
    let issues = index.attention_items().unwrap();
    let issue = issues.iter().find(|item| item.path == cue).unwrap();
    assert!(issue.explanation.contains("分轨表"));
    assert!(issue.explanation.contains("Permission denied"));
    assert!(issue.album_path.is_none());
    drop(restore);
}

#[test]
fn an_empty_cue_does_not_replace_previously_accepted_virtual_tracks() {
    let library = tempfile::tempdir().unwrap();
    audio(library.path(), "专辑/image.flac");
    let album = library.path().join("专辑");
    let cue = album.join("image.cue");
    fs::write(&cue, "FILE \"image.flac\" WAVE\n TRACK 01 AUDIO\n INDEX 01 00:00:00\n TRACK 02 AUDIO\n INDEX 01 03:00:00\n").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    fs::write(&cue, "FILE \"image.flac\" WAVE\n").unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    let saved = index.album(&album).unwrap().unwrap();
    assert_eq!(saved.tracks().count(), 2);
    assert!(
        saved
            .tracks()
            .all(|track| matches!(track.source, voon_core::TrackSource::Virtual { .. }))
    );
}
