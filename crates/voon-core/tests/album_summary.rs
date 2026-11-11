use std::path::Path;

use voon_core::{db::Index, *};

#[test]
fn index_returns_album_card_summary_with_user_title_and_disc_and_track_counts() {
    let root = Path::new("/假音乐库");
    let path = root.join("双碟");
    let track = |number| Track {
        number: Some(number),
        title: None,
        artist: None,
        format: AudioFormat::Flac,
        source: TrackSource::File {
            path: path.join(format!("{number}.flac")),
        },
        properties: AudioProperties::default(),
    };
    let cover = Cover {
        source: CoverSource::File(path.join("cover.jpg")),
    };
    let album = Album {
        source_directories: vec![],
        path: path.clone(),
        folder_name: "双碟".into(),
        discs: vec![
            Disc {
                number: 1,
                path: path.join("CD1"),
                tracks: vec![track(1), track(2)],
            },
            Disc {
                number: 2,
                path: path.join("CD2"),
                tracks: vec![track(3)],
            },
        ],
        cover: Some(cover.clone()),
        metadata: AlbumMetadata {
            title: Some(Field::new("自动标题".into(), FieldSource::Tag)),
            album_artist: Some(Field::new("阿格里奇".into(), FieldSource::Tag)),
            ..AlbumMetadata::default()
        },
        collection: None,
        signature: "summary-fixture".into(),
    };
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(
            root,
            &ScanResult {
                albums: vec![album],
                ..ScanResult::default()
            },
        )
        .unwrap();
    index.set_user_edit(&path, "title", "我的专辑标题").unwrap();

    let summaries = index.albums().unwrap();
    let summary = &summaries[0];
    assert_eq!(summary.display_title(), "我的专辑标题");
    assert_eq!(
        summary.metadata.album_artist.as_ref().unwrap().value,
        "阿格里奇"
    );
    assert_eq!(summary.cover, Some(cover));
    assert_eq!(summary.disc_count, 2);
    assert_eq!(summary.track_count, 3);
}
