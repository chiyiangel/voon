//! Embedded metadata tags and stream properties.
//!
//! Coverage (via `lofty`): FLAC, APE (APEv2), WavPack, WAV, AIFF, MP3, M4A,
//! AAC, Ogg Vorbis, Opus. DSF: stream properties are read from the file
//! header here; its ID3v2 tag is not read yet. DFF: nothing is read yet.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use lofty::prelude::*;

use crate::model::{AudioFormat, AudioProperties};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TagInfo {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub composer: Option<String>,
    pub conductor: Option<String>,
    pub genre: Option<String>,
    pub year: Option<u16>,
    pub track: Option<u32>,
    pub disc: Option<u32>,
    pub catalog_number: Option<String>,
    pub has_picture: bool,
    pub properties: AudioProperties,
}

#[derive(Debug, thiserror::Error)]
pub enum TagError {
    #[error("tags for {0} are not supported yet")]
    Unsupported(&'static str),
    #[error("reading tags: {0}")]
    Lofty(#[from] lofty::error::FileParseError),
    #[error("reading file: {0}")]
    Io(#[from] std::io::Error),
}

pub fn read(path: &Path, format: AudioFormat) -> Result<TagInfo, TagError> {
    match format {
        AudioFormat::Dsf => read_dsf_header(path),
        AudioFormat::Dff => Err(TagError::Unsupported("DFF")),
        _ => read_lofty(path),
    }
}

fn clean(s: Option<impl AsRef<str>>) -> Option<String> {
    let s = s?.as_ref().trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn read_lofty(path: &Path) -> Result<TagInfo, TagError> {
    let file = lofty::read_from_path(path)?;
    let p = file.properties();
    let mut info = TagInfo {
        properties: AudioProperties {
            sample_rate: p.sample_rate(),
            bit_depth: p.bit_depth(),
            channels: p.channels(),
            duration_ms: Some(p.duration().as_millis() as u64),
        },
        ..Default::default()
    };
    if let Some(tag) = file.primary_tag().or_else(|| file.first_tag()) {
        info.title = clean(tag.title());
        info.artist = clean(tag.artist());
        info.album = clean(tag.album());
        info.genre = clean(tag.genre());
        info.album_artist = clean(tag.get_string(ItemKey::AlbumArtist));
        info.composer = clean(tag.get_string(ItemKey::Composer));
        info.conductor = clean(tag.get_string(ItemKey::Conductor));
        info.catalog_number = clean(tag.get_string(ItemKey::CatalogNumber));
        info.year = tag.date().map(|d| d.year);
        info.track = tag.track();
        info.disc = tag.disk();
        info.has_picture = !tag.pictures().is_empty();
    }
    Ok(info)
}

/// DSF layout: "DSD " chunk (28 bytes), then "fmt " chunk whose body holds
/// channel count at +24 and sampling frequency at +28 (little-endian).
fn read_dsf_header(path: &Path) -> Result<TagInfo, TagError> {
    let mut buf = [0u8; 28 + 52];
    File::open(path)?.read_exact(&mut buf)?;
    Ok(TagInfo {
        properties: parse_dsf_header(&buf).unwrap_or_default(),
        ..Default::default()
    })
}

pub(crate) fn parse_dsf_header(buf: &[u8]) -> Option<AudioProperties> {
    if buf.len() < 80 || &buf[0..4] != b"DSD " || &buf[28..32] != b"fmt " {
        return None;
    }
    let u32_at = |o: usize| u32::from_le_bytes(buf[o..o + 4].try_into().unwrap());
    let u64_at = |o: usize| u64::from_le_bytes(buf[o..o + 8].try_into().unwrap());
    let channels = u32_at(28 + 24);
    let rate = u32_at(28 + 28);
    let bits = u32_at(28 + 32);
    let samples = u64_at(28 + 36);
    Some(AudioProperties {
        sample_rate: Some(rate),
        bit_depth: u8::try_from(bits).ok(),
        channels: u8::try_from(channels).ok(),
        duration_ms: (rate > 0).then(|| samples * 1000 / rate as u64),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dsf_header() {
        let mut b = vec![0u8; 80];
        b[0..4].copy_from_slice(b"DSD ");
        b[28..32].copy_from_slice(b"fmt ");
        b[52..56].copy_from_slice(&2u32.to_le_bytes());
        b[56..60].copy_from_slice(&2_822_400u32.to_le_bytes());
        b[60..64].copy_from_slice(&1u32.to_le_bytes());
        b[64..72].copy_from_slice(&(2_822_400u64 * 3).to_le_bytes());
        let p = parse_dsf_header(&b).unwrap();
        assert_eq!(p.sample_rate, Some(2_822_400));
        assert_eq!(p.channels, Some(2));
        assert_eq!(p.duration_ms, Some(3000));
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fake.flac");
        std::fs::write(&path, b"not audio").unwrap();
        assert!(read(&path, AudioFormat::Flac).is_err());
        let dff = dir.path().join("x.dff");
        std::fs::write(&dff, b"FRM8").unwrap();
        assert!(matches!(
            read(&dff, AudioFormat::Dff),
            Err(TagError::Unsupported("DFF"))
        ));
    }
}
