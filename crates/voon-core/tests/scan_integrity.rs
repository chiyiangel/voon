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
        fs::set_permissions(path, fs::Permissions::from_mode(0o000)).unwrap();
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
        index.manual_collection_albums(manual).unwrap().as_slice(),
        std::slice::from_ref(&album)
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
    fs::set_permissions(&cue, fs::Permissions::from_mode(0o000)).unwrap();
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

#[test]
fn committing_a_cancelled_scan_preserves_unvisited_albums_and_the_complete_rename_baseline() {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    audio(library.path(), "原路径/01.flac");
    audio(library.path(), "未访问/01.flac");
    let root = library.path();
    let database = local.path().join("index.sqlite3");
    let mut index = Index::open(&database).unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    let old = root.join("原路径");
    let new = root.join("新路径");
    index.set_user_edit(&old, "title", "保留的整理").unwrap();
    fs::rename(&old, &new).unwrap();
    let mut cancelled = scan_root(root, &mut NoTags);
    cancelled.cancelled = true;
    cancelled.albums.retain(|album| album.path == new);
    index.store_scan(root, &cancelled).unwrap();
    drop(index);
    let mut index = Index::open(&database).unwrap();
    assert_eq!(index.albums().unwrap().len(), 3);
    assert!(index.rename_candidates().unwrap().is_empty());
    assert_eq!(
        index.album(&old).unwrap().unwrap().display_title(),
        "保留的整理"
    );
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    let candidates = index.rename_candidates().unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].old_path, old);
    assert_eq!(candidates[0].new_path, new);
}

#[test]
fn cancellation_before_a_child_is_observed_does_not_publish_its_parent_album() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let library = tempfile::tempdir().unwrap();
    audio(library.path(), "专辑/01.flac");
    audio(library.path(), "专辑/未访问/02.flac");
    let cancel = AtomicBool::new(false);
    let mut reported = Vec::new();
    let scan =
        voon_core::scan_root_with(
            library.path(),
            &mut NoTags,
            &cancel,
            &mut |event| match event {
                voon_core::scan::ScanEvent::Directory(path)
                    if path == library.path().join("专辑/未访问") =>
                {
                    cancel.store(true, Ordering::Relaxed);
                }
                voon_core::scan::ScanEvent::Album(album) => reported.push(album.path.clone()),
                _ => {}
            },
        );
    assert!(scan.cancelled);
    assert!(scan.albums.is_empty());
    assert!(reported.is_empty());
}

#[test]
fn an_independently_removed_rename_destination_expires_even_when_another_album_fails() {
    let library = tempfile::tempdir().unwrap();
    let root = library.path();
    audio(root, "旧路径/01.flac");
    audio(root, "另一张/02.flac");
    let old = root.join("旧路径");
    let new = root.join("新路径");
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    index.set_user_edit(&old, "title", "保留的编辑").unwrap();
    fs::rename(&old, &new).unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    assert_eq!(index.rename_candidates().unwrap().len(), 1);
    fs::remove_dir_all(&new).unwrap();
    let mut scan = scan_root(root, &mut NoTags);
    scan.albums.clear();
    scan.issues.push(voon_core::scan::ScanIssue {
        path: root.join("另一张"),
        kind: voon_core::scan::IssueKind::Unreadable {
            error: "暂时离线".into(),
        },
    });
    index.store_scan(root, &scan).unwrap();
    assert!(index.rename_candidates().unwrap().is_empty());
    assert!(index.confirm_rename(&old, &new).is_err());
    audio(root, "旧路径/01.flac");
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    assert_eq!(
        index.album(&old).unwrap().unwrap().display_title(),
        "保留的编辑"
    );
}

#[test]
fn a_cancelled_retry_clears_only_the_failures_of_albums_it_completely_reobserved() {
    let library = tempfile::tempdir().unwrap();
    let root = library.path();
    audio(root, "已恢复/01.flac");
    audio(root, "未访问/02.flac");
    let recovered = root.join("已恢复");
    let unvisited = root.join("未访问");
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    let mut failed = voon_core::ScanResult::default();
    for path in [&recovered, &unvisited] {
        failed.issues.push(voon_core::scan::ScanIssue {
            path: path.clone(),
            kind: voon_core::scan::IssueKind::Unreadable {
                error: "暂时断连".into(),
            },
        });
    }
    index.store_scan(root, &failed).unwrap();
    let mut retry = scan_root(root, &mut NoTags);
    retry.cancelled = true;
    retry.albums.retain(|album| album.path == recovered);
    index.store_partial_scan(root, &retry).unwrap();
    assert!(index.album_scan_issues(&recovered).unwrap().is_empty());
    assert_eq!(index.album_scan_issues(&unvisited).unwrap().len(), 1);
    assert_eq!(index.albums().unwrap().len(), 2);
}

#[test]
fn returning_some_albums_without_root_coverage_cannot_confirm_missing_content() {
    let library = tempfile::tempdir().unwrap();
    let root = library.path();
    audio(root, "保留/01.flac");
    audio(root, "本次完成/02.flac");
    let full = scan_root(root, &mut NoTags);
    let mut index = Index::open_in_memory().unwrap();
    index.store_scan(root, &full).unwrap();
    let scan = voon_core::ScanResult {
        albums: full
            .albums
            .into_iter()
            .filter(|album| album.path == root.join("本次完成"))
            .collect(),
        ..Default::default()
    };
    index.store_scan(root, &scan).unwrap();
    assert_eq!(index.albums().unwrap().len(), 2);
    assert!(index.album(&root.join("保留")).unwrap().is_some());
    assert!(index.rename_candidates().unwrap().is_empty());
}

#[test]
fn partial_retries_keep_each_current_cue_failure_without_accumulating_old_attempts() {
    let library = tempfile::tempdir().unwrap();
    let root = library.path();
    audio(root, "专辑/other.flac");
    let cue = root.join("专辑/broken.cue");
    let mut index = Index::open_in_memory().unwrap();
    for names in [["first.wav", "second.wav"], ["third.wav", "fourth.wav"]] {
        fs::write(&cue, format!("FILE \"{}\" WAVE\n TRACK 01 AUDIO\n INDEX 01 00:00:00\nFILE \"{}\" WAVE\n TRACK 02 AUDIO\n INDEX 01 00:00:00\n", names[0], names[1])).unwrap();
        index
            .store_partial_scan(root, &scan_root(root, &mut NoTags))
            .unwrap();
        let issues = index.attention_items().unwrap();
        let missing: Vec<_> = issues
            .iter()
            .filter(|item| item.kind == "CueTargetMissing")
            .collect();
        assert_eq!(missing.len(), 2);
        assert!(
            names
                .iter()
                .all(|name| missing.iter().any(|item| item.explanation.contains(name)))
        );
    }
}

#[cfg(unix)]
#[test]
fn an_unavailable_album_link_does_not_block_an_independent_album_update() {
    let library = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    audio(source.path(), "原目录/01.flac");
    let root = library.path();
    std::os::unix::fs::symlink(source.path().join("原目录"), root.join("链接专辑")).unwrap();
    audio(root, "正常专辑/旧曲.flac");
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    fs::rename(source.path().join("原目录"), source.path().join("离线目录")).unwrap();
    fs::remove_file(root.join("正常专辑/旧曲.flac")).unwrap();
    audio(root, "正常专辑/新曲.flac");
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    let updated = index.album(&root.join("正常专辑")).unwrap().unwrap();
    assert_eq!(
        updated.tracks().next().unwrap().source.audio_path(),
        &root.join("正常专辑/新曲.flac")
    );
    assert!(index.album(&root.join("链接专辑")).unwrap().is_some());
    assert!(
        index
            .attention_items()
            .unwrap()
            .iter()
            .any(|item| item.path == root.join("链接专辑"))
    );
}
