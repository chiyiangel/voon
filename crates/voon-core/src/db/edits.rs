use std::path::Path;

use rusqlite::{OptionalExtension, params};

use super::{Index, Result, p};
use crate::{ALBUM_FIELDS, AlbumMetadata};

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("未找到专辑")]
    MissingAlbum,
    #[error("不支持的专辑字段：{0}")]
    InvalidField(String),
    #[error("年份应为 1 到 9999 的整数，或留空使用自动值")]
    InvalidYear,
    #[error(transparent)]
    Database(#[from] rusqlite::Error),
}

impl Index {
    /// Strongest automatic values, before the user's local overrides.
    pub fn automatic_metadata(&self, album: &Path) -> Result<Option<AlbumMetadata>> {
        let id = self
            .conn
            .query_row("SELECT id FROM album WHERE path = ?1", [p(album)], |row| {
                row.get::<_, i64>(0)
            })
            .optional()?;
        id.map(|id| self.album_metadata(id, album, false))
            .transpose()
    }

    /// Save a validated form in one transaction; `None` or blank text restores
    /// the automatic value. Unchanged fields should be omitted.
    pub fn save_user_edits(
        &mut self,
        album: &Path,
        edits: &[(String, Option<String>)],
    ) -> std::result::Result<(), EditError> {
        let mut validated = Vec::with_capacity(edits.len());
        for (field, value) in edits {
            if !ALBUM_FIELDS.contains(&field.as_str()) {
                return Err(EditError::InvalidField(field.clone()));
            }
            let value = value
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty());
            if field == "year"
                && value.is_some_and(|value| {
                    !value
                        .parse::<u16>()
                        .is_ok_and(|year| (1..=9999).contains(&year))
                })
            {
                return Err(EditError::InvalidYear);
            }
            validated.push((field, value));
        }
        let tx = self.conn.transaction()?;
        if !tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM album WHERE path = ?1)",
            [p(album)],
            |row| row.get::<_, bool>(0),
        )? {
            return Err(EditError::MissingAlbum);
        }
        for (field, value) in validated {
            if let Some(value) = value {
                tx.execute(
                    "INSERT INTO user_edit (album_path, name, value) VALUES (?1, ?2, ?3)
                    ON CONFLICT (album_path, name) DO UPDATE SET value = excluded.value",
                    params![p(album), field, value],
                )?;
            } else {
                tx.execute(
                    "DELETE FROM user_edit WHERE album_path = ?1 AND name = ?2",
                    params![p(album), field],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}
