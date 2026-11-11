//! Hints parsed out of an album folder name, e.g.
//! `Alfred Brendel - Schubert Impromptus (1988) [24-96] [438317-2] [台湾首版]`.

use regex::Regex;
use std::sync::LazyLock;

use crate::model::AudioFormat;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FolderHints {
    /// What is left once the recognised groups are removed.
    pub title: Option<String>,
    /// The part before ` - ` when the name has exactly one such separator.
    pub artist: Option<String>,
    pub year: Option<u16>,
    pub format: Option<AudioFormat>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u8>,
    /// DSD multiple of 44.1 kHz: 64, 128, 256, 512.
    pub dsd_rate: Option<u16>,
    pub catalog_number: Option<String>,
    pub edition: Option<String>,
}

static GROUP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]*)\]|\(([^)]*)\)|【([^】]*)】|（([^）]*)）").unwrap());
static YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(19[0-9]{2}|20[0-9]{2})$").unwrap());
static LEADING_YEAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(19[0-9]{2}|20[0-9]{2})\s*[-–.]\s+").unwrap());
static DSD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bDSD\s*(64|128|256|512)\b").unwrap());
// 24-96, 24/96, 24bit96kHz, 24bit-96khz, 16-44.1, 24B-192K
static BITS_RATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|[^0-9.])(?P<spec>(?P<bits>16|24|32)\s*(?:(?:bits?|b)\s*[-/ _]?|[-/ _])\s*(?P<rate>44\.1|48|88\.2|96|176\.4|192|352\.8|384)\s*(?:k|khz)?)\b")
        .unwrap()
});
// 96kHz/24bit, 192k-24b
static RATE_BITS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(44\.1|48|88\.2|96|176\.4|192|352\.8|384)\s*(?:k|khz)\s*[-/ _]?\s*(16|24|32)\s*(?:bits?|b)\b")
        .unwrap()
});
static FORMAT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(FLAC|APE|WAV|DSF|DFF|WV|WavPack|MP3|AIFF|ALAC)(?:\b|[0-9_])").unwrap()
});
static CATALOG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:[A-Z]{1,6}[- ]?)?[0-9]{3,}(?:[-/][0-9A-Z]{1,4})?$|^[0-9]{6,}$").unwrap()
});
static EDITION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)版|限定|限量|remaster|edition|limited|SHM-?CD|XRCD|K2HD|UHQCD|Blu-?spec|LPCD|HDCD|SACD")
        .unwrap()
});

fn sample_rate(khz: &str) -> Option<u32> {
    let khz: f64 = khz.parse().ok()?;
    Some((khz * 1000.0).round() as u32)
}

/// Parse what a folder name says about its album. Never fails; unknown text
/// stays in the title.
pub fn parse(name: &str) -> FolderHints {
    let mut hints = FolderHints::default();
    let mut rest = name.to_string();

    // Every bracketed group is either recognised (and removed) or kept.
    let mut removed: Vec<String> = Vec::new();
    for caps in GROUP.captures_iter(name) {
        let whole = caps.get(0).unwrap().as_str();
        let inner = (1..=4)
            .find_map(|i| caps.get(i))
            .map(|m| m.as_str().trim())
            .unwrap_or("");
        if classify_group(inner, &mut hints) {
            removed.push(whole.to_string());
        }
    }
    for r in &removed {
        rest = rest.replacen(r.as_str(), " ", 1);
    }

    if hints.year.is_none()
        && let Some(m) = LEADING_YEAR.captures(rest.trim_start())
    {
        hints.year = m[1].parse().ok();
        let trimmed = rest.trim_start();
        rest = trimmed[m.get(0).unwrap().end()..].to_string();
    }

    // Spec text outside brackets, e.g. `Album FLAC 24-96`.
    let before = rest.clone();
    scan_specs(&before, &mut hints, &mut rest);

    let rest = collapse(&rest);
    let parts: Vec<&str> = rest.split(" - ").collect();
    if parts.len() == 2 && !parts[0].trim().is_empty() && !parts[1].trim().is_empty() {
        hints.artist = Some(parts[0].trim().to_string());
        hints.title = Some(parts[1].trim().to_string());
    } else if !rest.is_empty() {
        hints.title = Some(rest);
    }
    hints
}

fn classify_group(inner: &str, hints: &mut FolderHints) -> bool {
    if inner.is_empty() {
        return true;
    }
    if YEAR.is_match(inner) {
        hints.year.get_or_insert(inner.parse().unwrap());
        return true;
    }
    let mut spec = false;
    if let Some(c) = DSD.captures(inner) {
        hints.dsd_rate.get_or_insert(c[1].parse().unwrap());
        spec = true;
    }
    if let Some(c) = BITS_RATE.captures(inner) {
        hints.bit_depth.get_or_insert(c["bits"].parse().unwrap());
        if let Some(r) = sample_rate(&c["rate"]) {
            hints.sample_rate.get_or_insert(r);
        }
        spec = true;
    } else if let Some(c) = RATE_BITS.captures(inner) {
        if let Some(r) = sample_rate(&c[1]) {
            hints.sample_rate.get_or_insert(r);
        }
        hints.bit_depth.get_or_insert(c[2].parse().unwrap());
        spec = true;
    }
    if let Some(c) = FORMAT.captures(inner) {
        if hints.format.is_none() {
            hints.format = format_word(&c[1]);
        }
        spec = true;
    }
    if spec {
        return true;
    }
    if CATALOG.is_match(inner) {
        hints.catalog_number.get_or_insert(inner.to_string());
        return true;
    }
    if EDITION.is_match(inner) {
        hints.edition.get_or_insert(inner.to_string());
        return true;
    }
    false
}

fn scan_specs(text: &str, hints: &mut FolderHints, rest: &mut String) {
    if let Some(m) = DSD.captures(text) {
        hints.dsd_rate.get_or_insert(m[1].parse().unwrap());
        *rest = rest.replacen(m.get(0).unwrap().as_str(), " ", 1);
    }
    if let Some(m) = BITS_RATE.captures(text) {
        hints.bit_depth.get_or_insert(m["bits"].parse().unwrap());
        if let Some(r) = sample_rate(&m["rate"]) {
            hints.sample_rate.get_or_insert(r);
        }
        *rest = rest.replacen(m.name("spec").unwrap().as_str(), " ", 1);
    }
}

fn format_word(w: &str) -> Option<AudioFormat> {
    match w.to_ascii_lowercase().as_str() {
        "wavpack" => Some(AudioFormat::WavPack),
        other => AudioFormat::from_extension(other),
    }
}

fn collapse(s: &str) -> String {
    let joined = s.split_whitespace().collect::<Vec<_>>().join(" ");
    joined
        .trim_matches(|c: char| c == '-' || c == '_' || c == '.' || c.is_whitespace())
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_name() {
        let h = parse("Alfred Brendel - Schubert Impromptus (1988) [24-96] [438317-2] [台湾首版]");
        assert_eq!(h.artist.as_deref(), Some("Alfred Brendel"));
        assert_eq!(h.title.as_deref(), Some("Schubert Impromptus"));
        assert_eq!(h.year, Some(1988));
        assert_eq!(h.bit_depth, Some(24));
        assert_eq!(h.sample_rate, Some(96_000));
        assert_eq!(h.catalog_number.as_deref(), Some("438317-2"));
        assert_eq!(h.edition.as_deref(), Some("台湾首版"));
    }

    #[test]
    fn dsd_and_glued_spec() {
        let h = parse("Kind of Blue [DSD64]");
        assert_eq!(h.dsd_rate, Some(64));
        assert_eq!(h.title.as_deref(), Some("Kind of Blue"));

        let h = parse("Bach Goldberg [FLAC24bit96kHz]");
        assert_eq!(h.format, Some(AudioFormat::Flac));
        assert_eq!(h.bit_depth, Some(24));
        assert_eq!(h.sample_rate, Some(96_000));
        assert_eq!(h.title.as_deref(), Some("Bach Goldberg"));
    }

    #[test]
    fn leading_year_and_rate_first() {
        let h = parse("1999 - 某专辑 [96kHz/24bit]");
        assert_eq!(h.year, Some(1999));
        assert_eq!(h.sample_rate, Some(96_000));
        assert_eq!(h.bit_depth, Some(24));
        assert_eq!(h.title.as_deref(), Some("某专辑"));
    }

    #[test]
    fn catalog_with_letters_and_cd_44() {
        let h = parse("ショパン 夜想曲集 [UCCG-1234] [16-44.1]");
        assert_eq!(h.catalog_number.as_deref(), Some("UCCG-1234"));
        assert_eq!(h.sample_rate, Some(44_100));
        assert_eq!(h.title.as_deref(), Some("ショパン 夜想曲集"));
    }

    #[test]
    fn unknown_groups_stay_in_title() {
        let h = parse("Symphony No. 9 (Live)");
        assert_eq!(h.title.as_deref(), Some("Symphony No. 9 (Live)"));
        assert_eq!(h.year, None);
    }

    #[test]
    fn catalog_digits_are_not_a_spec() {
        let h = parse("Album [POCL-2448]");
        assert_eq!(h.catalog_number.as_deref(), Some("POCL-2448"));
        assert_eq!(h.sample_rate, None);
    }

    #[test]
    fn spec_outside_brackets() {
        let h = parse("Album FLAC 24-96");
        assert_eq!(h.sample_rate, Some(96_000));
        assert_eq!(h.title.as_deref(), Some("Album FLAC"));
    }

    #[test]
    fn plain_name() {
        let h = parse("0015830558");
        assert_eq!(h.title.as_deref(), Some("0015830558"));
    }
}
