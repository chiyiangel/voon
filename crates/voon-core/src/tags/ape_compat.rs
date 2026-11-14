//! Conservative recovery for legacy-encoded APEv1/v2 text. The source File is
//! read-only; a bounded replacement tail exists only in memory. Lofty still
//! validates the resulting file and reads stream properties and binary items.
use std::{
    collections::BTreeSet,
    error::Error,
    fs::{self, File},
    io::{self, BufReader, Read, Seek, SeekFrom},
    path::Path,
};

use encoding_rs::{BIG5, EUC_KR, GBK, SHIFT_JIS, WINDOWS_1251, WINDOWS_1252};
use lofty::{
    config::ParseOptions,
    file::{FileType, TaggedFile},
    probe::Probe,
};

use crate::{
    AudioFormat,
    cue::{self, CueSheet},
};

const MAX_TAG: usize = 16 * 1024 * 1024;
const MAX_ITEMS: u32 = 4096;
const MAX_CUE: u64 = 256 * 1024;
const MAX_CUES: usize = 32;

pub(super) fn is_utf8_error(error: &dyn Error) -> bool {
    let mut cause = error.source();
    while let Some(error) = cause {
        if error.is::<std::str::Utf8Error>() || error.is::<std::string::FromUtf8Error>() {
            return true;
        }
        cause = error.source();
    }
    false
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_at(file: &mut File, offset: u64, size: usize) -> io::Result<Vec<u8>> {
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = vec![0; size];
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

/// Unsupported layouts, missing evidence and ambiguous encodings leave the
/// original parser error intact. No lossy decoding. A verified v1 ripping-path
/// Title can be omitted from the in-memory view when a CUE supplies the tracks.
pub(super) fn read(path: &Path, options: ParseOptions) -> Option<TaggedFile> {
    let absolute = std::path::absolute(path).ok()?;
    let path = absolute.as_path();
    let mut file = File::open(path).ok()?;
    let before = file.metadata().ok()?;
    let (start, tail) = replacement_tail(&mut file, path).ok()?;
    let reader = ReadOnlyTail {
        file,
        start,
        tail,
        position: 0,
    };
    let parsed = Probe::with_file_type(BufReader::new(reader), FileType::Ape)
        .options(options)
        .read()
        .ok()?;
    let after = fs::metadata(path).ok()?;
    if before.len() != after.len() || before.modified().ok()? != after.modified().ok()? {
        return None;
    }
    Some(parsed)
}

fn replacement_tail(file: &mut File, path: &Path) -> io::Result<(u64, Vec<u8>)> {
    let length = file.metadata()?.len();
    let mut end = length;
    let mut suffix = Vec::new();
    if length >= 128 {
        let id3 = read_at(file, length - 128, 128)?;
        if id3.starts_with(b"TAG") {
            end -= 128;
            suffix = id3;
        }
    }
    if end < 32 {
        return Err(invalid("missing APE footer"));
    }
    let mut footer = read_at(file, end - 32, 32)?;
    let version = u32_at(&footer, 8);
    if !footer.starts_with(b"APETAGEX") || !matches!(version, 1000 | 2000) {
        return Err(invalid("only trailing APEv1/v2 tags are supported"));
    }
    let size = u32_at(&footer, 12) as usize;
    let count = u32_at(&footer, 16);
    let flags = u32_at(&footer, 20);
    if !(32..=MAX_TAG).contains(&size)
        || size as u64 > end
        || count > MAX_ITEMS
        || flags & (1 << 29) != 0
        || footer[24..].iter().any(|b| *b != 0)
        || (version == 1000 && flags != 0)
    {
        return Err(invalid("invalid APE footer"));
    }
    let item_start = end - size as u64;
    let mut start = item_start;
    let mut header = None;
    if flags & (1 << 31) != 0 {
        start = start
            .checked_sub(32)
            .ok_or_else(|| invalid("missing APE header"))?;
        let bytes = read_at(file, start, 32)?;
        if !bytes.starts_with(b"APETAGEX")
            || bytes[8..20] != footer[8..20]
            || u32_at(&bytes, 20) != (flags | (1 << 29))
            || bytes[24..] != footer[24..]
        {
            return Err(invalid("APE header/footer mismatch"));
        }
        header = Some(bytes);
    }
    let raw = read_at(file, item_start, size - 32)?;
    let sheets = matching_cues(path)?;
    let mut position = 0usize;
    let mut items = Vec::new();
    let mut repaired = false;
    let mut written_count = 0u32;
    let mut keys = BTreeSet::new();
    for _ in 0..count {
        if raw.len() - position < 8 {
            return Err(invalid("truncated APE item"));
        }
        let value_len = u32_at(&raw, position) as usize;
        let item_flags = u32_at(&raw, position + 4);
        if version == 1000 && item_flags != 0 {
            return Err(invalid("unsupported APEv1 item flags"));
        }
        position += 8;
        let key_end = raw[position..]
            .iter()
            .take(256)
            .position(|b| *b == 0)
            .map(|n| position + n)
            .ok_or_else(|| invalid("invalid APE key"))?;
        let key = &raw[position..key_end];
        if key.is_empty() || !key.iter().all(|b| (0x20..=0x7e).contains(b)) {
            return Err(invalid("invalid APE key encoding"));
        }
        let key = std::str::from_utf8(key).map_err(|_| invalid("invalid APE key"))?;
        if !keys.insert(key.to_ascii_lowercase()) {
            return Err(invalid("duplicate APE key"));
        }
        position = key_end + 1;
        let value_end = position
            .checked_add(value_len)
            .filter(|end| *end <= raw.len())
            .ok_or_else(|| invalid("APE value outside tag"))?;
        let value = &raw[position..value_end];
        position = value_end;
        let kind = (item_flags >> 1) & 3;
        if kind == 3 {
            return Err(invalid("reserved APE item type"));
        }
        let replacement;
        let value = if kind == 0 && std::str::from_utf8(value).is_err() {
            if version == 1000
                && key.eq_ignore_ascii_case("title")
                && is_cue_image_path(value, &sheets)
            {
                repaired = true;
                continue;
            }
            replacement = corroborated_text(key, value, &sheets)
                .ok_or_else(|| invalid("legacy text has no unambiguous CUE match"))?;
            repaired = true;
            replacement.as_bytes()
        } else {
            // Binary values (including cover bytes) and valid UTF-8 are unchanged.
            value
        };
        if items.len() + 8 + key.len() + 1 + value.len() > MAX_TAG - 32 {
            return Err(invalid("expanded tag exceeds limit"));
        }
        items.extend_from_slice(&(value.len() as u32).to_le_bytes());
        items.extend_from_slice(&item_flags.to_le_bytes());
        items.extend_from_slice(key.as_bytes());
        items.push(0);
        items.extend_from_slice(value);
        written_count += 1;
    }
    if position != raw.len() || !repaired {
        return Err(invalid("incomplete or unnecessary recovery"));
    }
    let size = ((items.len() + 32) as u32).to_le_bytes();
    footer[12..16].copy_from_slice(&size);
    footer[16..20].copy_from_slice(&written_count.to_le_bytes());
    let mut tail = Vec::new();
    if let Some(mut header) = header {
        header[12..16].copy_from_slice(&size);
        header[16..20].copy_from_slice(&written_count.to_le_bytes());
        tail.extend(header);
    }
    tail.extend(items);
    tail.extend(footer);
    tail.extend(suffix);
    Ok((start, tail))
}

// Old EAC rips sometimes store an absolute Windows output path in Title.
// The undecodable directory component is not music metadata. Ignore it only
// when the ASCII image basename agrees with every related single-image CUE.
fn is_cue_image_path(bytes: &[u8], sheets: &[CueSheet]) -> bool {
    let bytes = bytes.strip_suffix(&[0]).unwrap_or(bytes);
    if bytes.len() < 4
        || !bytes[0].is_ascii_alphabetic()
        || bytes[1] != b':'
        || !matches!(bytes[2], b'\\' | b'/')
        || bytes.iter().any(|byte| *byte < 0x20)
    {
        return false;
    }
    let basename = bytes
        .rsplit(|byte| matches!(byte, b'\\' | b'/'))
        .next()
        .unwrap_or_default();
    if !basename.is_ascii() || basename.is_empty() {
        return false;
    }
    let Ok(basename) = std::str::from_utf8(basename) else {
        return false;
    };
    let stem = Path::new(basename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    !stem.is_empty()
        && !sheets.is_empty()
        && sheets.iter().all(|sheet| {
            sheet.files.len() == 1
                && sheet.track_count() > 0
                && Path::new(sheet.files[0].name.rsplit(['\\', '/']).next().unwrap_or(""))
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|name| name.eq_ignore_ascii_case(stem))
        })
}

fn corroborated_text(key: &str, bytes: &[u8], sheets: &[CueSheet]) -> Option<String> {
    let references: BTreeSet<_> = sheets
        .iter()
        .filter_map(|sheet| match key.to_ascii_lowercase().as_str() {
            "album" => sheet.title.as_deref(),
            "artist" | "album artist" | "albumartist" => sheet.performer.as_deref(),
            "composer" => sheet.composer.as_deref().or(sheet.songwriter.as_deref()),
            "genre" => sheet.genre.as_deref(),
            _ => None,
        })
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    // Even mutually consistent decoder guesses cannot overrule conflicting CUEs.
    if references.len() != 1 {
        return None;
    }
    let reference = *references.first()?;
    let mut matches = BTreeSet::new();
    for encoding in [WINDOWS_1252, GBK, BIG5, SHIFT_JIS, EUC_KR, WINDOWS_1251] {
        if let Some(text) = encoding.decode_without_bom_handling_and_without_replacement(bytes)
            && text.trim_end_matches('\0').trim() == reference
        {
            matches.insert(text.into_owned());
        }
    }
    (matches.len() == 1).then(|| matches.into_iter().next().unwrap())
}

fn matching_cues(path: &Path) -> io::Result<Vec<CueSheet>> {
    let directory = path.parent().ok_or_else(|| invalid("missing parent"))?;
    let mut cues = Vec::new();
    let mut audio = Vec::new();
    for (index, entry) in fs::read_dir(directory)?.enumerate() {
        if index >= 4096 {
            return Err(invalid("directory exceeds compatibility limit"));
        }
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(['.', '@', '#']) {
            continue;
        }
        let ext = entry_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if ext.eq_ignore_ascii_case("cue") {
            cues.push(entry_path.clone());
        }
        if AudioFormat::from_extension(ext).is_some() {
            audio.push(entry_path);
        }
    }
    if cues.len() > MAX_CUES {
        return Err(invalid("too many CUE files"));
    }
    let mut sheets = Vec::new();
    for cue in cues {
        let mut bytes = Vec::new();
        File::open(&cue)?
            .take(MAX_CUE + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_CUE {
            return Err(invalid("CUE exceeds limit"));
        }
        let sheet = cue::parse_bytes_with_audio(&bytes, &audio);
        if sheet.files.iter().any(|file| {
            cue::resolve_audio(&cue, &file.name, &audio, sheet.files.len() == 1).as_deref()
                == Some(path)
        }) {
            sheets.push(sheet);
        }
    }
    Ok(sheets)
}

/// A virtual file made of an untouched source prefix and an in-memory tag tail.
/// No Write implementation, temporary audio copies or source write handles.
struct ReadOnlyTail {
    file: File,
    start: u64,
    tail: Vec<u8>,
    position: u64,
}

impl Read for ReadOnlyTail {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let length = self.start + self.tail.len() as u64;
        if self.position >= length || buffer.is_empty() {
            return Ok(0);
        }
        let count = if self.position < self.start {
            let count = (self.start - self.position).min(buffer.len() as u64) as usize;
            self.file.seek(SeekFrom::Start(self.position))?;
            self.file.read(&mut buffer[..count])?
        } else {
            let offset = (self.position - self.start) as usize;
            let count = buffer.len().min(self.tail.len() - offset);
            buffer[..count].copy_from_slice(&self.tail[offset..offset + count]);
            count
        };
        self.position += count as u64;
        Ok(count)
    }
}

impl Seek for ReadOnlyTail {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = match from {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::End(n) => i128::from(self.start) + self.tail.len() as i128 + i128::from(n),
            SeekFrom::Current(n) => i128::from(self.position) + i128::from(n),
        };
        self.position =
            u64::try_from(position).map_err(|_| invalid("seek outside file address range"))?;
        Ok(self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_tail_preserves_binary_values_and_supports_seek_across_the_boundary() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("image.ape");
        let mut raw = vec![42; 128];
        let binary = b"front.png\0\xff\x00\xfe\x80\xe9";
        let mut items = Vec::new();
        for (key, value, flags) in [
            ("Album", b"Path\xe9tique".as_slice(), 0u32),
            ("Cover Art (Front)", binary.as_slice(), 2),
        ] {
            items.extend((value.len() as u32).to_le_bytes());
            items.extend(flags.to_le_bytes());
            items.extend(key.as_bytes());
            items.push(0);
            items.extend(value);
        }
        raw.extend(&items);
        raw.extend(b"APETAGEX");
        raw.extend(2000u32.to_le_bytes());
        raw.extend(((items.len() + 32) as u32).to_le_bytes());
        raw.extend(2u32.to_le_bytes());
        raw.extend(0x40000000u32.to_le_bytes());
        raw.extend([0; 8]);
        fs::write(&path, &raw).unwrap();
        fs::write(
            directory.path().join("image.cue"),
            "TITLE \"Pathétique\"\nFILE \"image.ape\" WAVE\n TRACK 01 AUDIO\n INDEX 01 00:00:00",
        )
        .unwrap();
        let mut file = File::open(&path).unwrap();
        let (start, tail) = replacement_tail(&mut file, &path).unwrap();
        assert_eq!(start, 128);
        assert!(tail.windows(binary.len()).any(|window| window == binary));
        assert!(
            tail.windows("Pathétique".len())
                .any(|window| window == "Pathétique".as_bytes())
        );
        let mut expected = raw[..128].to_vec();
        expected.extend(&tail);
        let mut reader = ReadOnlyTail {
            file,
            start,
            tail,
            position: 0,
        };
        let mut read_back = Vec::new();
        reader.read_to_end(&mut read_back).unwrap();
        assert_eq!(read_back, expected);
        reader.seek(SeekFrom::Start(124)).unwrap();
        let mut boundary = [0; 16];
        reader.read_exact(&mut boundary).unwrap();
        assert_eq!(boundary, expected[124..140]);
        reader.seek(SeekFrom::End(-32)).unwrap();
        let mut magic = [0; 8];
        reader.read_exact(&mut magic).unwrap();
        assert_eq!(&magic, b"APETAGEX");
        assert!(reader.seek(SeekFrom::Current(i64::MIN)).is_err());
        reader.seek(SeekFrom::End(10)).unwrap();
        assert_eq!(reader.read(&mut magic).unwrap(), 0);
        assert_eq!(fs::read(path).unwrap(), raw);
    }
}
