use std::{fs, path::Path, time::SystemTime};

use voon_core::db::{Index, SCHEMA_VERSION};
use voon_core::scan::{NoTags, scan_root};

fn snapshot(directory: &Path) -> Vec<(String, Vec<u8>, SystemTime)> {
    let mut files = fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().to_string_lossy().into_owned(),
                fs::read(entry.path()).unwrap(),
                entry.metadata().unwrap().modified().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

#[test]
fn opening_a_newer_index_preserves_the_database_and_existing_sidecars() {
    let local = tempfile::tempdir().unwrap();
    let path = local.path().join("future.sqlite3");
    drop(Index::open(&path).unwrap());
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .unwrap();
    drop(connection);
    fs::write(local.path().join("future.sqlite3-wal"), b"future WAL bytes").unwrap();
    fs::write(
        local.path().join("future.sqlite3-shm"),
        b"future shared memory bytes",
    )
    .unwrap();
    fs::write(
        local.path().join("future.sqlite3-journal"),
        b"future journal bytes",
    )
    .unwrap();
    let before = snapshot(local.path());

    let error = Index::open(&path)
        .err()
        .expect("a higher-version index must be refused before initialization");

    assert!(error.to_string().contains("更高版本"), "{error}");
    assert_eq!(snapshot(local.path()), before);
}

#[test]
fn a_newer_version_committed_in_wal_is_refused_without_touching_live_sidecars() {
    let local = tempfile::tempdir().unwrap();
    let path = local.path().join("future-in-wal.sqlite3");
    drop(Index::open(&path).unwrap());
    let writer = rusqlite::Connection::open(&path).unwrap();
    writer.pragma_update(None, "journal_mode", "WAL").unwrap();
    writer
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    writer
        .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .unwrap();
    let before = snapshot(local.path());

    let error = Index::open(&path)
        .err()
        .expect("a committed newer WAL version must be refused");

    assert!(error.to_string().contains("更高版本"), "{error}");
    assert_eq!(snapshot(local.path()), before);
}

#[cfg(unix)]
#[test]
fn opening_a_symlink_checks_the_targets_pending_version_without_touching_its_files() {
    let local = tempfile::tempdir().unwrap();
    let path = local.path().join("future-target.sqlite3");
    drop(Index::open(&path).unwrap());
    let writer = rusqlite::Connection::open(&path).unwrap();
    writer.pragma_update(None, "journal_mode", "WAL").unwrap();
    writer
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    writer
        .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .unwrap();
    let aliases = tempfile::tempdir().unwrap();
    let alias = aliases.path().join("chosen.sqlite3");
    std::os::unix::fs::symlink(&path, &alias).unwrap();
    let before = snapshot(local.path());

    let error = Index::open(&alias)
        .err()
        .expect("the actual index's WAL must be checked");

    assert!(error.to_string().contains("更高版本"), "{error}");
    assert_eq!(snapshot(local.path()), before);
}

#[test]
fn empty_and_foreign_files_are_kept_without_initialization() {
    for contents in [b"".as_slice(), b"not a SQLite database".as_slice()] {
        let local = tempfile::tempdir().unwrap();
        let path = local.path().join("unknown.sqlite3");
        fs::write(&path, contents).unwrap();
        fs::write(local.path().join("unknown.sqlite3-wal"), b"unknown WAL").unwrap();
        fs::write(
            local.path().join("unknown.sqlite3-shm"),
            b"unknown shared memory",
        )
        .unwrap();
        let before = snapshot(local.path());

        let error = Index::open(&path)
            .err()
            .expect("existing files are not new indices");

        assert!(error.to_string().contains("无法识别"), "{error}");
        assert_eq!(snapshot(local.path()), before);
    }
}

#[test]
fn an_unrecognized_database_is_not_initialized_as_a_voon_index() {
    let local = tempfile::tempdir().unwrap();
    let path = local.path().join("other.sqlite3");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch("CREATE TABLE other_application (value TEXT); INSERT INTO other_application VALUES ('keep me');")
        .unwrap();
    connection
        .pragma_update(None, "user_version", SCHEMA_VERSION)
        .unwrap();
    drop(connection);
    let before = snapshot(local.path());

    let error = Index::open(&path)
        .err()
        .expect("an unrelated database must not become a Voon index");

    assert!(error.to_string().contains("无法识别"), "{error}");
    assert_eq!(snapshot(local.path()), before);
}

#[test]
fn creating_a_new_index_never_replaces_an_existing_file_and_can_resume_scanning() {
    let old = tempfile::tempdir().unwrap();
    let old_path = old.path().join("old.sqlite3");
    fs::write(&old_path, b"an existing library must be kept").unwrap();
    fs::write(old.path().join("old.sqlite3-wal"), b"old sidecar").unwrap();
    let before = snapshot(old.path());

    assert!(Index::create(&old_path).is_err());
    assert_eq!(snapshot(old.path()), before);

    let library = tempfile::tempdir().unwrap();
    let album = library.path().join("第一张");
    fs::create_dir(&album).unwrap();
    fs::write(album.join("01.flac"), b"audio fixture").unwrap();
    let storage = tempfile::tempdir().unwrap();
    let database = storage.path().join("new.sqlite3");
    {
        let mut index = Index::create(&database).unwrap();
        index
            .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
            .unwrap();
    }
    let second = library.path().join("第二张");
    fs::create_dir(&second).unwrap();
    fs::write(second.join("01.flac"), b"another audio fixture").unwrap();
    let mut index = Index::open(&database).unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    drop(index);

    let index = Index::open(&database).unwrap();
    assert_eq!(index.albums().unwrap().len(), 2);
    assert!(index.album(&album).unwrap().is_some());
    assert!(index.album(&second).unwrap().is_some());
    assert_eq!(snapshot(old.path()), before);
}
