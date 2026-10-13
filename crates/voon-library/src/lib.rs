//! SQLite library schema for Voon.
//!
//! Scanning is a later slice. This crate creates the tracks table, including
//! DSD files that stay `playable = 0` until a phase-2 DoP path flips the flag,
//! plus the file size and mtime used by incremental scans and an FTS5 index.

use std::path::Path;
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};

const SCHEMA_VERSION: i32 = 1;
const LIST_LIMIT: i64 = 200;

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("library schema version {found} is newer than this build (version {expected})")]
    Schema { found: i32, expected: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecKind {
    Pcm,
    Dsd,
}

impl CodecKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pcm => "pcm",
            Self::Dsd => "dsd",
        }
    }

    fn parse(value: &str) -> Result<Self, LibraryError> {
        match value {
            "pcm" => Ok(Self::Pcm),
            "dsd" => Ok(Self::Dsd),
            other => Err(LibraryError::Sqlite(
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    format!("unknown codec_kind {other}").into(),
                ),
            )),
        }
    }
}

/// How a path should be indexed before a scanner has opened the file.
///
/// `.m4a` / `.mp4` stay unplayable until a later scanner confirms ALAC. AAC
/// must not appear in the default library view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexPolicy {
    pub codec_kind: CodecKind,
    pub playable: bool,
    pub container: &'static str,
}

pub fn index_policy(path: &Path) -> Option<IndexPolicy> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "flac" => Some(policy(CodecKind::Pcm, true, "flac")),
        "wav" | "wave" => Some(policy(CodecKind::Pcm, true, "wav")),
        "aiff" | "aif" => Some(policy(CodecKind::Pcm, true, "aiff")),
        "alac" => Some(policy(CodecKind::Pcm, true, "alac")),
        "dsf" => Some(policy(CodecKind::Dsd, false, "dsf")),
        "dff" => Some(policy(CodecKind::Dsd, false, "dff")),
        "m4a" => Some(policy(CodecKind::Pcm, false, "m4a")),
        "mp4" => Some(policy(CodecKind::Pcm, false, "mp4")),
        _ => None,
    }
}

fn policy(codec_kind: CodecKind, playable: bool, container: &'static str) -> IndexPolicy {
    IndexPolicy {
        codec_kind,
        playable,
        container,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTrack {
    pub path: String,
    pub file_size: i64,
    pub mtime_ns: i64,
    pub codec_kind: CodecKind,
    pub playable: bool,
    pub container: Option<String>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub bits_per_sample: Option<i64>,
    pub duration_ms: Option<i64>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
    pub year: Option<i64>,
    pub genre: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub id: i64,
    pub path: String,
    pub file_size: i64,
    pub mtime_ns: i64,
    pub codec_kind: CodecKind,
    pub playable: bool,
    pub container: Option<String>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub bits_per_sample: Option<i64>,
    pub duration_ms: Option<i64>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
    pub year: Option<i64>,
    pub genre: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanStamp {
    pub file_size: i64,
    pub mtime_ns: i64,
}

pub struct Library {
    conn: Connection,
}

impl Library {
    pub fn open(path: &Path) -> Result<Self, LibraryError> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|error| {
                    LibraryError::Sqlite(rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CANTOPEN),
                        Some(format!("failed to create {}: {error}", parent.display())),
                    ))
                })?;
            }
        }
        let conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Ok(Self { conn })
    }

    pub fn migrate(&self) -> Result<(), LibraryError> {
        let version: i32 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version == SCHEMA_VERSION {
            return Ok(());
        }
        if version > SCHEMA_VERSION {
            return Err(LibraryError::Schema {
                found: version,
                expected: SCHEMA_VERSION,
            });
        }
        if version != 0 {
            return Err(LibraryError::Schema {
                found: version,
                expected: SCHEMA_VERSION,
            });
        }
        self.conn.execute_batch(SCHEMA)?;
        self.conn
            .pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(())
    }

    /// Insert or replace metadata for `path`. The rowid is preserved on update
    /// so the FTS5 external-content trigger stays consistent.
    pub fn upsert_track(&self, track: &NewTrack) -> Result<i64, LibraryError> {
        self.conn.execute(
            "INSERT INTO tracks (
                path, file_size, mtime_ns, codec_kind, playable, container,
                sample_rate, channels, bits_per_sample, duration_ms,
                title, artist, album, album_artist, track_number, disc_number, year, genre
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18
            )
            ON CONFLICT(path) DO UPDATE SET
                file_size = excluded.file_size,
                mtime_ns = excluded.mtime_ns,
                codec_kind = excluded.codec_kind,
                playable = excluded.playable,
                container = excluded.container,
                sample_rate = excluded.sample_rate,
                channels = excluded.channels,
                bits_per_sample = excluded.bits_per_sample,
                duration_ms = excluded.duration_ms,
                title = excluded.title,
                artist = excluded.artist,
                album = excluded.album,
                album_artist = excluded.album_artist,
                track_number = excluded.track_number,
                disc_number = excluded.disc_number,
                year = excluded.year,
                genre = excluded.genre",
            params![
                track.path,
                track.file_size,
                track.mtime_ns,
                track.codec_kind.as_str(),
                i64::from(track.playable),
                track.container,
                track.sample_rate,
                track.channels,
                track.bits_per_sample,
                track.duration_ms,
                track.title,
                track.artist,
                track.album,
                track.album_artist,
                track.track_number,
                track.disc_number,
                track.year,
                track.genre,
            ],
        )?;
        let id = self.conn.query_row(
            "SELECT id FROM tracks WHERE path = ?1",
            params![track.path],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    pub fn scan_stamp(&self, path: &str) -> Result<Option<ScanStamp>, LibraryError> {
        self.conn
            .query_row(
                "SELECT file_size, mtime_ns FROM tracks WHERE path = ?1",
                params![path],
                |row| {
                    Ok(ScanStamp {
                        file_size: row.get(0)?,
                        mtime_ns: row.get(1)?,
                    })
                },
            )
            .optional()
            .map_err(LibraryError::from)
    }

    pub fn list_tracks(&self, include_unplayable: bool) -> Result<Vec<Track>, LibraryError> {
        let mut statement = self.conn.prepare(
            "SELECT
                id, path, file_size, mtime_ns, codec_kind, playable, container,
                sample_rate, channels, bits_per_sample, duration_ms,
                title, artist, album, album_artist, track_number, disc_number, year, genre
             FROM tracks
             WHERE (?1 = 1 OR playable = 1)
             ORDER BY album_artist, album, disc_number, track_number, path
             LIMIT ?2",
        )?;
        let rows = statement.query_map(
            params![i64::from(include_unplayable), LIST_LIMIT],
            map_track,
        )?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(LibraryError::from)
    }

    /// Full-text search. An empty query returns no rows. Unplayable tracks,
    /// including every DSD file in phase 1, are hidden unless
    /// `include_unplayable` is set.
    pub fn search(
        &self,
        query: &str,
        include_unplayable: bool,
    ) -> Result<Vec<Track>, LibraryError> {
        let Some(match_query) = fts_query(query) else {
            return Ok(Vec::new());
        };
        let mut statement = self.conn.prepare(
            "SELECT
                tracks.id, tracks.path, tracks.file_size, tracks.mtime_ns, tracks.codec_kind,
                tracks.playable, tracks.container, tracks.sample_rate, tracks.channels,
                tracks.bits_per_sample, tracks.duration_ms, tracks.title, tracks.artist,
                tracks.album, tracks.album_artist, tracks.track_number, tracks.disc_number,
                tracks.year, tracks.genre
             FROM tracks
             JOIN tracks_fts ON tracks_fts.rowid = tracks.id
             WHERE tracks_fts MATCH ?1
               AND (?2 = 1 OR tracks.playable = 1)
             ORDER BY tracks.album_artist, tracks.album, tracks.disc_number, tracks.track_number, tracks.path
             LIMIT ?3",
        )?;
        let rows = statement.query_map(
            params![match_query, i64::from(include_unplayable), LIST_LIMIT],
            map_track,
        )?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(LibraryError::from)
    }
}

fn map_track(row: &rusqlite::Row<'_>) -> rusqlite::Result<Track> {
    let codec: String = row.get(4)?;
    let playable: i64 = row.get(5)?;
    Ok(Track {
        id: row.get(0)?,
        path: row.get(1)?,
        file_size: row.get(2)?,
        mtime_ns: row.get(3)?,
        codec_kind: CodecKind::parse(&codec).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, error.into())
        })?,
        playable: playable != 0,
        container: row.get(6)?,
        sample_rate: row.get(7)?,
        channels: row.get(8)?,
        bits_per_sample: row.get(9)?,
        duration_ms: row.get(10)?,
        title: row.get(11)?,
        artist: row.get(12)?,
        album: row.get(13)?,
        album_artist: row.get(14)?,
        track_number: row.get(15)?,
        disc_number: row.get(16)?,
        year: row.get(17)?,
        genre: row.get(18)?,
    })
}

/// Turn user text into an FTS5 prefix query. Tokens keep letters, digits, and
/// apostrophes; everything else is dropped so the query cannot change the syntax.
fn fts_query(input: &str) -> Option<String> {
    let mut tokens = Vec::new();
    for raw in input.split_whitespace() {
        let token: String = raw
            .chars()
            .filter(|character| character.is_alphanumeric() || *character == '\'')
            .collect();
        if !token.is_empty() {
            tokens.push(format!("\"{token}\""));
        }
    }
    if tokens.is_empty() {
        None
    } else {
        Some(
            tokens
                .into_iter()
                .map(|token| format!("{token}*"))
                .collect::<Vec<_>>()
                .join(" "),
        )
    }
}

const SCHEMA: &str = r#"
CREATE TABLE tracks (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    file_size INTEGER NOT NULL,
    mtime_ns INTEGER NOT NULL,
    codec_kind TEXT NOT NULL CHECK (codec_kind IN ('pcm', 'dsd')),
    playable INTEGER NOT NULL CHECK (playable IN (0, 1)),
    container TEXT,
    sample_rate INTEGER,
    channels INTEGER,
    bits_per_sample INTEGER,
    duration_ms INTEGER,
    title TEXT,
    artist TEXT,
    album TEXT,
    album_artist TEXT,
    track_number INTEGER,
    disc_number INTEGER,
    year INTEGER,
    genre TEXT
);

CREATE INDEX idx_tracks_playable_album
    ON tracks(playable, album_artist, album, disc_number, track_number);

CREATE VIRTUAL TABLE tracks_fts USING fts5(
    title,
    artist,
    album,
    album_artist,
    genre,
    content='tracks',
    content_rowid='id',
    tokenize='unicode61'
);

CREATE TRIGGER tracks_ai AFTER INSERT ON tracks BEGIN
    INSERT INTO tracks_fts(rowid, title, artist, album, album_artist, genre)
    VALUES (new.id, new.title, new.artist, new.album, new.album_artist, new.genre);
END;

CREATE TRIGGER tracks_ad AFTER DELETE ON tracks BEGIN
    INSERT INTO tracks_fts(tracks_fts, rowid, title, artist, album, album_artist, genre)
    VALUES ('delete', old.id, old.title, old.artist, old.album, old.album_artist, old.genre);
END;

CREATE TRIGGER tracks_au AFTER UPDATE ON tracks BEGIN
    INSERT INTO tracks_fts(tracks_fts, rowid, title, artist, album, album_artist, genre)
    VALUES ('delete', old.id, old.title, old.artist, old.album, old.album_artist, old.genre);
    INSERT INTO tracks_fts(rowid, title, artist, album, album_artist, genre)
    VALUES (new.id, new.title, new.artist, new.album, new.album_artist, new.genre);
END;
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("voon-library-{}-{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("library.db")
    }

    fn track(path: &str, title: &str, kind: CodecKind, playable: bool) -> NewTrack {
        NewTrack {
            path: path.into(),
            file_size: 100,
            mtime_ns: 50,
            codec_kind: kind,
            playable,
            container: Some("flac".into()),
            sample_rate: Some(96000),
            channels: Some(2),
            bits_per_sample: Some(24),
            duration_ms: Some(1000),
            title: Some(title.into()),
            artist: Some("Miles Davis".into()),
            album: Some("Kind of Blue".into()),
            album_artist: Some("Miles Davis".into()),
            track_number: Some(1),
            disc_number: Some(1),
            year: Some(1959),
            genre: Some("Jazz".into()),
        }
    }

    #[test]
    fn schema_hides_dsd_and_keeps_scan_stamps() {
        let path = temp_db("schema");
        let library = Library::open(&path).unwrap();
        library.migrate().unwrap();
        library.migrate().unwrap();
        let pcm_id = library
            .upsert_track(&track("/music/blue.flac", "So What", CodecKind::Pcm, true))
            .unwrap();
        library
            .upsert_track(&track(
                "/music/blue.dsf",
                "So What DSD",
                CodecKind::Dsd,
                false,
            ))
            .unwrap();
        let updated = track("/music/blue.flac", "So What", CodecKind::Pcm, true);
        let mut updated = updated;
        updated.file_size = 200;
        updated.mtime_ns = 80;
        let again = library.upsert_track(&updated).unwrap();
        assert_eq!(pcm_id, again);

        let visible = library.search("so what", false).unwrap();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].path, "/music/blue.flac");
        assert_eq!(visible[0].codec_kind, CodecKind::Pcm);
        let all = library.search("so what", true).unwrap();
        assert_eq!(all.len(), 2);
        assert!(library.search("   ", false).unwrap().is_empty());
        assert_eq!(library.list_tracks(false).unwrap().len(), 1);
        assert_eq!(library.list_tracks(true).unwrap().len(), 2);
        assert_eq!(
            library.scan_stamp("/music/blue.flac").unwrap(),
            Some(ScanStamp {
                file_size: 200,
                mtime_ns: 80,
            })
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn newer_schema_is_rejected() {
        let path = temp_db("future");
        let library = Library::open(&path).unwrap();
        library.migrate().unwrap();
        library
            .conn
            .pragma_update(None, "user_version", 9i32)
            .unwrap();
        let error = library.migrate().unwrap_err();
        assert!(matches!(error, LibraryError::Schema { found: 9, .. }));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn index_policy_marks_dsd_and_unconfirmed_m4a_unplayable() {
        assert_eq!(
            index_policy(Path::new("/music/a.FLAC")),
            Some(IndexPolicy {
                codec_kind: CodecKind::Pcm,
                playable: true,
                container: "flac",
            })
        );
        assert_eq!(
            index_policy(Path::new("track.dsf")).map(|policy| (policy.codec_kind, policy.playable)),
            Some((CodecKind::Dsd, false))
        );
        assert_eq!(
            index_policy(Path::new("track.dff")).map(|policy| policy.playable),
            Some(false)
        );
        assert_eq!(
            index_policy(Path::new("song.m4a")).map(|policy| (policy.codec_kind, policy.playable)),
            Some((CodecKind::Pcm, false))
        );
        assert!(index_policy(Path::new("notes.txt")).is_none());
        assert_eq!(
            index_policy(Path::new("song.aif")).map(|policy| policy.container),
            Some("aiff")
        );
    }
}
