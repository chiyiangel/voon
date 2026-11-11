use std::path::Path;
use voon_core::{
    AudioFormat, FieldSource,
    db::Index,
    scan::{FileStamp, TagReader, scan_root},
    tags::TagInfo,
};

struct TaggedAlbum;
impl TagReader for TaggedAlbum {
    fn read(&mut self, _: &Path, _: AudioFormat, _: FileStamp) -> Option<TagInfo> {
        Some(TagInfo {
            album: Some("自动标题".into()),
            year: Some(1999),
            ..TagInfo::default()
        })
    }
}

#[test]
fn album_edits_save_together_survive_rescans_and_restore_automatic_values() {
    let library = tempfile::tempdir().unwrap();
    let album = library.path().join("Philips Duo");
    std::fs::create_dir(&album).unwrap();
    std::fs::write(album.join("01.flac"), b"fixture").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut TaggedAlbum))
        .unwrap();

    index
        .save_user_edits(
            &album,
            &[
                ("title".into(), Some("我的标题".into())),
                ("year".into(), Some("2001".into())),
            ],
        )
        .unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut TaggedAlbum))
        .unwrap();

    let edited = index.album(&album).unwrap().unwrap();
    assert_eq!(edited.display_title(), "我的标题");
    assert_eq!(
        edited.metadata.title.as_ref().unwrap().source,
        FieldSource::UserEdit
    );
    assert_eq!(edited.metadata.year.as_ref().unwrap().value, 2001);
    assert_eq!(
        index
            .automatic_metadata(&album)
            .unwrap()
            .unwrap()
            .title
            .unwrap()
            .value,
        "自动标题"
    );

    index
        .save_user_edits(&album, &[("title".into(), None), ("year".into(), None)])
        .unwrap();
    let restored = index.album(&album).unwrap().unwrap();
    assert_eq!(restored.display_title(), "自动标题");
    assert_eq!(restored.metadata.title.unwrap().source, FieldSource::Tag);
    assert_eq!(restored.metadata.year.unwrap().value, 1999);
}

#[test]
fn an_invalid_field_or_year_rejects_the_entire_edit_form() {
    let library = tempfile::tempdir().unwrap();
    let album = library.path().join("专辑");
    std::fs::create_dir(&album).unwrap();
    std::fs::write(album.join("01.flac"), b"fixture").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut TaggedAlbum))
        .unwrap();

    for invalid_year in ["20xx", "0", "10000", "-1"] {
        assert!(
            index
                .save_user_edits(
                    &album,
                    &[
                        ("title".into(), Some("不应保存".into())),
                        ("year".into(), Some(invalid_year.into()))
                    ]
                )
                .is_err()
        );
        let unchanged = index.album(&album).unwrap().unwrap();
        assert_eq!(unchanged.display_title(), "自动标题");
        assert_eq!(unchanged.metadata.year.unwrap().value, 1999);
    }
    assert!(
        index
            .save_user_edits(&album, &[("unsupported".into(), None)])
            .is_err()
    );
    assert!(
        index
            .save_user_edits(
                &library.path().join("不存在"),
                &[("title".into(), Some("标题".into()))]
            )
            .is_err()
    );
}
