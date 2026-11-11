//! Domain types. See `GLOSSARY.md` for what each term means.

use std::path::PathBuf;

/// Where a metadata value came from. Higher variants win: a user edit beats
/// everything, then embedded tags, then the CUE sheet, then the folder name.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum FieldSource {
    FolderName,
    Cue,
    Tag,
    UserEdit,
}

impl FieldSource {
    pub fn as_str(self) -> &'static str {
        match self {
            FieldSource::FolderName => "folder_name",
            FieldSource::Cue => "cue",
            FieldSource::Tag => "tag",
            FieldSource::UserEdit => "user_edit",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "folder_name" => FieldSource::FolderName,
            "cue" => FieldSource::Cue,
            "tag" => FieldSource::Tag,
            "user_edit" => FieldSource::UserEdit,
            _ => return None,
        })
    }
}

/// A metadata value together with its provenance.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Field<T> {
    pub value: T,
    pub source: FieldSource,
}

impl<T> Field<T> {
    pub fn new(value: T, source: FieldSource) -> Self {
        Self { value, source }
    }
}

/// Keep whichever candidate has the stronger source; ties keep the existing one.
pub fn merge_field<T>(current: &mut Option<Field<T>>, candidate: Option<Field<T>>) {
    match (current.as_ref(), candidate) {
        (_, None) => {}
        (None, Some(c)) => *current = Some(c),
        (Some(cur), Some(c)) if c.source > cur.source => *current = Some(c),
        _ => {}
    }
}

/// The album-level fields that carry provenance.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AlbumMetadata {
    pub title: Option<Field<String>>,
    pub album_artist: Option<Field<String>>,
    pub year: Option<Field<u16>>,
    pub genre: Option<Field<String>>,
    pub catalog_number: Option<Field<String>>,
    pub edition: Option<Field<String>>,
    pub composer: Option<Field<String>>,
    pub performers: Option<Field<String>>,
    pub conductor: Option<Field<String>>,
    pub orchestra: Option<Field<String>>,
}

/// Names of the album fields as stored in the index.
pub const ALBUM_FIELDS: &[&str] = &[
    "title",
    "album_artist",
    "year",
    "genre",
    "catalog_number",
    "edition",
    "composer",
    "performers",
    "conductor",
    "orchestra",
];

impl AlbumMetadata {
    /// Every set field as `(name, value, source)`, values rendered as text.
    pub fn entries(&self) -> Vec<(&'static str, String, FieldSource)> {
        let mut out = Vec::new();
        let mut push = |name: &'static str, f: &Option<Field<String>>| {
            if let Some(f) = f {
                out.push((name, f.value.clone(), f.source));
            }
        };
        push("title", &self.title);
        push("album_artist", &self.album_artist);
        push("genre", &self.genre);
        push("catalog_number", &self.catalog_number);
        push("edition", &self.edition);
        push("composer", &self.composer);
        push("performers", &self.performers);
        push("conductor", &self.conductor);
        push("orchestra", &self.orchestra);
        if let Some(y) = &self.year {
            out.push(("year", y.value.to_string(), y.source));
        }
        out
    }

    /// Apply one stored `(name, value, source)` entry, respecting priority.
    pub fn apply(&mut self, name: &str, value: &str, source: FieldSource) {
        let s = || Some(Field::new(value.to_string(), source));
        match name {
            "title" => merge_field(&mut self.title, s()),
            "album_artist" => merge_field(&mut self.album_artist, s()),
            "genre" => merge_field(&mut self.genre, s()),
            "catalog_number" => merge_field(&mut self.catalog_number, s()),
            "edition" => merge_field(&mut self.edition, s()),
            "composer" => merge_field(&mut self.composer, s()),
            "performers" => merge_field(&mut self.performers, s()),
            "conductor" => merge_field(&mut self.conductor, s()),
            "orchestra" => merge_field(&mut self.orchestra, s()),
            "year" => merge_field(
                &mut self.year,
                value.parse().ok().map(|y| Field::new(y, source)),
            ),
            _ => {}
        }
    }
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum AudioFormat {
    Flac,
    Ape,
    WavPack,
    Wav,
    Aiff,
    Dsf,
    Dff,
    Mp3,
    M4a,
    Aac,
    Ogg,
    Opus,
}

impl AudioFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        Some(match ext.to_ascii_lowercase().as_str() {
            "flac" => Self::Flac,
            "ape" => Self::Ape,
            "wv" => Self::WavPack,
            "wav" => Self::Wav,
            "aif" | "aiff" => Self::Aiff,
            "dsf" => Self::Dsf,
            "dff" => Self::Dff,
            "mp3" => Self::Mp3,
            "m4a" | "alac" => Self::M4a,
            "aac" => Self::Aac,
            "ogg" => Self::Ogg,
            "opus" => Self::Opus,
            _ => return None,
        })
    }

    pub fn is_dsd(self) -> bool {
        matches!(self, Self::Dsf | Self::Dff)
    }

    pub fn is_lossless(self) -> bool {
        !matches!(self, Self::Mp3 | Self::Aac | Self::Ogg | Self::Opus)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Flac => "FLAC",
            Self::Ape => "APE",
            Self::WavPack => "WavPack",
            Self::Wav => "WAV",
            Self::Aiff => "AIFF",
            Self::Dsf => "DSF",
            Self::Dff => "DFF",
            Self::Mp3 => "MP3",
            Self::M4a => "M4A",
            Self::Aac => "AAC",
            Self::Ogg => "Ogg Vorbis",
            Self::Opus => "Opus",
        }
    }
}

/// A folder the user added as the top of a music library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryRoot {
    pub path: PathBuf,
}

/// A folder that only groups albums (a box set series, a performer folder).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collection {
    pub path: PathBuf,
    pub name: String,
    /// Paths of the albums directly or indirectly under it.
    pub albums: Vec<PathBuf>,
    /// The innermost enclosing collection, if any.
    pub parent: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CoverSource {
    /// An image file next to the audio.
    File(PathBuf),
    /// A picture embedded in this audio file's tags.
    Embedded(PathBuf),
    /// A particular picture in the file's tag order (zero based).
    EmbeddedPicture { path: PathBuf, index: u32 },
}

impl CoverSource {
    pub fn path(&self) -> &std::path::Path {
        match self {
            Self::File(path) | Self::Embedded(path) | Self::EmbeddedPicture { path, .. } => path,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Cover {
    pub source: CoverSource,
}

/// CUE time position, in CD frames (75 per second).
pub type CueFrames = u32;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TrackSource {
    /// One audio file is one track.
    File { path: PathBuf },
    /// A span of a whole-disc image, described by a CUE sheet.
    Virtual {
        image: PathBuf,
        cue: PathBuf,
        start: CueFrames,
        /// `None` for the last track: it runs to the end of the image.
        end: Option<CueFrames>,
    },
}

impl TrackSource {
    pub fn audio_path(&self) -> &PathBuf {
        match self {
            TrackSource::File { path } => path,
            TrackSource::Virtual { image, .. } => image,
        }
    }
}

/// Technical properties read from the audio stream.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AudioProperties {
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u8>,
    pub channels: Option<u8>,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Track {
    pub number: Option<u32>,
    pub title: Option<Field<String>>,
    pub artist: Option<Field<String>>,
    pub format: AudioFormat,
    pub source: TrackSource,
    pub properties: AudioProperties,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Disc {
    /// 1-based.
    pub number: u32,
    pub path: PathBuf,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Album {
    pub path: PathBuf,
    /// Physical Album directories retained by local merges and splits.
    /// Empty in older snapshots means the ordinary Album's own path.
    #[serde(default)]
    pub source_directories: Vec<PathBuf>,
    pub folder_name: String,
    pub discs: Vec<Disc>,
    pub cover: Option<Cover>,
    pub metadata: AlbumMetadata,
    /// The innermost enclosing collection, if any.
    pub collection: Option<PathBuf>,
    /// Content signature: a hash of each audio file's path relative to the
    /// album folder plus its size. Unchanged when the album folder is renamed
    /// or moved, so a rescan can propose the old and new paths as one album.
    pub signature: String,
}

/// Prefix of the synthetic path given to an album assembled from loose audio
/// files directly in a library root (`<root>/.voon-loose/<artist>/<album>`).
/// Dot folders are never scanned, so it cannot clash with a real folder.
pub const LOOSE_DIR: &str = ".voon-loose";

/// Content signature of an album from `(relative path, size)` pairs.
/// FNV-1a 64 over the sorted pairs, prefixed with the file count.
pub fn content_signature(files: &mut [(String, u64)]) -> String {
    files.sort();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for (path, size) in files.iter() {
        eat(path.as_bytes());
        eat(&[0]);
        eat(&size.to_le_bytes());
    }
    format!("{}:{h:016x}", files.len())
}

impl Album {
    pub fn physical_directories(&self) -> Vec<&std::path::Path> {
        if !self.source_directories.is_empty() {
            self.source_directories
                .iter()
                .map(PathBuf::as_path)
                .collect()
        } else if self
            .path
            .components()
            .any(|part| part.as_os_str() == LOOSE_DIR)
        {
            // Legacy Loose audio snapshots already record the physical files.
            self.tracks()
                .filter_map(|track| track.source.audio_path().parent())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect()
        } else {
            vec![self.path.as_path()]
        }
    }

    pub fn tracks(&self) -> impl Iterator<Item = &Track> {
        self.discs.iter().flat_map(|d| d.tracks.iter())
    }

    /// Title to show: the resolved title or the folder name.
    pub fn display_title(&self) -> &str {
        self.metadata
            .title
            .as_ref()
            .map(|f| f.value.as_str())
            .unwrap_or(&self.folder_name)
    }
}
