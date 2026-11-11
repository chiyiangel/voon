//! The local index: a SQLite database on the Mac, so browsing works while
//! the NAS is unmounted.
//!
//! Scan results are rebuilt per Library root on every scan. What the user
//! owns (field edits, user tags, manual collections) is keyed by album path
//! instead of row id, so it survives rescans.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

use crate::model::*;
use crate::scan::{FileStamp, ScanResult, TagReader};
use crate::tags::TagInfo;

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

pub struct Index {
    conn: Connection,
}

/// An album as read back from the index, with user edits applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlbumRow {
    pub id: i64,
    pub path: PathBuf,
    pub folder_name: String,
    pub collection: Option<PathBuf>,
    pub cover: Option<Cover>,
    pub metadata: AlbumMetadata,
    pub track_count: u32,
}

impl AlbumRow {
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

impl Index {
    pub fn open(path: &Path) -> Result<Self> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        let mut index = Self { conn };
        index.migrate()?;
        Ok(index)
    }

    pub fn schema_version(&self) -> Result<u32> {
        self.conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
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

    /// Replace everything scanned under `root` with `scan`. A cancelled scan
    /// is stored as well; it simply holds fewer albums.
    pub fn store_scan(&mut self, root: &Path, scan: &ScanResult) -> Result<()> {
        let root_id = self.add_root(root)?;
        let tx = self.conn.transaction()?;
        let before: Vec<(String, String)> = {
            let mut st = tx.prepare("SELECT path, signature FROM album WHERE root_id = ?1")?;
            st.query_map([root_id], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<_>>()?
        };
        tx.execute("DELETE FROM album WHERE root_id = ?1", [root_id])?;
        tx.execute("DELETE FROM collection WHERE root_id = ?1", [root_id])?;
        tx.execute("DELETE FROM scan_issue WHERE root_id = ?1", [root_id])?;
        for c in &scan.collections {
            tx.execute(
                "INSERT INTO collection (root_id, path, name, parent_path) VALUES (?1, ?2, ?3, ?4)",
                params![root_id, p(&c.path), c.name, c.parent.as_deref().map(p)],
            )?;
        }
        for a in &scan.albums {
            let (cover_path, embedded) = match &a.cover {
                Some(Cover {
                    source: CoverSource::File(f),
                }) => (Some(p(f)), false),
                Some(Cover {
                    source: CoverSource::Embedded(f),
                }) => (Some(p(f)), true),
                None => (None, false),
            };
            tx.execute(
                "INSERT INTO album (root_id, path, folder_name, collection_path, cover_path, cover_embedded, signature)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    root_id,
                    p(&a.path),
                    a.folder_name,
                    a.collection.as_deref().map(p),
                    cover_path,
                    embedded,
                    a.signature
                ],
            )?;
            let album_id = tx.last_insert_rowid();
            for (name, value, source) in a.metadata.entries() {
                tx.execute(
                    "INSERT INTO album_field (album_id, name, source, value) VALUES (?1, ?2, ?3, ?4)",
                    params![album_id, name, source.as_str(), value],
                )?;
            }
            for d in &a.discs {
                tx.execute(
                    "INSERT INTO disc (album_id, number, path) VALUES (?1, ?2, ?3)",
                    params![album_id, d.number, p(&d.path)],
                )?;
                let disc_id = tx.last_insert_rowid();
                for t in &d.tracks {
                    let (cue, start, end) = match &t.source {
                        TrackSource::File { .. } => (None, None, None),
                        TrackSource::Virtual {
                            cue, start, end, ..
                        } => (Some(p(cue)), Some(*start), *end),
                    };
                    tx.execute(
                        "INSERT INTO track (disc_id, number, title, title_source, artist, artist_source,
                           format, path, cue_path, start_frames, end_frames, sample_rate, bit_depth,
                           channels, duration_ms)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                        params![
                            disc_id,
                            t.number,
                            t.title.as_ref().map(|f| &f.value),
                            t.title.as_ref().map(|f| f.source.as_str()),
                            t.artist.as_ref().map(|f| &f.value),
                            t.artist.as_ref().map(|f| f.source.as_str()),
                            t.format.label(),
                            p(t.source.audio_path()),
                            cue,
                            start,
                            end,
                            t.properties.sample_rate,
                            t.properties.bit_depth,
                            t.properties.channels,
                            t.properties.duration_ms.map(|d| d as i64),
                        ],
                    )?;
                }
            }
        }
        for issue in &scan.issues {
            let detail = format!("{:?}", issue.kind);
            let kind = detail
                .split([' ', '{', '('])
                .next()
                .unwrap_or("")
                .to_string();
            tx.execute(
                "INSERT INTO scan_issue (root_id, path, kind, detail) VALUES (?1, ?2, ?3, ?4)",
                params![root_id, p(&issue.path), kind, detail],
            )?;
        }
        record_rename_candidates(&tx, &before, &scan.albums)?;
        tx.execute(
            "UPDATE library_root SET last_scan = strftime('%s','now') WHERE id = ?1",
            [root_id],
        )?;
        tx.commit()
    }

    /// All albums, sorted by display title, with user edits applied.
    pub fn albums(&self) -> Result<Vec<AlbumRow>> {
        let mut st = self.conn.prepare(
            "SELECT a.id, a.path, a.folder_name, a.collection_path, a.cover_path, a.cover_embedded,
                    (SELECT COUNT(*) FROM track t JOIN disc d ON t.disc_id = d.id WHERE d.album_id = a.id)
             FROM album a ORDER BY a.folder_name",
        )?;
        let mut rows: Vec<AlbumRow> = st
            .query_map([], |r| {
                let cover = r.get::<_, Option<String>>(4)?.map(|c| {
                    let c = PathBuf::from(c);
                    Cover {
                        source: if r.get::<_, bool>(5).unwrap_or(false) {
                            CoverSource::Embedded(c)
                        } else {
                            CoverSource::File(c)
                        },
                    }
                });
                Ok(AlbumRow {
                    id: r.get(0)?,
                    path: PathBuf::from(r.get::<_, String>(1)?),
                    folder_name: r.get(2)?,
                    collection: r.get::<_, Option<String>>(3)?.map(PathBuf::from),
                    cover,
                    metadata: AlbumMetadata::default(),
                    track_count: r.get(6)?,
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
        tx.execute(
            "DELETE FROM rename_candidate WHERE old_path = ?1 OR new_path = ?2",
            params![old, new],
        )?;
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
            "SELECT album_path FROM manual_collection_album WHERE collection_id = ?1 ORDER BY position",
        )?;
        st.query_map([collection], |r| Ok(PathBuf::from(r.get::<_, String>(0)?)))?
            .collect()
    }

    fn cached_tags(&self, path: &Path, stamp: FileStamp) -> Result<Option<Option<TagInfo>>> {
        self.conn
            .query_row(
                "SELECT readable, title, artist, album, album_artist, composer, conductor, genre,
                        year, track, disc, catalog_number, has_picture, sample_rate, bit_depth,
                        channels, duration_ms
                 FROM file_cache WHERE path = ?1 AND size = ?2 AND mtime = ?3",
                params![p(path), stamp.size as i64, stamp.mtime],
                |r| {
                    if !r.get::<_, bool>(0)? {
                        return Ok(None);
                    }
                    Ok(Some(TagInfo {
                        title: r.get(1)?,
                        artist: r.get(2)?,
                        album: r.get(3)?,
                        album_artist: r.get(4)?,
                        composer: r.get(5)?,
                        conductor: r.get(6)?,
                        genre: r.get(7)?,
                        year: r.get(8)?,
                        track: r.get(9)?,
                        disc: r.get(10)?,
                        catalog_number: r.get(11)?,
                        has_picture: r.get(12)?,
                        properties: AudioProperties {
                            sample_rate: r.get(13)?,
                            bit_depth: r.get(14)?,
                            channels: r.get(15)?,
                            duration_ms: r.get::<_, Option<i64>>(16)?.map(|d| d as u64),
                        },
                    }))
                },
            )
            .optional()
    }

    fn cache_tags(&self, path: &Path, stamp: FileStamp, info: Option<&TagInfo>) -> Result<()> {
        let d = TagInfo::default();
        let t = info.unwrap_or(&d);
        self.conn.execute(
            "INSERT OR REPLACE INTO file_cache (path, size, mtime, readable, title, artist, album,
               album_artist, composer, conductor, genre, year, track, disc, catalog_number,
               has_picture, sample_rate, bit_depth, channels, duration_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                p(path),
                stamp.size as i64,
                stamp.mtime,
                info.is_some(),
                t.title,
                t.artist,
                t.album,
                t.album_artist,
                t.composer,
                t.conductor,
                t.genre,
                t.year,
                t.track,
                t.disc,
                t.catalog_number,
                t.has_picture,
                t.properties.sample_rate,
                t.properties.bit_depth,
                t.properties.channels,
                t.properties.duration_ms.map(|d| d as i64),
            ],
        )?;
        Ok(())
    }
}

/// Pair album paths that vanished in this scan with paths that appeared,
/// when exactly one of each shares a content signature. Ambiguous matches
/// (duplicate rips) are not proposed. Stale candidates are pruned.
fn record_rename_candidates(
    tx: &rusqlite::Transaction<'_>,
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
    // A candidate is stale once its new path is gone or its old path is back.
    tx.execute(
        "DELETE FROM rename_candidate
         WHERE new_path NOT IN (SELECT path FROM album) OR old_path IN (SELECT path FROM album)",
        [],
    )?;
    Ok(())
}

/// A tag reader that consults the index first and reads the file only when
/// its size or modification time changed since the last scan.
pub struct CachedTags<'a, R: TagReader> {
    pub index: &'a Index,
    pub inner: R,
    /// Files actually read from disk during this scan.
    pub reads: usize,
}

impl<'a, R: TagReader> CachedTags<'a, R> {
    pub fn new(index: &'a Index, inner: R) -> Self {
        Self {
            index,
            inner,
            reads: 0,
        }
    }
}

impl<R: TagReader> TagReader for CachedTags<'_, R> {
    fn read(&mut self, path: &Path, format: AudioFormat, stamp: FileStamp) -> Option<TagInfo> {
        if let Ok(Some(cached)) = self.index.cached_tags(path, stamp) {
            return cached;
        }
        self.reads += 1;
        let info = self.inner.read(path, format, stamp);
        let _ = self.index.cache_tags(path, stamp, info.as_ref());
        info
    }
}
