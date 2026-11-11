use std::path::Path;

use voon_core::{
    AudioFormat, AudioProperties, Field, FieldSource, TrackSource,
    db::Index,
    scan::{DiskTags, FileStamp, TagReader, scan_root},
    tags::TagInfo,
};

struct FixtureTags;
impl TagReader for FixtureTags {
    fn read(&mut self, _: &Path, format: AudioFormat, _: FileStamp) -> Option<TagInfo> {
        Some(TagInfo {
            title: (format == AudioFormat::Flac).then(|| "Andante".into()),
            genre: Some("Classical".into()),
            year: Some(1999),
            properties: AudioProperties {
                sample_rate: Some(96_000),
                bit_depth: Some(24),
                channels: Some(2),
                duration_ms: Some(360_000),
            },
            ..TagInfo::default()
        })
    }
}

#[test]
fn cue_boundaries_and_performers_survive_reopening_when_dff_tags_are_unsupported() {
    let library = tempfile::tempdir().unwrap();
    let album_path = library.path().join("双演奏者");
    std::fs::create_dir(&album_path).unwrap();
    let image = album_path.join("disc.dff");
    let cue = album_path.join("disc.cue");
    let audio_bytes = b"DFF placeholder: properties deliberately unsupported";
    let cue_bytes = b"TITLE \"Two performers\"\nFILE \"disc.dff\" WAVE\n TRACK 01 AUDIO\n  TITLE \"Allegro\"\n  PERFORMER \"First performer\"\n  INDEX 01 00:00:00\n TRACK 02 AUDIO\n  TITLE \"Andante\"\n  PERFORMER \"Second performer\"\n  INDEX 01 03:00:00\n";
    std::fs::write(&image, audio_bytes).unwrap();
    std::fs::write(&cue, cue_bytes).unwrap();
    let before = [FileStamp::of(&image), FileStamp::of(&cue)];
    let storage = tempfile::tempdir().unwrap();
    let database = storage.path().join("index.sqlite");
    {
        let mut index = Index::open(&database).unwrap();
        index
            .store_scan(library.path(), &scan_root(library.path(), &mut DiskTags))
            .unwrap();
    }

    let index = Index::open(&database).unwrap();
    let album = index.album(&album_path).unwrap().unwrap();
    let tracks = &album.discs[0].tracks;
    assert_eq!(
        tracks
            .iter()
            .map(|track| track.properties.duration_ms)
            .collect::<Vec<_>>(),
        vec![Some(180_000), None]
    );
    assert_eq!(
        tracks
            .iter()
            .map(|track| track.artist.clone())
            .collect::<Vec<_>>(),
        vec![
            Some(Field::new("First performer".into(), FieldSource::Cue)),
            Some(Field::new("Second performer".into(), FieldSource::Cue)),
        ]
    );
    assert_eq!(
        tracks[0].source,
        TrackSource::Virtual {
            image: image.clone(),
            cue: cue.clone(),
            start: 0,
            end: Some(13_500),
        }
    );
    assert_eq!(
        tracks[1].source,
        TrackSource::Virtual {
            image: image.clone(),
            cue: cue.clone(),
            start: 13_500,
            end: None,
        }
    );
    assert_eq!([FileStamp::of(&image), FileStamp::of(&cue)], before);
    assert_eq!(std::fs::read(image).unwrap(), audio_bytes);
    assert_eq!(std::fs::read(cue).unwrap(), cue_bytes);
}

#[test]
fn querying_an_album_preserves_discs_virtual_tracks_properties_and_field_sources() {
    let library = tempfile::tempdir().unwrap();
    let album_path = library.path().join("Alfred Brendel/Philips Duo");
    std::fs::create_dir_all(album_path.join("CD1")).unwrap();
    std::fs::create_dir_all(album_path.join("CD2")).unwrap();
    std::fs::create_dir_all(album_path.join("Booklet")).unwrap();
    std::fs::write(album_path.join("CD1/CDImage.ape"), b"fixture").unwrap();
    std::fs::write(album_path.join("CD2/01.flac"), b"fixture").unwrap();
    std::fs::write(album_path.join("cover.jpg"), b"cover fixture").unwrap();
    let cue = "PERFORMER \"阿尔弗雷德布伦德尔\"\nTITLE \"贝多芬钢琴奏鸣曲\"\nFILE \"CDImage.wav\" WAVE\n  TRACK 01 AUDIO\n    TITLE \"第一乐章快板\"\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    TITLE \"第二乐章行板\"\n    INDEX 01 03:00:00\n";
    let (bytes, _, errors) = encoding_rs::GBK.encode(cue);
    assert!(!errors);
    std::fs::write(album_path.join("CD1/program.cue"), bytes).unwrap();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut FixtureTags))
        .unwrap();
    index
        .set_user_edit(&album_path, "title", "我的贝多芬")
        .unwrap();

    let album = index.album(&album_path).unwrap().unwrap();

    assert_eq!(album.display_title(), "我的贝多芬");
    assert_eq!(
        album.metadata.title.as_ref().unwrap().source,
        FieldSource::UserEdit
    );
    assert_eq!(
        album.metadata.album_artist,
        Some(Field::new("阿尔弗雷德布伦德尔".into(), FieldSource::Cue))
    );
    assert_eq!(
        album.metadata.genre,
        Some(Field::new("Classical".into(), FieldSource::Tag))
    );
    assert_eq!(
        album.collection,
        Some(library.path().join("Alfred Brendel"))
    );
    assert_eq!(
        album
            .discs
            .iter()
            .map(|disc| disc.number)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    let tracks = &album.discs[0].tracks;
    assert_eq!(tracks.len(), 2);
    assert_eq!(
        tracks[0].title,
        Some(Field::new("第一乐章快板".into(), FieldSource::Cue))
    );
    assert_eq!(tracks[0].format, AudioFormat::Ape);
    assert_eq!(
        tracks[0].source,
        TrackSource::Virtual {
            image: album_path.join("CD1/CDImage.ape"),
            cue: album_path.join("CD1/program.cue"),
            start: 0,
            end: Some(13_500),
        }
    );
    assert_eq!(tracks[1].title.as_ref().unwrap().value, "第二乐章行板");
    assert_eq!(
        tracks[1].source,
        TrackSource::Virtual {
            image: album_path.join("CD1/CDImage.ape"),
            cue: album_path.join("CD1/program.cue"),
            start: 13_500,
            end: None,
        }
    );
    assert_eq!(
        tracks[1].properties,
        AudioProperties {
            sample_rate: Some(96_000),
            bit_depth: Some(24),
            channels: Some(2),
            duration_ms: Some(180_000)
        }
    );
    let file = &album.discs[1].tracks[0];
    assert_eq!(
        file.title,
        Some(Field::new("Andante".into(), FieldSource::Tag))
    );
    assert_eq!(
        file.source,
        TrackSource::File {
            path: album_path.join("CD2/01.flac")
        }
    );
    assert_eq!(file.properties.duration_ms, Some(360_000));
    assert!(album.cover.is_some());
    assert!(!album.signature.is_empty());
    assert!(
        index
            .album(&library.path().join("不存在"))
            .unwrap()
            .is_none()
    );
}

#[test]
fn querying_an_album_keeps_shift_jis_cue_titles() {
    let library = tempfile::tempdir().unwrap();
    let path = library.path().join("ヴァイオリン協奏曲");
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("CDImage.ape"), b"fixture").unwrap();
    let cue = "PERFORMER \"諏訪内晶子\"\nTITLE \"ヴァイオリン協奏曲\"\nFILE \"CDImage.ape\" WAVE\n  TRACK 01 AUDIO\n    TITLE \"第一楽章アレグロ\"\n    INDEX 01 00:00:00\n";
    let (bytes, _, errors) = encoding_rs::SHIFT_JIS.encode(cue);
    assert!(!errors);
    std::fs::write(path.join("program.cue"), bytes).unwrap();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut FixtureTags))
        .unwrap();

    let album = index.album(&path).unwrap().unwrap();

    assert_eq!(album.display_title(), "ヴァイオリン協奏曲");
    assert_eq!(
        album.metadata.title.as_ref().unwrap().source,
        FieldSource::Cue
    );
    assert_eq!(
        album.discs[0].tracks[0].title,
        Some(Field::new("第一楽章アレグロ".into(), FieldSource::Cue))
    );
}
