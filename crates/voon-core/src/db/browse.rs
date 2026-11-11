//! Stable discovery order for browsing, independent of rebuilt scan rows.

use std::{collections::HashMap, path::Path};

use rusqlite::{Transaction, params};

use super::{AlbumRow, Index, Result, p};
use crate::{Album, AudioFormat};

pub(super) const MIGRATION: &str = r#"
CREATE TABLE album_added (
    added_order INTEGER PRIMARY KEY AUTOINCREMENT,
    root_path TEXT NOT NULL,
    album_path TEXT NOT NULL UNIQUE
);
INSERT INTO album_added (root_path, album_path)
SELECT r.path, a.path FROM album a JOIN library_root r ON r.id = a.root_id ORDER BY a.id;
"#;

pub(super) fn record_album_added(
    tx: &Transaction<'_>,
    root: &Path,
    albums: &[Album],
) -> Result<()> {
    let mut insert =
        tx.prepare("INSERT OR IGNORE INTO album_added (root_path, album_path) VALUES (?1, ?2)")?;
    for album in albums {
        insert.execute(params![p(root), p(&album.path)])?;
    }
    Ok(())
}

impl Index {
    pub(super) fn populate_browse_fields(&self, albums: &mut [AlbumRow]) -> Result<()> {
        let mut statement = self
            .conn
            .prepare("SELECT album_path, added_order FROM album_added")?;
        let order: HashMap<String, i64> = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_>>()?;
        let by_id: HashMap<i64, usize> = albums
            .iter()
            .enumerate()
            .map(|(index, album)| (album.id, index))
            .collect();
        let mut genres = self.conn.prepare(
            "SELECT album_id, value FROM album_field WHERE name = 'genre' AND source = 'tag'",
        )?;
        let genres: HashMap<i64, String> = genres
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_>>()?;
        let mut tracks = self.conn.prepare(
            "SELECT d.album_id, t.format, MAX(t.sample_rate), MAX(t.bit_depth)
             FROM track t JOIN disc d ON d.id = t.disc_id GROUP BY d.album_id, t.format",
        )?;
        for row in tracks.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<u32>>(2)?,
                row.get::<_, Option<u8>>(3)?,
            ))
        })? {
            let (id, format, rate, bits) = row?;
            let Some(&position) = by_id.get(&id) else {
                continue;
            };
            let format = match format.as_str() {
                "WavPack" => Some(AudioFormat::WavPack),
                "Ogg Vorbis" => Some(AudioFormat::Ogg),
                format => AudioFormat::from_extension(format),
            };
            let facets = &mut albums[position].facets;
            if let Some(format) = format {
                facets.formats.insert(format);
            }
            facets.max_sample_rate = facets.max_sample_rate.max(rate);
            facets.max_bit_depth = facets.max_bit_depth.max(bits);
        }
        for album in albums {
            album.added_order = *order.get(&p(&album.path)).unwrap_or(&0);
            album.facets = crate::facets::from_summary(
                &album.folder_name,
                &album.metadata,
                std::mem::take(&mut album.facets.formats),
                album.facets.max_sample_rate,
                album.facets.max_bit_depth,
            );
            album.facets.genre = genres
                .get(&album.id)
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .map(str::to_owned);
        }
        Ok(())
    }
}
