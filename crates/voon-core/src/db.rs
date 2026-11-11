//! The local index: a SQLite database on the Mac, so browsing works while
//! the NAS is unmounted.
//!
//! Scan results are rebuilt per Library root on every scan. What the user
//! owns (field edits, user tags, manual collections) is keyed by album path
//! instead of row id, so it survives rescans.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};

use crate::model::*;
use crate::scan::{FileStamp, ScanResult, TagReader};
mod candidates;
mod details;
mod edits;
mod structure;
mod tag_cache;
pub use edits::EditError;
pub use structure::{DiscRef, SplitMode, StructureOperation, TrackRef};
mod collections;
mod composers;
mod covers;

mod browse;
mod ignore;
mod organization;
pub use organization::{ManualCollectionRow, UserTagRow};
mod attention;
pub use attention::AttentionItem;
mod protection;
pub use protection::OpenError;

pub type Result<T> = rusqlite::Result<T>;

/// Each entry upgrades the schema by one version (`PRAGMA user_version`).
const MIGRATIONS: &[&str] = &[
    r#"
CREATE TABLE library_root (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    last_scan INTEGER
);
CREATE TABLE collection (
    id INTEGER PRIMARY KEY,
    root_id INTEGER NOT NULL REFERENCES library_root(id) ON DELETE CASCADE,
    path TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    parent_path TEXT
);
CREATE TABLE album (
    id INTEGER PRIMARY KEY,
    root_id INTEGER NOT NULL REFERENCES library_root(id) ON DELETE CASCADE,
    path TEXT NOT NULL UNIQUE,
    folder_name TEXT NOT NULL,
    collection_path TEXT,
    cover_path TEXT,
    cover_embedded INTEGER NOT NULL DEFAULT 0
);
-- Every candidate value per field and source; the strongest source wins on read.
CREATE TABLE album_field (
    album_id INTEGER NOT NULL REFERENCES album(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    source TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (album_id, name, source)
);
CREATE TABLE disc (
    id INTEGER PRIMARY KEY,
    album_id INTEGER NOT NULL REFERENCES album(id) ON DELETE CASCADE,
    number INTEGER NOT NULL,
    path TEXT NOT NULL
);
CREATE TABLE track (
    id INTEGER PRIMARY KEY,
    disc_id INTEGER NOT NULL REFERENCES disc(id) ON DELETE CASCADE,
    number INTEGER,
    title TEXT,
    title_source TEXT,
    artist TEXT,
    artist_source TEXT,
    format TEXT NOT NULL,
    path TEXT NOT NULL,
    cue_path TEXT,
    start_frames INTEGER,
    end_frames INTEGER,
    sample_rate INTEGER,
    bit_depth INTEGER,
    channels INTEGER,
    duration_ms INTEGER
);
CREATE INDEX track_disc ON track(disc_id);
CREATE TABLE scan_issue (
    id INTEGER PRIMARY KEY,
    root_id INTEGER NOT NULL REFERENCES library_root(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    kind TEXT NOT NULL,
    detail TEXT
);
-- Tag cache for incremental rescans: re-read only when size or mtime changed.
CREATE TABLE file_cache (
    path TEXT PRIMARY KEY,
    size INTEGER NOT NULL,
    mtime INTEGER NOT NULL,
    readable INTEGER NOT NULL,
    title TEXT, artist TEXT, album TEXT, album_artist TEXT, composer TEXT,
    conductor TEXT, genre TEXT, year INTEGER, track INTEGER, disc INTEGER,
    catalog_number TEXT, has_picture INTEGER NOT NULL DEFAULT 0,
    sample_rate INTEGER, bit_depth INTEGER, channels INTEGER, duration_ms INTEGER
);
-- User-owned data, keyed by album path.
CREATE TABLE user_edit (
    album_path TEXT NOT NULL,
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (album_path, name)
);
CREATE TABLE user_tag (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);
CREATE TABLE album_user_tag (
    album_path TEXT NOT NULL,
    tag_id INTEGER NOT NULL REFERENCES user_tag(id) ON DELETE CASCADE,
    PRIMARY KEY (album_path, tag_id)
);
CREATE TABLE manual_collection (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);
CREATE TABLE manual_collection_album (
    collection_id INTEGER NOT NULL REFERENCES manual_collection(id) ON DELETE CASCADE,
    album_path TEXT NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY (collection_id, album_path)
);
"#,
    r#"
ALTER TABLE album ADD COLUMN signature TEXT NOT NULL DEFAULT '';
CREATE INDEX album_signature ON album(signature);
-- An album path that vanished in a rescan while a new path with the same
-- content signature appeared. Kept across rescans until confirmed or
-- dismissed, or until the new path itself disappears.
CREATE TABLE rename_candidate (
    old_path TEXT NOT NULL,
    new_path TEXT NOT NULL,
    signature TEXT NOT NULL,
    PRIMARY KEY (old_path, new_path)
);
"#,
    ignore::MIGRATION,
    r#"
CREATE TABLE scan_baseline (
    root_id INTEGER NOT NULL REFERENCES library_root(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    signature TEXT NOT NULL,
    PRIMARY KEY(root_id, path)
);
INSERT INTO scan_baseline (root_id, path, signature) SELECT root_id, path, signature FROM album;
"#,
    browse::MIGRATION,
    covers::MIGRATION,
    structure::MIGRATION,
    structure::PERSIST_ROOT_MIGRATION,
    covers::SOURCE_DIRECTORIES_MIGRATION,
    tag_cache::MIGRATION,
    candidates::MIGRATION,
    tag_cache::READ_ORDER_MIGRATION,
];

/// A proposed pairing of a vanished album path with a new one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenameCandidate {
    pub old_path: PathBuf,
    pub new_path: PathBuf,
}

/// How many user-owned rows a confirmed rename moved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenameMigration {
    pub edits: usize,
    pub tags: usize,
    pub manual_collections: usize,
}

/// Schema version after all migrations.
pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;
/// SQLite application identity, independent of the structure version.
pub const APPLICATION_ID: u32 = u32::from_be_bytes(*b"VOON");

pub struct Index {
    conn: Connection,
}

/// An album as read back from the index, with user edits applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlbumRow {
    pub id: i64,
    pub path: PathBuf,
    pub source_directories: Vec<PathBuf>,
    pub folder_name: String,
    pub collection: Option<PathBuf>,
    pub cover: Option<Cover>,
    pub metadata: AlbumMetadata,
    pub track_count: u32,
    pub disc_count: u32,
    pub added_order: i64,
    pub facets: crate::facets::Facets,
}

impl AlbumRow {
    /// Summary for an Album reported during a scan, before it has an Index id.
    pub fn from_album(album: &Album) -> Self {
        Self {
            id: 0,
            path: album.path.clone(),
            source_directories: album
                .physical_directories()
                .into_iter()
                .map(Path::to_path_buf)
                .collect(),
            folder_name: album.folder_name.clone(),
            collection: album.collection.clone(),
            cover: album.cover.clone(),
            metadata: album.metadata.clone(),
            track_count: album.tracks().count() as u32,
            disc_count: album.discs.len() as u32,
            added_order: 0,
            facets: crate::facets::facets(album),
        }
    }

    pub fn display_title(&self) -> &str {
        self.metadata
            .title
            .as_ref()
            .map(|f| f.value.as_str())
            .unwrap_or(&self.folder_name)
    }
}

fn p(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn album_overlaps_scope(album: &Album, scope: &Path) -> bool {
    let loose = album
        .path
        .components()
        .any(|part| part.as_os_str() == LOOSE_DIR);
    album.physical_directories().iter().any(|source| {
        source.starts_with(scope)
            || if loose {
                album
                    .tracks()
                    .any(|track| track.source.audio_path().starts_with(scope))
            } else {
                scope.starts_with(source)
            }
    })
}

impl Index {
    pub fn open(path: &Path) -> std::result::Result<Self, OpenError> {
        if path.try_exists()? {
            let path = path.canonicalize()?;
            protection::check_existing(&path, MIGRATIONS)?;
            return Self::open_validated(&path);
        }
        Self::create(path)
    }

    /// Create a separate Index. An existing file or sidecar is never replaced.
    pub fn create(path: &Path) -> std::result::Result<Self, OpenError> {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let mut related = path.as_os_str().to_os_string();
            related.push(suffix);
            match std::fs::symlink_metadata(related) {
                Ok(_) => return Err(OpenError::AlreadyExists),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        Self::open_validated(path)
    }

    fn open_validated(path: &Path) -> std::result::Result<Self, OpenError> {
        let connection = Connection::open_with_flags(
            path.canonicalize()?,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        Self::init(connection).map_err(OpenError::from)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        let mut index = Self { conn };
        index.migrate()?;
        index
            .conn
            .pragma_update(None, "application_id", APPLICATION_ID)?;
        Ok(index)
    }

    pub fn schema_version(&self) -> Result<u32> {
        self.conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
    }

    /// The local database file, absent for an in-memory index.
    pub fn database_path(&self) -> Option<&Path> {
        self.conn
            .path()
            .filter(|path| !path.is_empty())
            .map(Path::new)
    }

    fn migrate(&mut self) -> Result<()> {
        let current = self.schema_version()? as usize;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(current) {
            let tx = self.conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (i + 1) as u32)?;
            tx.commit()?;
        }
        Ok(())
    }

    pub fn add_root(&self, path: &Path) -> Result<i64> {
        self.conn.execute(
            "INSERT OR IGNORE INTO library_root (path) VALUES (?1)",
            [p(path)],
        )?;
        self.conn.query_row(
            "SELECT id FROM library_root WHERE path = ?1",
            [p(path)],
            |r| r.get(0),
        )
    }

    pub fn roots(&self) -> Result<Vec<LibraryRoot>> {
        let mut st = self
            .conn
            .prepare("SELECT path FROM library_root ORDER BY id")?;
        st.query_map([], |r| {
            Ok(LibraryRoot {
                path: PathBuf::from(r.get::<_, String>(0)?),
            })
        })?
        .collect()
    }

    /// Whether this root has accepted physical Albums, including completed
    /// discoveries retained from an interrupted first scan.
    pub fn has_scanned_albums(&self, root: &Path) -> Result<bool> {
        self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM scan_baseline s JOIN library_root r ON r.id = s.root_id WHERE r.path = ?1)
             OR EXISTS(SELECT 1 FROM structure_raw s JOIN library_root r ON r.id = s.root_id WHERE r.path = ?1)",
            [p(root)], |row| row.get(0),
        )
    }

    /// Remove indexed results for a root, retaining path-keyed user edits,
    /// tags and manual collection membership for a later re-addition.
    pub fn remove_root(&mut self, root: &Path) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM rename_candidate WHERE new_path IN
             (SELECT a.path FROM album a JOIN library_root r ON a.root_id = r.id WHERE r.path = ?1)",
            [p(root)],
        )?;
        tx.execute("DELETE FROM library_root WHERE path = ?1", [p(root)])?;
        structure::replay(&tx)?;
        tx.commit()
    }

    /// Replace observed physical scopes under `root`, retaining incomplete
    /// Albums. Without successful root coverage this commits additions and
    /// updates only, just like `store_partial_scan`.
    pub fn store_scan(&mut self, root: &Path, scan: &ScanResult) -> Result<()> {
        self.store_scan_result(root, scan, true, &[])
    }

    /// Save completed portions of a cancelled scan without discarding folders
    /// the scanner has not visited or inferring renames from incomplete input.
    pub fn store_partial_scan(&mut self, root: &Path, scan: &ScanResult) -> Result<()> {
        self.store_scan_result(root, scan, false, &[])
    }

    /// Confirm structural changes only around explicitly retried locations.
    /// Other Albums and the complete-root rename baseline remain untouched.
    /// Cancellation or missing directory coverage cannot confirm a removal.
    pub fn store_scoped_scan(
        &mut self,
        root: &Path,
        scan: &ScanResult,
        scopes: &[PathBuf],
    ) -> Result<()> {
        self.store_scan_result(root, scan, false, scopes)
    }

    fn store_scan_result(
        &mut self,
        root: &Path,
        scan: &ScanResult,
        complete: bool,
        scopes: &[PathBuf],
    ) -> Result<()> {
        // Cancellation is part of the result's contract, even if a caller uses
        // the ordinary commit entry point instead of store_partial_scan.
        let complete = complete && !scan.cancelled && scan.root_was_listed(root);
        let root_id = self.add_root(root)?;
        let tx = self.conn.transaction()?;
        let incomplete = scan.incomplete_paths();
        let bounded = !scopes.is_empty();
        let requested: Vec<PathBuf> = scopes
            .iter()
            .filter_map(|scope| {
                if scope == root {
                    Some(root.to_path_buf())
                } else {
                    crate::scan::relative_ignored_path(root, scope).map(|path| root.join(path))
                }
            })
            .collect();
        let previous = if bounded {
            structure::physical_albums(&tx, root_id)?
        } else {
            vec![]
        };
        let previously_affected: Vec<PathBuf> = previous
            .iter()
            .filter(|album| {
                requested
                    .iter()
                    .any(|scope| album_overlaps_scope(album, scope))
            })
            .map(|album| album.path.clone())
            .collect();
        let mut accepted = scan.clone();
        accepted.albums.retain(|album| {
            scan.album_is_complete(album)
                && (!bounded
                    || requested
                        .iter()
                        .any(|scope| album_overlaps_scope(album, scope))
                    || previously_affected.contains(&album.path))
        });
        if bounded {
            let relevant: Vec<_> = previous
                .iter()
                .chain(&scan.albums)
                .filter(|album| {
                    requested
                        .iter()
                        .any(|scope| album_overlaps_scope(album, scope))
                        || previously_affected.contains(&album.path)
                })
                .collect();
            accepted.issues.retain(|issue| {
                let path = match &issue.kind {
                    crate::scan::IssueKind::LooseUntagged { file } => file.as_path(),
                    _ => issue.path.as_path(),
                };
                requested
                    .iter()
                    .any(|scope| path.starts_with(scope) || scope.starts_with(path))
                    || relevant
                        .iter()
                        .any(|album| album_overlaps_scope(album, path))
            });
            accepted.collections.retain(|collection| {
                requested.iter().any(|scope| {
                    collection.path.starts_with(scope) || scope.starts_with(&collection.path)
                })
            });
        }
        let scopes: Vec<PathBuf> = requested
            .into_iter()
            .filter(|path| {
                if scan.cancelled {
                    return false;
                }
                scan.coverage
                    .listed_directories
                    .iter()
                    .any(|listed| path.starts_with(listed))
            })
            .collect();
        if !scopes.is_empty() {
            for previous in previous {
                let selected = scopes
                    .iter()
                    .any(|scope| album_overlaps_scope(&previous, scope));
                let observed = previous.physical_directories().iter().all(|source| {
                    scan.coverage
                        .listed_directories
                        .iter()
                        .any(|listed| source.starts_with(listed))
                });
                if selected
                    && observed
                    && scan.album_is_complete(&previous)
                    && !accepted
                        .albums
                        .iter()
                        .any(|album| album.path == previous.path)
                {
                    tx.execute(
                        "DELETE FROM album WHERE root_id=?1 AND path=?2",
                        params![root_id, p(&previous.path)],
                    )?;
                    tx.execute(
                        "DELETE FROM structure_raw WHERE root_id=?1 AND path=?2",
                        params![root_id, p(&previous.path)],
                    )?;
                }
            }
        }
        if complete && !incomplete.is_empty() {
            accepted.albums.extend(
                structure::physical_albums(&tx, root_id)?
                    .into_iter()
                    .filter(|album| !scan.album_is_complete(album)),
            );
        }
        let scan = &accepted;
        browse::record_album_added(&tx, root, &scan.albums)?;
        let before: Vec<(String, String)> = {
            let mut st =
                tx.prepare("SELECT path, signature FROM scan_baseline WHERE root_id = ?1")?;
            st.query_map([root_id], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<_>>()?
        };
        if complete {
            tx.execute("DELETE FROM album WHERE root_id = ?1", [root_id])?;
            let collections: Vec<String> = {
                let mut statement = tx.prepare("SELECT path FROM collection WHERE root_id=?1")?;
                statement
                    .query_map([root_id], |row| row.get(0))?
                    .collect::<Result<_>>()?
            };
            for path in collections {
                if !incomplete
                    .iter()
                    .any(|failed| Path::new(&path).starts_with(failed) || failed.starts_with(&path))
                {
                    tx.execute(
                        "DELETE FROM collection WHERE root_id=?1 AND path=?2",
                        params![root_id, path],
                    )?;
                }
            }
            tx.execute("DELETE FROM scan_issue WHERE root_id = ?1", [root_id])?;
        } else {
            let issues: Vec<(i64, String)> = {
                let mut statement =
                    tx.prepare("SELECT id,path FROM scan_issue WHERE root_id=?1")?;
                statement
                    .query_map([root_id], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<Result<_>>()?
            };
            for (id, path) in issues {
                if scopes
                    .iter()
                    .any(|scope| Path::new(&path).starts_with(scope))
                    || scan.albums.iter().any(|album| {
                        if album
                            .path
                            .components()
                            .any(|part| part.as_os_str() == LOOSE_DIR)
                        {
                            return album
                                .tracks()
                                .any(|track| track.source.audio_path() == Path::new(&path));
                        }
                        album
                            .physical_directories()
                            .iter()
                            .any(|source| Path::new(&path).starts_with(source))
                    })
                {
                    tx.execute("DELETE FROM scan_issue WHERE id=?1", [id])?;
                }
            }
        }
        for c in &scan.collections {
            tx.execute(
                "INSERT INTO collection (root_id, path, name, parent_path) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(path) DO UPDATE SET name = excluded.name, parent_path = excluded.parent_path
                 WHERE collection.root_id = excluded.root_id",
                params![root_id, p(&c.path), c.name, c.parent.as_deref().map(p)],
            )?;
        }
        for album in &scan.albums {
            if !complete {
                // A task can be detached immediately after reporting a complete
                // Album, before traversal has emitted its Collection records.
                let mut collection = album.collection.as_deref();
                while let Some(path) =
                    collection.filter(|path| *path != root && path.starts_with(root))
                {
                    let parent = path
                        .parent()
                        .filter(|path| *path != root && path.starts_with(root));
                    tx.execute(
                        "INSERT OR IGNORE INTO collection(root_id,path,name,parent_path) VALUES (?1,?2,?3,?4)",
                        params![root_id, p(path), path.file_name().unwrap_or_default().to_string_lossy(), parent.map(p)],
                    )?;
                    collection = parent;
                }
            }
            details::store_album(&tx, root_id, album)?;
        }
        let mut replaced_issue_scopes = std::collections::HashSet::new();
        for issue in &scan.issues {
            let (kind, detail) = attention::describe(issue);
            if replaced_issue_scopes.insert((issue.path.as_path(), kind)) {
                tx.execute(
                    "DELETE FROM scan_issue WHERE root_id=?1 AND path=?2 AND kind=?3",
                    params![root_id, p(&issue.path), kind],
                )?;
            }
            tx.execute(
                "INSERT INTO scan_issue (root_id, path, kind, detail)
                 SELECT ?1, ?2, ?3, ?4 WHERE NOT EXISTS
                 (SELECT 1 FROM scan_issue WHERE root_id = ?1 AND path = ?2 AND kind = ?3 AND detail = ?4)",
                params![root_id, p(&issue.path), kind, detail],
            )?;
        }
        if complete && incomplete.is_empty() {
            record_rename_candidates(&tx, root_id, &before, &scan.albums)?;
            tx.execute("DELETE FROM scan_baseline WHERE root_id = ?1", [root_id])?;
            tx.execute(
                "INSERT INTO scan_baseline (root_id, path, signature) SELECT root_id, path, signature FROM album WHERE root_id = ?1",
                [root_id],
            )?;
            tx.execute(
                "UPDATE library_root SET last_scan = strftime('%s','now') WHERE id = ?1",
                [root_id],
            )?;
        } else if complete || !scopes.is_empty() {
            // A confirmed removal/change can expire an existing proposal even
            // when another subtree is protected. Never infer a new pairing.
            prune_rename_candidates(&tx, root_id)?;
        }
        structure::record_scan_and_replay(&tx, root_id, root, scan, complete)?;
        if !scopes.is_empty() {
            let collections: Vec<String> = {
                let mut statement = tx.prepare("SELECT path FROM collection WHERE root_id=?1")?;
                statement
                    .query_map([root_id], |row| row.get(0))?
                    .collect::<Result<_>>()?
            };
            for path in collections {
                let path = Path::new(&path);
                if scopes
                    .iter()
                    .any(|scope| path.starts_with(scope) || scope.starts_with(path))
                    && !scan
                        .collections
                        .iter()
                        .any(|collection| collection.path == path)
                    && !incomplete
                        .iter()
                        .any(|failed| path.starts_with(failed) || failed.starts_with(path))
                {
                    tx.execute(
                        "DELETE FROM collection WHERE root_id=?1 AND path=?2",
                        params![root_id, p(path)],
                    )?;
                }
            }
        }
        tx.commit()
    }

    /// All albums, sorted by display title, with user edits applied.
    pub fn albums(&self) -> Result<Vec<AlbumRow>> {
        let mut st = self.conn.prepare(
            "SELECT a.id, a.path, a.folder_name, a.collection_path, a.cover_path, a.cover_embedded,
                    (SELECT COUNT(*) FROM track t JOIN disc d ON t.disc_id = d.id WHERE d.album_id = a.id),
                    (SELECT COUNT(*) FROM disc d WHERE d.album_id = a.id), a.cover_picture_index, a.source_directories
             FROM album a ORDER BY a.folder_name",
        )?;
        let mut rows: Vec<AlbumRow> = st
            .query_map([], |r| {
                let cover = r.get::<_, Option<String>>(4)?.map(|c| {
                    let c = PathBuf::from(c);
                    covers::from_parts(
                        c,
                        r.get::<_, bool>(5).unwrap_or(false),
                        r.get::<_, Option<u32>>(8).unwrap_or(None),
                    )
                });
                let path = PathBuf::from(r.get::<_, String>(1)?);
                let mut source_directories =
                    details::read_source_directories(&r.get::<_, String>(9)?)?;
                if source_directories.is_empty() {
                    source_directories.push(path.clone());
                }
                Ok(AlbumRow {
                    id: r.get(0)?,
                    path,
                    source_directories,
                    folder_name: r.get(2)?,
                    collection: r.get::<_, Option<String>>(3)?.map(PathBuf::from),
                    cover,
                    metadata: AlbumMetadata::default(),
                    track_count: r.get(6)?,
                    disc_count: r.get(7)?,
                    added_order: 0,
                    facets: crate::facets::Facets::default(),
                })
            })?
            .collect::<Result<_>>()?;
        let mut fields = self
            .conn
            .prepare("SELECT name, value, source FROM album_field WHERE album_id = ?1")?;
        let mut edits = self
            .conn
            .prepare("SELECT name, value FROM user_edit WHERE album_path = ?1")?;
        for row in &mut rows {
            let found = fields.query_map([row.id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?;
            for f in found {
                let (name, value, source) = f?;
                if let Some(source) = FieldSource::parse(&source) {
                    row.metadata.apply(&name, &value, source);
                }
            }
            let found = edits.query_map([p(&row.path)], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?;
            for e in found {
                let (name, value) = e?;
                row.metadata.apply(&name, &value, FieldSource::UserEdit);
            }
        }
        self.populate_browse_fields(&mut rows)?;
        self.apply_user_covers(&mut rows)?;
        rows.sort_by(|a, b| a.display_title().cmp(b.display_title()));
        Ok(rows)
    }

    pub fn collections(&self) -> Result<Vec<(PathBuf, String)>> {
        let mut st = self
            .conn
            .prepare("SELECT path, name FROM collection ORDER BY name")?;
        st.query_map([], |r| {
            Ok((PathBuf::from(r.get::<_, String>(0)?), r.get(1)?))
        })?
        .collect()
    }

    /// `(path, kind, detail)` for the 待处理 view, including rename
    /// candidates (kind `PossibleRename`, path = the new album path).
    pub fn issues(&self) -> Result<Vec<(PathBuf, String, String)>> {
        let mut st = self.conn.prepare(
            "SELECT path, kind, detail FROM scan_issue
             UNION ALL
             SELECT new_path, 'PossibleRename', old_path || ' -> ' || new_path FROM rename_candidate
             ORDER BY 1",
        )?;
        st.query_map([], |r| {
            Ok((PathBuf::from(r.get::<_, String>(0)?), r.get(1)?, r.get(2)?))
        })?
        .collect()
    }

    pub fn rename_candidates(&self) -> Result<Vec<RenameCandidate>> {
        let mut st = self
            .conn
            .prepare("SELECT old_path, new_path FROM rename_candidate ORDER BY new_path")?;
        st.query_map([], |r| {
            Ok(RenameCandidate {
                old_path: PathBuf::from(r.get::<_, String>(0)?),
                new_path: PathBuf::from(r.get::<_, String>(1)?),
            })
        })?
        .collect()
    }

    /// The user confirmed that `old` was renamed or moved to `new`: move user
    /// edits, user tags and manual collection membership over, in one
    /// transaction. Where `new` already has its own value (an edit of the
    /// same field, the same tag, the same collection), that value is kept and
    /// the old one dropped. Any candidate involving either path is cleared.
    pub fn confirm_rename(&mut self, old: &Path, new: &Path) -> Result<RenameMigration> {
        let (old, new) = (p(old), p(new));
        let tx = self.conn.transaction()?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM rename_candidate c JOIN album a ON a.path = c.new_path
             WHERE c.old_path = ?1 AND c.new_path = ?2 AND c.signature = a.signature)",
            params![old, new],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        let mut moved = RenameMigration::default();
        for (table, count) in [
            ("user_edit", &mut moved.edits),
            ("album_user_tag", &mut moved.tags),
            ("manual_collection_album", &mut moved.manual_collections),
        ] {
            *count = tx.execute(
                &format!("UPDATE OR IGNORE {table} SET album_path = ?2 WHERE album_path = ?1"),
                params![old, new],
            )?;
            tx.execute(
                &format!("DELETE FROM {table} WHERE album_path = ?1"),
                [&old],
            )?;
        }
        covers::migrate_user_cover(&tx, Path::new(&old), Path::new(&new))?;
        tx.execute(
            "DELETE FROM rename_candidate WHERE old_path = ?1 OR new_path = ?2",
            params![old, new],
        )?;
        structure::confirm_rename(&tx, Path::new(&old), Path::new(&new))?;
        tx.commit()?;
        Ok(moved)
    }

    /// The user says the pairing is wrong; forget it.
    pub fn dismiss_rename(&self, old: &Path, new: &Path) -> Result<()> {
        self.conn.execute(
            "DELETE FROM rename_candidate WHERE old_path = ?1 AND new_path = ?2",
            params![p(old), p(new)],
        )?;
        Ok(())
    }

    /// Set a user edit for an album field. It beats every scanned source.
    pub fn set_user_edit(&self, album: &Path, field: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO user_edit (album_path, name, value) VALUES (?1, ?2, ?3)
             ON CONFLICT (album_path, name) DO UPDATE SET value = excluded.value",
            params![p(album), field, value],
        )?;
        Ok(())
    }

    pub fn clear_user_edit(&self, album: &Path, field: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM user_edit WHERE album_path = ?1 AND name = ?2",
            params![p(album), field],
        )?;
        Ok(())
    }

    pub fn add_user_tag(&self, album: &Path, tag: &str) -> Result<()> {
        let tag = organization::valid_name(tag)?;
        self.conn
            .execute("INSERT OR IGNORE INTO user_tag (name) VALUES (?1)", [tag])?;
        self.conn.execute(
            "INSERT OR IGNORE INTO album_user_tag (album_path, tag_id)
             SELECT ?1, id FROM user_tag WHERE name = ?2",
            params![p(album), tag],
        )?;
        Ok(())
    }

    pub fn user_tags(&self, album: &Path) -> Result<Vec<String>> {
        let mut st = self.conn.prepare(
            "SELECT t.name FROM user_tag t JOIN album_user_tag at ON at.tag_id = t.id
             WHERE at.album_path = ?1 ORDER BY t.name",
        )?;
        st.query_map([p(album)], |r| r.get(0))?.collect()
    }

    pub fn create_manual_collection(&self, name: &str) -> Result<i64> {
        let name = organization::valid_name(name)?;
        self.conn.execute(
            "INSERT OR IGNORE INTO manual_collection (name) VALUES (?1)",
            [name],
        )?;
        self.conn.query_row(
            "SELECT id FROM manual_collection WHERE name = ?1",
            [name],
            |r| r.get(0),
        )
    }

    pub fn add_to_manual_collection(&self, collection: i64, album: &Path) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO manual_collection_album (collection_id, album_path, position)
             SELECT ?1, ?2, COALESCE(MAX(position) + 1, 0) FROM manual_collection_album WHERE collection_id = ?1",
            params![collection, p(album)],
        )?;
        Ok(())
    }

    pub fn manual_collection_albums(&self, collection: i64) -> Result<Vec<PathBuf>> {
        let mut st = self.conn.prepare(
            "SELECT album_path FROM manual_collection_album WHERE collection_id = ?1 ORDER BY position, album_path",
        )?;
        st.query_map([collection], |r| Ok(PathBuf::from(r.get::<_, String>(0)?)))?
            .collect()
    }
}

/// Pair album paths that vanished in this scan with paths that appeared,
/// when exactly one of each shares a content signature. Ambiguous matches
/// (duplicate rips) are not proposed. Stale candidates are pruned.
fn record_rename_candidates(
    tx: &rusqlite::Transaction<'_>,
    root_id: i64,
    before: &[(String, String)],
    after: &[Album],
) -> Result<()> {
    use std::collections::{HashMap, HashSet};
    let now: HashSet<String> = after.iter().map(|a| p(&a.path)).collect();
    let was: HashSet<&str> = before.iter().map(|(path, _)| path.as_str()).collect();
    let mut gone: HashMap<&str, Vec<&str>> = HashMap::new();
    for (path, sig) in before {
        if !sig.is_empty() && !now.contains(path) {
            gone.entry(sig.as_str()).or_default().push(path);
        }
    }
    let mut appeared: HashMap<&str, Vec<String>> = HashMap::new();
    for a in after {
        let path = p(&a.path);
        if !a.signature.is_empty() && !was.contains(path.as_str()) {
            appeared.entry(a.signature.as_str()).or_default().push(path);
        }
    }
    for (sig, old) in &gone {
        if let (Some(new), [old]) = (appeared.get(sig), old.as_slice())
            && let [new] = new.as_slice()
        {
            // Renamed again before the user decided: user data still lives
            // at the first path, so chain X→old into X→new (or drop it when
            // the folder went back to X).
            let earlier: Vec<String> = {
                let mut st =
                    tx.prepare("SELECT old_path FROM rename_candidate WHERE new_path = ?1")?;
                st.query_map([old], |r| r.get(0))?.collect::<Result<_>>()?
            };
            tx.execute("DELETE FROM rename_candidate WHERE new_path = ?1", [old])?;
            let origins = if earlier.is_empty() {
                vec![old.to_string()]
            } else {
                earlier
            };
            for origin in origins.iter().filter(|o| *o != new) {
                tx.execute(
                    "INSERT OR IGNORE INTO rename_candidate (old_path, new_path, signature) VALUES (?1, ?2, ?3)",
                    params![origin, new, sig],
                )?;
            }
        }
    }
    prune_rename_candidates(tx, root_id)
}

fn prune_rename_candidates(tx: &rusqlite::Transaction<'_>, root_id: i64) -> Result<()> {
    // Only reconcile the root whose accepted physical snapshot was replaced.
    // Other roots may currently expose a user merge instead of physical rows.
    tx.execute(
        "DELETE FROM rename_candidate
         WHERE (new_path IN (SELECT path FROM scan_baseline WHERE root_id = ?1)
             OR new_path IN (SELECT path FROM album WHERE root_id = ?1)
             OR old_path IN (SELECT path FROM album WHERE root_id = ?1))
         AND (NOT EXISTS (SELECT 1 FROM album a WHERE a.path = rename_candidate.new_path
                          AND a.signature = rename_candidate.signature)
              OR old_path IN (SELECT path FROM album WHERE root_id = ?1))",
        [root_id],
    )?;
    Ok(())
}

/// Reuses successful unchanged observations, retries failed reads, and can
/// explicitly bypass the cache for selected audio sources.
pub struct CachedTags<'a, R: TagReader> {
    pub index: &'a Index,
    pub inner: R,
    /// Files actually read from disk during this scan.
    pub reads: usize,
    forced_paths: std::collections::BTreeSet<PathBuf>,
}

impl<'a, R: TagReader> CachedTags<'a, R> {
    pub fn new(index: &'a Index, inner: R) -> Self {
        Self {
            index,
            inner,
            reads: 0,
            forced_paths: Default::default(),
        }
    }

    /// Force only the selected audio sources; every other cache keeps its
    /// normal incremental behavior, including its original observation time.
    pub fn with_forced_paths(mut self, paths: impl IntoIterator<Item = PathBuf>) -> Self {
        self.forced_paths = paths.into_iter().collect();
        self
    }
}

impl<R: TagReader> TagReader for CachedTags<'_, R> {
    fn read(
        &mut self,
        path: &Path,
        format: AudioFormat,
        stamp: FileStamp,
    ) -> Option<crate::tags::TagInfo> {
        self.read_observation(path, format, stamp).info().cloned()
    }

    fn read_observation(
        &mut self,
        path: &Path,
        format: AudioFormat,
        stamp: FileStamp,
    ) -> crate::tags::TagObservation {
        let previous = self.index.tag_observation(path).ok().flatten();
        if let Ok(Some(cached)) = self.index.cached_observation(path, stamp)
            && !cached.result.retryable()
            && !self.forced_paths.contains(path)
        {
            return cached;
        }
        self.reads += 1;
        let read_started_at = tag_cache::next_read_order();
        let mut observation = self.inner.read_observation(path, format, stamp);
        if observation.result.reason().is_some() {
            observation.last_success = previous.and_then(|old| {
                Some(crate::tags::SuccessfulTags {
                    info: old.info()?.clone(),
                    observed_at: old.data_observed_at()?,
                })
            });
        }
        // A cache write failure cannot turn a successful source read into missing metadata.
        if matches!(
            self.index
                .cache_observation(path, stamp, &observation, read_started_at),
            Ok(false)
        ) && let Ok(Some(newer)) = self.index.tag_observation(path)
        {
            return newer;
        }
        observation
    }
}
