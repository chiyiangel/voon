//! CUE sheet decoding and parsing.
//!
//! CUE files in the wild are UTF-8 (with or without BOM), UTF-16 with BOM, or
//! a legacy code page (GBK for Chinese releases, Shift-JIS for Japanese ones).
//! Detection: BOM first, then strict UTF-8, then `chardetng` restricted to the
//! CJK legacy encodings we expect. A short or ambiguous file can still be
//! guessed wrong; the decoded text is what the rest of Voon sees.

use encoding_rs::{Encoding, GBK, SHIFT_JIS, UTF_8, UTF_16BE, UTF_16LE};

use crate::model::CueFrames;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CueSheet {
    pub title: Option<String>,
    pub performer: Option<String>,
    pub songwriter: Option<String>,
    pub genre: Option<String>,
    pub date: Option<String>,
    pub catalog: Option<String>,
    pub composer: Option<String>,
    pub files: Vec<CueFile>,
    /// Name of the encoding the text was decoded with.
    pub encoding: &'static str,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CueFile {
    /// The file name exactly as written in the sheet.
    pub name: String,
    pub tracks: Vec<CueTrack>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CueTrack {
    pub number: u32,
    pub title: Option<String>,
    pub performer: Option<String>,
    pub songwriter: Option<String>,
    /// `INDEX 01`, in CD frames from the start of the file.
    pub start: Option<CueFrames>,
}

impl CueSheet {
    pub fn track_count(&self) -> usize {
        self.files.iter().map(|f| f.tracks.len()).sum()
    }
}

/// Decode raw CUE bytes to text, returning the encoding used.
pub fn decode(bytes: &[u8]) -> (String, &'static Encoding) {
    if let Some((enc, bom_len)) = Encoding::for_bom(bytes) {
        let (text, _) = enc.decode_without_bom_handling(&bytes[bom_len..]);
        return (text.into_owned(), enc);
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return (s.to_string(), UTF_8);
    }
    // UTF-16 without BOM: lots of NULs in alternate positions.
    let nul_even = bytes.iter().step_by(2).filter(|b| **b == 0).count();
    let nul_odd = bytes.iter().skip(1).step_by(2).filter(|b| **b == 0).count();
    let half = bytes.len() / 2;
    if half > 0 && nul_odd * 3 > half * 2 {
        return (
            UTF_16LE.decode_without_bom_handling(bytes).0.into_owned(),
            UTF_16LE,
        );
    }
    if half > 0 && nul_even * 3 > half * 2 {
        return (
            UTF_16BE.decode_without_bom_handling(bytes).0.into_owned(),
            UTF_16BE,
        );
    }
    let enc = guess_legacy(bytes);
    (enc.decode_without_bom_handling(bytes).0.into_owned(), enc)
}

fn guess_legacy(bytes: &[u8]) -> &'static Encoding {
    let mut det = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Deny);
    det.feed(bytes, true);
    let guess = det.guess(None, chardetng::Utf8Detection::Deny);
    if guess == GBK
        || guess == SHIFT_JIS
        || guess == encoding_rs::BIG5
        || guess == encoding_rs::EUC_KR
    {
        return guess;
    }
    // Neither Chinese nor Japanese won outright: pick the one that decodes
    // without replacement characters, preferring GBK (the larger share of
    // the library).
    for enc in [GBK, SHIFT_JIS] {
        let (_, had_errors) = enc.decode_without_bom_handling(bytes);
        if !had_errors {
            return enc;
        }
    }
    guess
}

/// Decode and parse a CUE sheet.
pub fn parse_bytes(bytes: &[u8]) -> CueSheet {
    let (text, enc) = decode(bytes);
    let mut sheet = parse_str(&text);
    sheet.encoding = enc.name();
    sheet
}

/// Parse already-decoded CUE text. Unknown commands are skipped.
pub fn parse_str(text: &str) -> CueSheet {
    let mut sheet = CueSheet {
        encoding: "UTF-8",
        ..Default::default()
    };
    for raw in text.lines() {
        let line = raw.trim();
        let (cmd, rest) = split_word(line);
        let cmd = cmd.to_ascii_uppercase();
        let in_track = sheet.files.last().is_some_and(|f| !f.tracks.is_empty());
        match cmd.as_str() {
            "FILE" => {
                let name = file_name(rest);
                sheet.files.push(CueFile {
                    name,
                    tracks: Vec::new(),
                });
            }
            "TRACK" => {
                let (num, _kind) = split_word(rest);
                if sheet.files.is_empty() {
                    // TRACK before FILE: malformed, keep the track anyway.
                    sheet.files.push(CueFile::default());
                }
                sheet.files.last_mut().unwrap().tracks.push(CueTrack {
                    number: num.parse().unwrap_or(0),
                    ..Default::default()
                });
            }
            "INDEX" => {
                let (idx, time) = split_word(rest);
                if idx.parse::<u32>().ok() == Some(1)
                    && let Some(t) = current_track(&mut sheet)
                {
                    t.start = parse_time(time.trim());
                }
            }
            "TITLE" | "PERFORMER" | "SONGWRITER" => {
                let v = unquote(rest);
                if in_track {
                    let t = current_track(&mut sheet).unwrap();
                    match cmd.as_str() {
                        "TITLE" => t.title = Some(v),
                        "PERFORMER" => t.performer = Some(v),
                        _ => t.songwriter = Some(v),
                    }
                } else {
                    match cmd.as_str() {
                        "TITLE" => sheet.title = Some(v),
                        "PERFORMER" => sheet.performer = Some(v),
                        _ => sheet.songwriter = Some(v),
                    }
                }
            }
            "CATALOG" => sheet.catalog = Some(unquote(rest)),
            "REM" if !in_track => {
                let (key, value) = split_word(rest);
                let value = unquote(value);
                match key.to_ascii_uppercase().as_str() {
                    "GENRE" => sheet.genre = Some(value),
                    "DATE" => sheet.date = Some(value),
                    "COMPOSER" => sheet.composer = Some(value),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    sheet
}

fn current_track(sheet: &mut CueSheet) -> Option<&mut CueTrack> {
    sheet.files.last_mut()?.tracks.last_mut()
}

fn split_word(s: &str) -> (&str, &str) {
    let s = s.trim_start();
    match s.find(char::is_whitespace) {
        Some(i) => (&s[..i], s[i..].trim_start()),
        None => (s, ""),
    }
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix('"')
        && let Some(end) = inner.rfind('"')
    {
        return inner[..end].to_string();
    }
    s.to_string()
}

/// `FILE "name with spaces.ape" WAVE` or `FILE name.flac WAVE`.
fn file_name(rest: &str) -> String {
    let rest = rest.trim();
    if let Some(inner) = rest.strip_prefix('"') {
        return match inner.rfind('"') {
            Some(end) => inner[..end].to_string(),
            None => inner.to_string(),
        };
    }
    // Unquoted: drop the trailing file type word when there is one.
    match rest.rsplit_once(char::is_whitespace) {
        Some((name, _kind)) => name.trim().to_string(),
        None => rest.to_string(),
    }
}

/// `mm:ss:ff` to frames.
pub fn parse_time(s: &str) -> Option<CueFrames> {
    let mut it = s.split(':');
    let m: u32 = it.next()?.trim().parse().ok()?;
    let sec: u32 = it.next()?.trim().parse().ok()?;
    let f: u32 = it.next()?.trim().parse().ok()?;
    Some((m * 60 + sec) * 75 + f)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHEET: &str = "REM GENRE Classical\r\nREM DATE 1988\r\nREM COMPOSER \"Franz Schubert\"\r\nPERFORMER \"Alfred Brendel\"\r\nTITLE \"Impromptus\"\r\nFILE \"CDImage.wav\" WAVE\r\n  TRACK 01 AUDIO\r\n    TITLE \"Impromptu D.899 No.1\"\r\n    INDEX 00 00:00:00\r\n    INDEX 01 00:00:32\r\n  TRACK 02 AUDIO\r\n    TITLE \"Impromptu D.899 No.2\"\r\n    PERFORMER \"Brendel\"\r\n    INDEX 01 10:02:15\r\n";

    #[test]
    fn parses_album_and_tracks() {
        let s = parse_str(SHEET);
        assert_eq!(s.title.as_deref(), Some("Impromptus"));
        assert_eq!(s.performer.as_deref(), Some("Alfred Brendel"));
        assert_eq!(s.genre.as_deref(), Some("Classical"));
        assert_eq!(s.date.as_deref(), Some("1988"));
        assert_eq!(s.composer.as_deref(), Some("Franz Schubert"));
        assert_eq!(s.files.len(), 1);
        assert_eq!(s.files[0].name, "CDImage.wav");
        let t = &s.files[0].tracks;
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].start, Some(32));
        assert_eq!(t[1].start, Some((10 * 60 + 2) * 75 + 15));
        assert_eq!(t[1].performer.as_deref(), Some("Brendel"));
        assert_eq!(t[0].performer, None);
    }

    #[test]
    fn utf8_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("TITLE \"夜想曲\"\nFILE \"a.flac\" WAVE\n".as_bytes());
        let s = parse_bytes(&bytes);
        assert_eq!(s.title.as_deref(), Some("夜想曲"));
        assert_eq!(s.encoding, "UTF-8");
    }

    #[test]
    fn gbk_sheet() {
        let text = "PERFORMER \"邓丽君\"\nTITLE \"淡淡幽情\"\nFILE \"邓丽君 - 淡淡幽情.ape\" WAVE\n  TRACK 01 AUDIO\n    TITLE \"独上西楼\"\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    TITLE \"但愿人长久\"\n    INDEX 01 03:10:00\n";
        let (bytes, _, _) = GBK.encode(text);
        assert!(std::str::from_utf8(&bytes).is_err());
        let s = parse_bytes(&bytes);
        assert_eq!(s.encoding, "GBK");
        assert_eq!(s.performer.as_deref(), Some("邓丽君"));
        assert_eq!(s.files[0].name, "邓丽君 - 淡淡幽情.ape");
        assert_eq!(s.files[0].tracks[1].title.as_deref(), Some("但愿人长久"));
    }

    #[test]
    fn shift_jis_sheet() {
        let text = "PERFORMER \"中島みゆき\"\nTITLE \"寒水魚\"\nFILE \"CDImage.flac\" WAVE\n  TRACK 01 AUDIO\n    TITLE \"悪女\"\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    TITLE \"傾斜\"\n    INDEX 01 04:00:00\n";
        let (bytes, _, _) = SHIFT_JIS.encode(text);
        let s = parse_bytes(&bytes);
        assert_eq!(s.encoding, "Shift_JIS");
        assert_eq!(s.performer.as_deref(), Some("中島みゆき"));
        assert_eq!(s.files[0].tracks[0].title.as_deref(), Some("悪女"));
    }

    #[test]
    fn utf16le_bom() {
        let mut bytes = vec![0xFF, 0xFE];
        for u in "TITLE \"Album\"\n".encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(parse_bytes(&bytes).title.as_deref(), Some("Album"));
    }

    #[test]
    fn unquoted_file_and_multi_file() {
        let s = parse_str(
            "FILE 01.flac WAVE\nTRACK 01 AUDIO\nINDEX 01 00:00:00\nFILE 02.flac WAVE\nTRACK 02 AUDIO\nINDEX 01 00:00:00\n",
        );
        assert_eq!(s.files.len(), 2);
        assert_eq!(s.files[0].name, "01.flac");
        assert_eq!(s.track_count(), 2);
    }
}
