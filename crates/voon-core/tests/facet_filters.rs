use std::path::Path;
use voon_core::{
    AudioFormat,
    facets::{self, Language, Resolution},
    scan::{FileStamp, TagReader, scan_root},
    tags::TagInfo,
};

struct Tags;
impl TagReader for Tags {
    fn read(&mut self, path: &Path, _: AudioFormat, _: FileStamp) -> Option<TagInfo> {
        path.parent()?.ends_with("标签流派").then(|| TagInfo {
            genre: Some("古典".into()),
            ..TagInfo::default()
        })
    }
}

#[test]
fn nas_shaped_album_facets_use_directory_specs_and_only_tag_genres() {
    let library = tempfile::tempdir().unwrap();
    for relative in [
        "无标签 [DSD64]/01.dsf",
        "国语 (1983) [24-96]/01.flac",
        "标签流派/01.flac",
        "线索流派/01.flac",
    ] {
        let path = library.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"fake").unwrap();
    }
    std::fs::write(
        library.path().join("线索流派/album.cue"),
        "REM GENRE 流行\nFILE \"01.flac\" WAVE\n TRACK 01 AUDIO\n INDEX 01 00:00:00\n",
    )
    .unwrap();
    let scan = scan_root(library.path(), &mut Tags);
    let dsd = facets::facets(
        scan.albums
            .iter()
            .find(|album| album.folder_name.contains("DSD64"))
            .unwrap(),
    );
    assert_eq!(dsd.resolution, Resolution::Dsd(64));
    let hires = facets::facets(
        scan.albums
            .iter()
            .find(|album| album.folder_name.starts_with("国语"))
            .unwrap(),
    );
    assert_eq!(hires.resolution, Resolution::HiRes);
    assert_eq!(hires.max_sample_rate, Some(96_000));
    assert_eq!(hires.max_bit_depth, Some(24));
    assert_eq!(hires.language, Language::Chinese);
    assert_eq!(hires.decade, Some(1980));
    assert_eq!(
        facets::facets(
            scan.albums
                .iter()
                .find(|album| album.folder_name == "标签流派")
                .unwrap()
        )
        .genre
        .as_deref(),
        Some("古典")
    );
    assert!(
        facets::facets(
            scan.albums
                .iter()
                .find(|album| album.folder_name == "线索流派")
                .unwrap()
        )
        .genre
        .is_none()
    );
    let mut index = voon_core::db::Index::open_in_memory().unwrap();
    index.store_scan(library.path(), &scan).unwrap();
    index
        .set_user_edit(&library.path().join("标签流派"), "genre", "手动流派")
        .unwrap();
    let rows = index.albums().unwrap();
    assert_eq!(
        rows.iter()
            .find(|album| album.folder_name == "标签流派")
            .unwrap()
            .facets
            .genre
            .as_deref(),
        Some("古典")
    );
}
