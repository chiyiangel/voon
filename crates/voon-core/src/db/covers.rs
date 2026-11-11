use super::{AlbumRow, Index, Result, p};
use crate::{Cover, CoverSource};
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub(super) const MIGRATION: &str = r#"
ALTER TABLE album ADD COLUMN cover_picture_index INTEGER;
CREATE TABLE user_cover (
    album_path TEXT PRIMARY KEY,
    cover_path TEXT NOT NULL,
    embedded INTEGER NOT NULL,
    picture_index INTEGER
);
"#;

pub(super) const SOURCE_DIRECTORIES_MIGRATION: &str =
    "ALTER TABLE album ADD COLUMN source_directories TEXT NOT NULL DEFAULT '[]';";

pub(super) fn parts(cover: &Cover) -> (String, bool, Option<u32>) {
    match &cover.source {
        CoverSource::File(path) => (p(path), false, None),
        CoverSource::Embedded(path) => (p(path), true, None),
        CoverSource::EmbeddedPicture { path, index } => (p(path), true, Some(*index)),
    }
}

pub(super) fn from_parts(path: PathBuf, embedded: bool, index: Option<u32>) -> Cover {
    Cover {
        source: if !embedded {
            CoverSource::File(path)
        } else if let Some(index) = index {
            CoverSource::EmbeddedPicture { path, index }
        } else {
            CoverSource::Embedded(path)
        },
    }
}

pub(super) fn read_user_cover(conn: &Connection, album: &Path) -> Result<Option<Cover>> {
    conn.query_row(
        "SELECT cover_path, embedded, picture_index FROM user_cover WHERE album_path = ?1",
        [p(album)],
        |row| {
            Ok(from_parts(
                PathBuf::from(row.get::<_, String>(0)?),
                row.get(1)?,
                row.get(2)?,
            ))
        },
    )
    .optional()
}

/// Transaction-friendly primitives are also used by structural edit undo.
pub(super) fn write_user_cover(
    conn: &Connection,
    album: &Path,
    cover: Option<&Cover>,
) -> Result<()> {
    if let Some(cover) = cover {
        let (path, embedded, index) = parts(cover);
        conn.execute("INSERT INTO user_cover(album_path, cover_path, embedded, picture_index) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(album_path) DO UPDATE SET cover_path=excluded.cover_path, embedded=excluded.embedded, picture_index=excluded.picture_index", params![p(album), path, embedded, index])?;
    } else {
        conn.execute("DELETE FROM user_cover WHERE album_path = ?1", [p(album)])?;
    }
    Ok(())
}

pub(super) fn migrate_user_cover(conn: &Connection, old: &Path, new: &Path) -> Result<usize> {
    if old == new {
        return Ok(0);
    }
    let Some(mut cover) = read_user_cover(conn, old)? else {
        return Ok(0);
    };
    let path = match &mut cover.source {
        CoverSource::File(path)
        | CoverSource::Embedded(path)
        | CoverSource::EmbeddedPicture { path, .. } => path,
    };
    if let Ok(relative) = path.strip_prefix(old) {
        *path = new.join(relative);
    }
    let (path, embedded, index) = parts(&cover);
    let moved = conn.execute("INSERT OR IGNORE INTO user_cover(album_path,cover_path,embedded,picture_index) VALUES (?1,?2,?3,?4)", params![p(new), path, embedded, index])?;
    write_user_cover(conn, old, None)?;
    Ok(moved)
}

impl Index {
    pub fn automatic_cover(&self, album: &Path) -> Result<Option<Cover>> {
        self.conn
            .query_row(
                "SELECT cover_path, cover_embedded, cover_picture_index FROM album WHERE path=?1",
                [p(album)],
                |row| {
                    Ok(row.get::<_, Option<String>>(0)?.map(|path| {
                        from_parts(
                            path.into(),
                            row.get::<_, bool>(1).unwrap_or(false),
                            row.get::<_, Option<u32>>(2).unwrap_or(None),
                        )
                    }))
                },
            )
            .optional()
            .map(Option::flatten)
    }
    pub fn user_cover(&self, album: &Path) -> Result<Option<Cover>> {
        read_user_cover(&self.conn, album)
    }
    pub fn set_user_cover(&self, album: &Path, cover: &Cover) -> Result<()> {
        let exists: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM album WHERE path=?1)",
            [p(album)],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        write_user_cover(&self.conn, album, Some(cover))
    }
    pub fn clear_user_cover(&self, album: &Path) -> Result<()> {
        write_user_cover(&self.conn, album, None)
    }

    pub(super) fn apply_user_covers(&self, albums: &mut [AlbumRow]) -> Result<()> {
        let mut statement = self
            .conn
            .prepare("SELECT album_path,cover_path,embedded,picture_index FROM user_cover")?;
        let covers: HashMap<_, _> = statement
            .query_map([], |row| {
                Ok((
                    PathBuf::from(row.get::<_, String>(0)?),
                    from_parts(
                        PathBuf::from(row.get::<_, String>(1)?),
                        row.get(2)?,
                        row.get(3)?,
                    ),
                ))
            })?
            .collect::<Result<_>>()?;
        for album in albums {
            if let Some(cover) = covers.get(&album.path) {
                album.cover = Some(cover.clone());
            }
        }
        Ok(())
    }
}
