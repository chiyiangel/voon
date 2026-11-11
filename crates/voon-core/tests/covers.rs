use voon_core::{
    db::Index,
    scan::{NoTags, scan_root},
    *,
};

fn library() -> tempfile::TempDir {
    use id3::TagLike;
    let directory = tempfile::tempdir().unwrap();
    for relative in [
        "专辑/cover.jpg",
        "专辑/CD1/folder.jpg",
        "专辑/Booklet/back.png",
    ] {
        let path = directory.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"image fixture").unwrap();
    }
    let mut tag = id3::Tag::new();
    for (description, data, picture_type) in [
        ("正面", vec![1, 2, 3], id3::frame::PictureType::CoverFront),
        ("背面", vec![4, 5, 6], id3::frame::PictureType::CoverBack),
    ] {
        tag.add_frame(id3::frame::Picture {
            mime_type: "image/jpeg".into(),
            picture_type,
            description: description.into(),
            data,
        });
    }
    let mut bytes = vec![0; 80];
    bytes[..4].copy_from_slice(b"DSD ");
    bytes[20..28].copy_from_slice(&80u64.to_le_bytes());
    bytes[28..32].copy_from_slice(b"fmt ");
    tag.write_to(&mut bytes, id3::Version::Id3v24).unwrap();
    std::fs::write(directory.path().join("专辑/CD1/01.dsf"), bytes).unwrap();
    directory
}

#[test]
fn every_directory_and_embedded_candidate_is_selectable_and_survives_rescans() {
    let library = library();
    let scan = scan_root(library.path(), &mut NoTags);
    let album = &scan.albums[0];
    let candidates = voon_core::covers::candidates(album, &[]).unwrap();
    assert_eq!(candidates.len(), 5);
    assert!(candidates.iter().any(|candidate| candidate.cover.source
        == CoverSource::File(album.path.join("Booklet/back.png"))));
    let embedded: Vec<_> = candidates
        .iter()
        .filter(|candidate| matches!(candidate.cover.source, CoverSource::EmbeddedPicture { .. }))
        .collect();
    assert_eq!(embedded.len(), 2);
    assert_eq!(tags::read_cover(&embedded[1].cover).unwrap(), [4, 5, 6]);

    let mut index = Index::open_in_memory().unwrap();
    index.store_scan(library.path(), &scan).unwrap();
    index
        .set_user_cover(&album.path, &embedded[1].cover)
        .unwrap();
    assert_eq!(
        index.albums().unwrap()[0].cover,
        Some(embedded[1].cover.clone())
    );
    assert_eq!(
        index.album(&album.path).unwrap().unwrap().cover,
        Some(embedded[1].cover.clone())
    );
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    assert_eq!(
        index.albums().unwrap()[0].cover,
        Some(embedded[1].cover.clone())
    );
    index.clear_user_cover(&album.path).unwrap();
    assert_eq!(index.albums().unwrap()[0].cover, album.cover);
}

#[test]
fn confirming_a_rename_moves_the_user_cover_and_rebases_its_file_path() {
    let library = library();
    let old = library.path().join("专辑");
    let new = library.path().join("改名后");
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    index
        .set_user_cover(
            &old,
            &Cover {
                source: CoverSource::File(old.join("Booklet/back.png")),
            },
        )
        .unwrap();
    std::fs::rename(&old, &new).unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    index.confirm_rename(&old, &new).unwrap();
    assert_eq!(
        index.user_cover(&new).unwrap(),
        Some(Cover {
            source: CoverSource::File(new.join("Booklet/back.png"))
        })
    );
    assert!(index.user_cover(&old).unwrap().is_none());
}

#[test]
fn image_discovery_honors_ignores_and_cancellation_and_skips_symbolic_links() {
    use std::sync::atomic::AtomicBool;
    let library = library();
    let scan = scan_root(library.path(), &mut NoTags);
    let album = &scan.albums[0];
    std::fs::write(album.path.join(".hidden.jpg"), b"hidden").unwrap();
    std::fs::create_dir(album.path.join("另一专辑")).unwrap();
    std::fs::write(album.path.join("另一专辑/cover.jpg"), b"outside scope").unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(album.path.join("cover.jpg"), album.path.join("linked.jpg"))
            .unwrap();
        std::os::unix::fs::symlink(album.path.join("Booklet"), album.path.join("Artwork")).unwrap();
    }
    let images = voon_core::covers::discover_images(
        album,
        &[album.path.join("Booklet")],
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(images.len(), 2);
    assert!(images.contains(&album.path.join("cover.jpg")));
    assert!(images.contains(&album.path.join("CD1/folder.jpg")));
    assert!(
        voon_core::covers::discover_images(album, &[], &AtomicBool::new(true))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn structurally_merged_albums_keep_directory_and_booklet_cover_candidates() {
    let library = library();
    let first = library.path().join("专辑");
    let second = library.path().join("另一张专辑");
    std::fs::create_dir(&second).unwrap();
    std::fs::write(second.join("01.flac"), b"audio fixture").unwrap();
    std::fs::write(second.join("cover.jpg"), b"image fixture").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    let merged = index
        .merge_albums(&[first.clone(), second.clone()], &first, &[])
        .unwrap();
    let album = index.album(&merged).unwrap().unwrap();

    let candidates = voon_core::covers::candidates(&album, &[]).unwrap();

    for image in [
        first.join("cover.jpg"),
        first.join("Booklet/back.png"),
        second.join("cover.jpg"),
    ] {
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.cover.source == CoverSource::File(image.clone())),
            "合并后应保留候选封面：{}",
            image.display()
        );
    }
}
