use voon_core::{
    AudioFormat, AudioProperties,
    facets::Resolution,
    scan::{FileStamp, TagReader},
    tags::TagInfo,
};
use voon_core::{
    db::Index,
    scan::{NoTags, scan_root},
};

#[test]
fn album_addition_order_survives_rescans_and_reopening_the_index() {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let root = library.path();
    std::fs::create_dir_all(root.join("Philips Duo/Schubert/CD1")).unwrap();
    std::fs::write(root.join("Philips Duo/Schubert/CD1/01.flac"), b"fixture").unwrap();
    let database = local.path().join("library.sqlite3");
    let mut index = Index::open(&database).unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    let original = index.albums().unwrap()[0].added_order;
    assert!(original > 0);

    std::fs::create_dir_all(root.join("Alfred_Brendel/Mozart/Disc 1")).unwrap();
    std::fs::write(root.join("Alfred_Brendel/Mozart/Disc 1/01.wv"), b"fixture").unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    drop(index);
    let index = Index::open(&database).unwrap();
    let albums = index.albums().unwrap();
    assert_eq!(
        albums
            .iter()
            .find(|album| album.folder_name == "Schubert")
            .unwrap()
            .added_order,
        original
    );
    assert!(
        albums
            .iter()
            .find(|album| album.folder_name == "Mozart")
            .unwrap()
            .added_order
            > original
    );
}

#[test]
fn browse_rows_expose_all_formats_and_album_resolution_from_tracks() {
    struct Tags;
    impl TagReader for Tags {
        fn read(&mut self, _: &std::path::Path, _: AudioFormat, _: FileStamp) -> Option<TagInfo> {
            Some(TagInfo {
                properties: AudioProperties {
                    sample_rate: Some(96_000),
                    bit_depth: Some(24),
                    ..AudioProperties::default()
                },
                ..TagInfo::default()
            })
        }
    }
    let library = tempfile::tempdir().unwrap();
    let root = library.path();
    std::fs::create_dir_all(root.join("Philips Duo/Schubert/CD1")).unwrap();
    for file in ["01.flac", "02.wv"] {
        std::fs::write(root.join("Philips Duo/Schubert/CD1").join(file), b"fixture").unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    index.store_scan(root, &scan_root(root, &mut Tags)).unwrap();
    let albums = index.albums().unwrap();
    assert_eq!(
        albums[0].facets.formats.iter().copied().collect::<Vec<_>>(),
        [AudioFormat::Flac, AudioFormat::WavPack]
    );
    assert_eq!(albums[0].facets.resolution, Resolution::HiRes);
    assert_eq!(albums[0].facets.max_sample_rate, Some(96_000));
    assert_eq!(albums[0].facets.max_bit_depth, Some(24));
}
