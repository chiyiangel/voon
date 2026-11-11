//! Complete Album queries stay local, including when the Library root is offline.
use super::{Index, Result, p};
use crate::model::*;
use rusqlite::{Connection, OptionalExtension, Row, params};
use std::path::{Path, PathBuf};

impl Index {
    /// One Album with user edits applied and all Disc and Track details.
    pub fn album(&self, path: &Path) -> Result<Option<Album>> {
        stored_album(&self.conn, path, true)
    }
    pub(super) fn album_metadata(
        &self,
        album_id: i64,
        path: &Path,
        include_user_edits: bool,
    ) -> Result<AlbumMetadata> {
        read_metadata(&self.conn, album_id, path, include_user_edits)
    }
}

pub(super) fn stored_album(
    conn: &Connection,
    path: &Path,
    include_user_data: bool,
) -> Result<Option<Album>> {
    let found = conn
            .query_row(
                "SELECT id, folder_name, collection_path, cover_path, cover_embedded, signature, cover_picture_index, source_directories,candidates_json
             FROM album WHERE path = ?1",
                [p(path)],
                |row| {
                    let cover = row.get::<_, Option<String>>(3)?.map(|cover| super::covers::from_parts(cover.into(), row.get::<_, bool>(4).unwrap_or(false), row.get::<_, Option<u32>>(6).unwrap_or(None)));
                    Ok((
                        row.get::<_, i64>(0)?,
                        Album {
                            path: path.to_path_buf(),
                            source_directories: read_source_directories(&row.get::<_, String>(7)?)?,
                            folder_name: row.get(1)?,
                            collection: row.get::<_, Option<String>>(2)?.map(PathBuf::from),
                            cover,
                            signature: row.get(5)?,
                            metadata: AlbumMetadata::default(),
                            candidates: serde_json::from_str(&row.get::<_,String>(8)?).map_err(|error| rusqlite::Error::FromSqlConversionFailure(8,rusqlite::types::Type::Text,Box::new(error)))?,
                            discs: Vec::new(),
                        },
                    ))
                },
            )
            .optional()?;
    let Some((album_id, mut album)) = found else {
        return Ok(None);
    };
    album.metadata = read_metadata(conn, album_id, path, include_user_data)?;
    let mut discs =
        conn.prepare("SELECT id, number, path FROM disc WHERE album_id = ?1 ORDER BY number, id")?;
    let mut tracks = conn.prepare(
            "SELECT number, title, title_source, artist, artist_source, format, path,
                    cue_path, start_frames, end_frames, sample_rate, bit_depth, channels, duration_ms
             FROM track WHERE disc_id = ?1 ORDER BY number IS NULL, number, id",
        )?;
    let rows = discs.query_map([album_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, u32>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (disc_id, number, path) = row?;
        album.discs.push(Disc {
            number,
            path: path.into(),
            tracks: tracks
                .query_map([disc_id], read_track)?
                .collect::<Result<_>>()?,
        });
    }
    if include_user_data {
        album.cover = super::covers::read_user_cover(conn, path)?.or(album.cover);
    }
    Ok(Some(album))
}

fn read_metadata(
    conn: &Connection,
    album_id: i64,
    path: &Path,
    include_user_edits: bool,
) -> Result<AlbumMetadata> {
    let mut fields = conn.prepare(
        "SELECT name, value, source FROM album_field WHERE album_id = ?1
             UNION ALL SELECT name, value, 'user_edit' FROM user_edit WHERE album_path = ?2 AND ?3",
    )?;
    let entries = fields.query_map(params![album_id, p(path), include_user_edits], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut metadata = AlbumMetadata::default();
    for entry in entries {
        let (name, value, source) = entry?;
        if let Some(source) = FieldSource::parse(&source) {
            metadata.apply(&name, &value, source);
        }
    }
    Ok(metadata)
}

fn read_track(row: &Row<'_>) -> Result<Track> {
    let label: String = row.get(5)?;
    let format = [
        AudioFormat::Flac,
        AudioFormat::Ape,
        AudioFormat::WavPack,
        AudioFormat::Wav,
        AudioFormat::Aiff,
        AudioFormat::Dsf,
        AudioFormat::Dff,
        AudioFormat::Mp3,
        AudioFormat::M4a,
        AudioFormat::Aac,
        AudioFormat::Ogg,
        AudioFormat::Opus,
    ]
    .into_iter()
    .find(|format| format.label() == label)
    .ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            5,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Unknown audio format: {label}"),
            )),
        )
    })?;
    let path: String = row.get(6)?;
    let source = match row.get::<_, Option<String>>(7)? {
        Some(cue) => TrackSource::Virtual {
            image: path.into(),
            cue: cue.into(),
            start: row.get(8)?,
            end: row.get(9)?,
        },
        None => TrackSource::File { path: path.into() },
    };
    Ok(Track {
        number: row.get(0)?,
        title: read_field(row, 1, 2)?,
        artist: read_field(row, 3, 4)?,
        format,
        source,
        properties: AudioProperties {
            sample_rate: row.get(10)?,
            bit_depth: row.get(11)?,
            channels: row.get(12)?,
            duration_ms: row
                .get::<_, Option<i64>>(13)?
                .map(|duration| duration as u64),
        },
    })
}

fn read_field(row: &Row<'_>, value: usize, source: usize) -> Result<Option<Field<String>>> {
    let value: Option<String> = row.get(value)?;
    let source: Option<String> = row.get(source)?;
    Ok(value
        .zip(source.and_then(|source| FieldSource::parse(&source)))
        .map(|(value, source)| Field::new(value, source)))
}

/// The scanner and structure replay share one normalized Album writer.
pub(super) fn store_album(conn: &Connection, root_id: i64, album: &Album) -> Result<()> {
    conn.execute("DELETE FROM album WHERE path=?1", [p(&album.path)])?;
    let (cover_path, embedded, picture_index) = album
        .cover
        .as_ref()
        .map(super::covers::parts)
        .map(|(path, embedded, index)| (Some(path), embedded, index))
        .unwrap_or((None, false, None));
    let source_directories = serde_json::to_string(&album.source_directories)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    conn.execute("INSERT INTO album(root_id,path,folder_name,collection_path,cover_path,cover_embedded,signature,cover_picture_index,source_directories)
        VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![root_id, p(&album.path), album.folder_name, album.collection.as_deref().map(p), cover_path, embedded, album.signature, picture_index, source_directories])?;
    let album_id = conn.last_insert_rowid();
    let candidates = serde_json::to_string(&crate::metadata_candidates::for_album(album))
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    conn.execute(
        "UPDATE album SET candidates_json=?1 WHERE id=?2",
        params![candidates, album_id],
    )?;
    for (name, value, source) in album.metadata.entries() {
        conn.execute(
            "INSERT INTO album_field(album_id,name,source,value) VALUES (?1,?2,?3,?4)",
            params![album_id, name, source.as_str(), value],
        )?;
    }
    for disc in &album.discs {
        conn.execute(
            "INSERT INTO disc(album_id,number,path) VALUES (?1,?2,?3)",
            params![album_id, disc.number, p(&disc.path)],
        )?;
        let disc_id = conn.last_insert_rowid();
        for track in &disc.tracks {
            let (cue, start, end) = match &track.source {
                TrackSource::File { .. } => (None, None, None),
                TrackSource::Virtual {
                    cue, start, end, ..
                } => (Some(p(cue)), Some(*start), *end),
            };
            conn.execute("INSERT INTO track(disc_id,number,title,title_source,artist,artist_source,format,path,cue_path,start_frames,end_frames,sample_rate,bit_depth,channels,duration_ms)
                VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)", params![disc_id,track.number,
                track.title.as_ref().map(|field| &field.value), track.title.as_ref().map(|field| field.source.as_str()),
                track.artist.as_ref().map(|field| &field.value), track.artist.as_ref().map(|field| field.source.as_str()),
                track.format.label(),p(track.source.audio_path()),cue,start,end,track.properties.sample_rate,track.properties.bit_depth,track.properties.channels,track.properties.duration_ms.map(|duration| duration as i64)])?;
        }
    }
    Ok(())
}

pub(super) fn read_source_directories(json: &str) -> Result<Vec<PathBuf>> {
    serde_json::from_str(json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}
