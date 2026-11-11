use std::path::Path;
use voon_core::{
    FieldSource,
    db::{Index, SCHEMA_VERSION},
};

#[test]
fn upgrading_a_populated_v2_index_preserves_offline_data_and_unrelated_roots() {
    let local = tempfile::tempdir().unwrap();
    let path = local.path().join("index.sqlite3");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(include_str!("fixtures/schema-v2.sql"))
        .unwrap();
    connection.execute_batch("
        INSERT INTO library_root(id,path) VALUES (1,'/offline/one'),(2,'/offline/two');
        INSERT INTO album(id,root_id,path,folder_name,signature)
          VALUES (1,1,'/offline/one/album','第一张','one'),(2,2,'/offline/two/album','第二张','two');
        INSERT INTO album_field(album_id,name,source,value) VALUES (2,'title','tag','标签标题');
        INSERT INTO disc(id,album_id,number,path) VALUES (1,2,1,'/offline/two/album');
        INSERT INTO track(disc_id,number,format,path,sample_rate,bit_depth)
          VALUES (1,1,'FLAC','/offline/two/album/01.flac',96000,24);
        INSERT INTO user_edit(album_path,name,value) VALUES ('/offline/two/album','title','旧版编辑');
        INSERT INTO user_tag(id,name) VALUES (1,'收藏');
        INSERT INTO album_user_tag(album_path,tag_id) VALUES ('/offline/two/album',1);
        INSERT INTO manual_collection(id,name) VALUES (1,'周末');
        INSERT INTO manual_collection_album(collection_id,album_path,position) VALUES (1,'/offline/two/album',0);
    ").unwrap();
    drop(connection);

    let mut index = Index::open(&path).unwrap();
    assert_eq!(index.schema_version().unwrap(), SCHEMA_VERSION);
    assert_eq!(index.albums().unwrap().len(), 2);
    let album_path = Path::new("/offline/two/album");
    let album = index.album(album_path).unwrap().unwrap();
    assert_eq!(album.display_title(), "旧版编辑");
    assert_eq!(album.metadata.title.unwrap().source, FieldSource::UserEdit);
    assert_eq!(album.discs[0].tracks[0].properties.sample_rate, Some(96000));
    assert_eq!(index.user_tags(album_path).unwrap(), vec!["收藏"]);
    assert_eq!(
        index.manual_collection_albums(1).unwrap(),
        vec![album_path.to_path_buf()]
    );

    // No rescan is possible while offline. Removing another root must not
    // depend on that rescan to preserve the legacy records.
    index.remove_root(Path::new("/offline/one")).unwrap();
    drop(index);
    let index = Index::open(&path).unwrap();
    assert_eq!(index.albums().unwrap().len(), 1);
    assert_eq!(
        index.album(album_path).unwrap().unwrap().display_title(),
        "旧版编辑"
    );
}
