use std::path::PathBuf;
use voon_core::{db::Index, scan::resolve_metadata, tags::TagInfo, *};

#[test]
fn classical_metadata_uses_tag_or_cue_composers_and_distinct_track_performers() {
    let sheet = cue::parse_str(
        "REM COMPOSER \"巴赫\"\nPERFORMER \"乐团\"\nFILE \"whole.flac\" WAVE\n TRACK 01 AUDIO\n PERFORMER \"独奏甲\"\n INDEX 01 00:00:00\n TRACK 02 AUDIO\n PERFORMER \"独奏乙\"\n INDEX 01 01:00:00\n TRACK 03 AUDIO\n PERFORMER \"独奏甲\"\n INDEX 01 02:00:00\n",
    );
    let hints = folder_name::parse("古典专辑");
    let cue_metadata = resolve_metadata(&hints, std::slice::from_ref(&sheet), &[]);
    assert_eq!(
        cue_metadata.composer,
        Some(Field::new("巴赫".into(), FieldSource::Cue))
    );
    assert_eq!(
        cue_metadata.performers,
        Some(Field::new("独奏甲; 独奏乙".into(), FieldSource::Cue))
    );
    let tags: Vec<_> = ["独奏甲", "独奏乙", "独奏甲"]
        .into_iter()
        .enumerate()
        .map(|(number, artist)| {
            (
                PathBuf::from(format!("{number}.flac")),
                TagInfo {
                    composer: Some("莫扎特".into()),
                    artist: Some(artist.into()),
                    ..TagInfo::default()
                },
            )
        })
        .collect();
    let tag_metadata = resolve_metadata(&hints, &[sheet], &tags);
    assert_eq!(
        tag_metadata.composer,
        Some(Field::new("莫扎特".into(), FieldSource::Tag))
    );
    assert_eq!(
        tag_metadata.performers,
        Some(Field::new("独奏甲; 独奏乙".into(), FieldSource::Tag))
    );

    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(
            std::path::Path::new("/假库"),
            &ScanResult {
                albums: vec![Album {
                    source_directories: vec![],
                    path: "/假库/古典".into(),
                    folder_name: "古典".into(),
                    metadata: tag_metadata,
                    discs: vec![],
                    cover: None,
                    collection: None,
                    signature: "classical".into(),
                }],
                ..ScanResult::default()
            },
        )
        .unwrap();
    assert_eq!(index.albums_by_composer("莫扎特").unwrap().len(), 1);
    assert!(index.albums_by_composer("巴赫").unwrap().is_empty());
}
