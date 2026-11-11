//! Automatic facets: properties of an album Voon derives by itself, used for
//! filtering. They are recomputed from the album, never stored as truth.

use std::collections::BTreeSet;

use crate::folder_name;
use crate::model::{Album, AudioFormat};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Encoding {
    Dsd,
    Pcm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Language {
    Chinese,
    Japanese,
    Korean,
    Latin,
    Unknown,
}

impl Language {
    pub fn label(self) -> &'static str {
        match self {
            Language::Chinese => "中文",
            Language::Japanese => "日文",
            Language::Korean => "韩文",
            Language::Latin => "西文",
            Language::Unknown => "未知",
        }
    }
}

/// Resolution class shown as a facet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Resolution {
    /// Lossy formats.
    Lossy,
    /// 16 bit / 44.1–48 kHz.
    Cd,
    /// Above CD in bit depth or sample rate.
    HiRes,
    /// DSD64 and up.
    Dsd(u16),
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Facets {
    pub formats: BTreeSet<AudioFormat>,
    pub encoding: Encoding,
    pub resolution: Resolution,
    pub max_sample_rate: Option<u32>,
    pub max_bit_depth: Option<u8>,
    pub decade: Option<u16>,
    pub language: Language,
}

/// Guess the language from the writing system of the text.
/// Kana means Japanese; Hangul means Korean; other Han means Chinese.
pub fn language_of(text: &str) -> Language {
    let (mut han, mut kana, mut hangul, mut latin) = (0, 0, 0, 0);
    for c in text.chars() {
        match c as u32 {
            0x3040..=0x30FF | 0x31F0..=0x31FF => kana += 1,
            0xAC00..=0xD7AF | 0x1100..=0x11FF => hangul += 1,
            0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0xF900..=0xFAFF => han += 1,
            _ if c.is_ascii_alphabetic() || ('\u{C0}'..='\u{24F}').contains(&c) => latin += 1,
            _ => {}
        }
    }
    if kana > 0 {
        Language::Japanese
    } else if hangul > 0 {
        Language::Korean
    } else if han > 0 {
        Language::Chinese
    } else if latin > 0 {
        Language::Latin
    } else {
        Language::Unknown
    }
}

pub fn facets(album: &Album) -> Facets {
    let formats: BTreeSet<AudioFormat> = album.tracks().map(|t| t.format).collect();
    let hints = folder_name::parse(&album.folder_name);
    let max_sample_rate = album
        .tracks()
        .filter_map(|t| t.properties.sample_rate)
        .max()
        .or(hints.sample_rate);
    let max_bit_depth = album
        .tracks()
        .filter_map(|t| t.properties.bit_depth)
        .max()
        .or(hints.bit_depth);
    let dsd = formats.iter().any(|f| f.is_dsd()) || hints.dsd_rate.is_some();
    let encoding = if dsd { Encoding::Dsd } else { Encoding::Pcm };
    let resolution = if dsd {
        let rate = hints
            .dsd_rate
            .or_else(|| max_sample_rate.map(|r| ((r as f64 / 44_100.0).round() as u16).max(64)));
        Resolution::Dsd(rate.unwrap_or(64))
    } else if !formats.is_empty() && formats.iter().all(|f| !f.is_lossless()) {
        Resolution::Lossy
    } else {
        match (max_bit_depth, max_sample_rate) {
            (None, None) => Resolution::Unknown,
            (b, r) if b.unwrap_or(16) > 16 || r.unwrap_or(44_100) > 48_000 => Resolution::HiRes,
            _ => Resolution::Cd,
        }
    };
    let decade = album.metadata.year.as_ref().map(|y| y.value / 10 * 10);
    let mut text = album.display_title().to_string();
    if let Some(a) = &album.metadata.album_artist {
        text.push(' ');
        text.push_str(&a.value);
    }
    Facets {
        formats,
        encoding,
        resolution,
        max_sample_rate,
        max_bit_depth,
        decade,
        language: language_of(&text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages() {
        assert_eq!(language_of("淡淡幽情"), Language::Chinese);
        assert_eq!(language_of("中島みゆき 寒水魚"), Language::Japanese);
        assert_eq!(language_of("Kind of Blue"), Language::Latin);
        assert_eq!(language_of("0015830558"), Language::Unknown);
    }
}
