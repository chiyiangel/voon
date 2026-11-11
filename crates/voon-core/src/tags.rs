//! Embedded metadata tags and stream properties.
//!
//! Coverage (via `lofty`): FLAC, APE (APEv2), WavPack, WAV, AIFF, MP3, M4A,
//! AAC, Ogg Vorbis, Opus. DSF: stream properties come from the file header
//! and tags from the ID3v2 chunk the header points at (via the `id3` crate).
//! DFF: nothing is read in phase 1; folder name and CUE still apply.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
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
    #[error("没有内嵌封面")]
    MissingPicture,
    #[error("tags for {0} are not supported yet")]
    Unsupported(&'static str),
    #[error("reading tags: {0}")]
    Lofty(#[from] lofty::error::FileParseError),
    #[error("reading file: {0}")]
    Io(#[from] std::io::Error),
    #[error("reading ID3v2: {0}")]
    Id3(#[from] id3::Error),
}

pub fn read(path: &Path, format: AudioFormat) -> Result<TagInfo, TagError> {
    match format {
        AudioFormat::Dsf => read_dsf_header(path),
        AudioFormat::Dff => Err(TagError::Unsupported("DFF")),
        _ => read_lofty(path),
    }
}

/// Read the automatically selected Cover without changing its image/audio file.
/// The caller schedules this potentially slow NAS access away from the UI thread.
pub fn read_cover(cover: &crate::Cover) -> Result<Vec<u8>, TagError> {
    match &cover.source {
        crate::CoverSource::File(path) => Ok(std::fs::read(path)?),
        crate::CoverSource::Embedded(path) => {
            let pictures = embedded_pictures(path)?;
            let index = pictures.iter().position(|(front, _)| *front).unwrap_or(0);
            pictures
                .into_iter()
                .nth(index)
                .map(|(_, data)| data)
                .ok_or(TagError::MissingPicture)
        }
        crate::CoverSource::EmbeddedPicture { path, index } => embedded_pictures(path)?
            .into_iter()
            .nth(*index as usize)
            .map(|(_, data)| data)
            .ok_or(TagError::MissingPicture),
    }
}

pub fn embedded_cover_count(path: &Path) -> Result<usize, TagError> {
    Ok(embedded_pictures(path)?.len())
}

fn embedded_pictures(path: &Path) -> Result<Vec<(bool, Vec<u8>)>, TagError> {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("dsf"))
    {
        let mut file = File::open(path)?;
        let mut header = [0; 28];
        file.read_exact(&mut header)?;
        if &header[..4] != b"DSD " {
            return Err(TagError::MissingPicture);
        }
        let offset = u64::from_le_bytes(header[20..28].try_into().unwrap());
        if offset == 0 {
            return Ok(vec![]);
        }
        file.seek(SeekFrom::Start(offset))?;
        let tag = id3::Tag::read_from2(file)?;
        Ok(tag
            .pictures()
            .map(|picture| {
                (
                    picture.picture_type == id3::frame::PictureType::CoverFront,
                    picture.data.clone(),
                )
            })
            .collect())
    } else {
        let file = lofty::probe::Probe::open(path)?
            .options(lofty::config::ParseOptions::new().read_properties(false))
            .read()?;
        Ok(file
            .tags()
            .iter()
            .flat_map(|tag| tag.pictures())
            .map(|picture| {
                (
                    picture.pic_type() == lofty::picture::PictureType::CoverFront,
                    picture.data().to_vec(),
                )
            })
            .collect())
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

/// DSF layout: "DSD " chunk (28 bytes, metadata offset at +20), then the
/// "fmt " chunk whose body holds channel count at +24 and sampling frequency
/// at +28 (little-endian). A non-zero metadata offset points at an ID3v2 tag.
fn read_dsf_header(path: &Path) -> Result<TagInfo, TagError> {
    let mut file = File::open(path)?;
    let mut buf = [0u8; 28 + 52];
    file.read_exact(&mut buf)?;
    let mut info = TagInfo {
        properties: parse_dsf_header(&buf).unwrap_or_default(),
        ..Default::default()
    };
    let metadata = u64::from_le_bytes(buf[20..28].try_into().unwrap());
    if &buf[0..4] == b"DSD " && metadata > 0 {
        file.seek(SeekFrom::Start(metadata))?;
        match id3::Tag::read_from2(&mut file) {
            Ok(tag) => apply_id3(&tag, &mut info),
            // A dangling pointer or an empty chunk: properties only.
            Err(e) if matches!(e.kind, id3::ErrorKind::NoTag) => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(info)
}

fn apply_id3(tag: &id3::Tag, info: &mut TagInfo) {
    use id3::TagLike;
    let text = |id: &str| clean(tag.get(id).and_then(|f| f.content().text()));
    info.title = clean(tag.title());
    info.artist = clean(tag.artist());
    info.album = clean(tag.album());
    info.album_artist = clean(tag.album_artist());
    info.genre = clean(tag.genre_parsed());
    info.composer = text("TCOM");
    info.conductor = text("TPE3");
    info.track = tag.track();
    info.disc = tag.disc();
    info.year = tag
        .date_recorded()
        .map(|d| d.year)
        .or_else(|| tag.year())
        .and_then(|y| u16::try_from(y).ok());
    info.has_picture = tag.pictures().next().is_some();
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
    fn cover_bytes_are_read_from_a_directory_image_or_embedded_front_picture() {
        use id3::TagLike;
        let directory = tempfile::tempdir().unwrap();
        let image = directory.path().join("cover.jpg");
        std::fs::write(&image, [1, 2, 3, 4]).unwrap();
        assert_eq!(
            read_cover(&crate::Cover {
                source: crate::CoverSource::File(image)
            })
            .unwrap(),
            [1, 2, 3, 4]
        );

        let mut tag = id3::Tag::new();
        tag.add_frame(id3::frame::Picture {
            mime_type: "image/jpeg".into(),
            picture_type: id3::frame::PictureType::Other,
            description: "背面".into(),
            data: vec![8, 9],
        });
        tag.add_frame(id3::frame::Picture {
            mime_type: "image/jpeg".into(),
            picture_type: id3::frame::PictureType::CoverFront,
            description: "正面".into(),
            data: vec![5, 6, 7],
        });
        let audio = directory.path().join("01.dsf");
        std::fs::write(&audio, synth_dsf(Some(&tag))).unwrap();
        assert_eq!(
            read_cover(&crate::Cover {
                source: crate::CoverSource::Embedded(audio)
            })
            .unwrap(),
            [5, 6, 7]
        );
    }

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

    /// A minimal DSF: header chunks, a few bytes of "audio", then ID3v2.
    fn synth_dsf(tag: Option<&id3::Tag>) -> Vec<u8> {
        let mut b = vec![0u8; 80];
        b[0..4].copy_from_slice(b"DSD ");
        b[4..12].copy_from_slice(&28u64.to_le_bytes());
        b[28..32].copy_from_slice(b"fmt ");
        b[32..40].copy_from_slice(&52u64.to_le_bytes());
        b[52..56].copy_from_slice(&2u32.to_le_bytes());
        b[56..60].copy_from_slice(&5_644_800u32.to_le_bytes());
        b[60..64].copy_from_slice(&1u32.to_le_bytes());
        b[64..72].copy_from_slice(&(5_644_800u64 * 2).to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&20u64.to_le_bytes());
        b.extend_from_slice(&[0x69; 8]);
        if let Some(tag) = tag {
            let offset = b.len() as u64;
            b[20..28].copy_from_slice(&offset.to_le_bytes());
            tag.write_to(&mut b, id3::Version::Id3v24).unwrap();
        }
        let total = b.len() as u64;
        b[12..20].copy_from_slice(&total.to_le_bytes());
        b
    }

    #[test]
    fn dsf_id3_tag() {
        use id3::TagLike;
        let mut tag = id3::Tag::new();
        tag.set_title("So What");
        tag.set_artist("Miles Davis");
        tag.set_album("Kind of Blue");
        tag.set_album_artist("Miles Davis Sextet");
        tag.set_genre("Jazz");
        tag.set_track(1);
        tag.set_disc(1);
        tag.set_date_recorded("1959-03-02".parse().unwrap());
        tag.set_text("TCOM", "Miles Davis");
        tag.add_frame(id3::frame::Picture {
            mime_type: "image/jpeg".into(),
            picture_type: id3::frame::PictureType::CoverFront,
            description: String::new(),
            data: vec![0xFF, 0xD8, 0xFF],
        });
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("01.dsf");
        std::fs::write(&path, synth_dsf(Some(&tag))).unwrap();

        let info = read(&path, AudioFormat::Dsf).unwrap();
        assert_eq!(info.title.as_deref(), Some("So What"));
        assert_eq!(info.artist.as_deref(), Some("Miles Davis"));
        assert_eq!(info.album.as_deref(), Some("Kind of Blue"));
        assert_eq!(info.album_artist.as_deref(), Some("Miles Davis Sextet"));
        assert_eq!(info.genre.as_deref(), Some("Jazz"));
        assert_eq!(info.composer.as_deref(), Some("Miles Davis"));
        assert_eq!(
            (info.track, info.disc, info.year),
            (Some(1), Some(1), Some(1959))
        );
        assert!(info.has_picture);
        assert_eq!(info.properties.sample_rate, Some(5_644_800));
        assert_eq!(info.properties.duration_ms, Some(2000));
    }

    #[test]
    fn dsf_without_tag() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("02.dsf");
        std::fs::write(&path, synth_dsf(None)).unwrap();
        let info = read(&path, AudioFormat::Dsf).unwrap();
        assert_eq!(info.title, None);
        assert_eq!(info.properties.channels, Some(2));
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
