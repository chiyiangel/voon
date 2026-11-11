//! Walks a Library root and decides what each folder is: an Album, a Disc
//! of an album, a Collection of albums, or nothing worth showing.
//!
//! The walk only reads: it lists directories, reads CUE sheets and (through
//! the caller's tag reader) audio tags. It never writes to the library.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

use regex::Regex;

use crate::cue::{self, CueSheet};
use crate::folder_name::{self, FolderHints};
use crate::model::*;
use crate::tags::{TagInfo, TagObservation, TagReadResult};
mod candidates;

struct ObservedCue {
    path: PathBuf,
    sheet: CueSheet,
    observed_at: i64,
    audio_paths: Vec<PathBuf>,
    used_for_resolution: bool,
}

pub const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "bmp", "gif"];

/// A strict child of this root, never the root itself or a traversal outside
/// it. Absolute children from a folder picker become relative stored rules.
pub(crate) fn relative_ignored_path(root: &Path, path: &Path) -> Option<PathBuf> {
    use std::path::Component;
    if path
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return None;
    }
    let relative = if path.is_absolute() {
        path.strip_prefix(root).ok()?
    } else {
        path
    };
    let normalized: PathBuf = relative
        .components()
        .filter(|part| !matches!(part, Component::CurDir))
        .collect();
    (!normalized.as_os_str().is_empty()
        && normalized
            .components()
            .all(|part| matches!(part, Component::Normal(_))))
    .then_some(normalized)
}

/// Size and modification time; a file whose stamp is unchanged is not re-read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FileStamp {
    pub size: u64,
    /// Nanoseconds since the Unix epoch; preserves same-second file changes.
    pub mtime: i64,
}

impl FileStamp {
    pub fn of(path: &Path) -> Option<Self> {
        let meta = fs::metadata(path).ok()?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| i64::try_from(d.as_nanos()).unwrap_or(i64::MAX))
            .unwrap_or(0);
        Some(Self {
            size: meta.len(),
            mtime,
        })
    }
}

/// Why an album needs the user's attention (the 待处理 view).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IssueKind {
    /// A CUE sheet names an audio file that is not in the folder.
    CueTargetMissing { cue: PathBuf, file: String },
    /// A CUE sheet has no tracks.
    CueWithoutTracks { cue: PathBuf },
    /// An album folder contains another album folder.
    NestedAlbum { inner: PathBuf },
    /// A folder could not be listed.
    Unreadable { error: String },
    /// The CUE could not be read, so its Track structure is unconfirmed.
    CueUnreadable { error: String },
    /// No title from tags or CUE; only the folder name is known.
    TitleFromFolderOnly,
    /// An audio file directly in a library root has no album tag, so it
    /// cannot be grouped into an album.
    LooseUntagged { file: PathBuf },
    /// Metadata failure does not imply missing audio or incomplete structure.
    TagRead {
        status: String,
        reason: String,
        retained: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanIssue {
    pub path: PathBuf,
    pub kind: IssueKind,
}

#[derive(Clone, Debug, Default)]
pub struct ScanCoverage {
    /// Directories enumerated; individual failed entries are scoped separately.
    pub listed_directories: Vec<PathBuf>,
    /// A failure anywhere in an Album's physical subtree protects its structure.
    pub incomplete_paths: Vec<PathBuf>,
    /// Subtrees skipped when cancellation was observed; absence is unconfirmed.
    pub unvisited_paths: Vec<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub struct ScanResult {
    pub albums: Vec<Album>,
    pub collections: Vec<Collection>,
    pub issues: Vec<ScanIssue>,
    /// Every audio file seen, for incremental rescans.
    pub stamps: Vec<(PathBuf, FileStamp)>,
    pub tag_observations: BTreeMap<PathBuf, TagObservation>,
    pub cancelled: bool,
    pub coverage: ScanCoverage,
}

impl ScanResult {
    /// A root listing is required before absent paths can be considered gone.
    /// Descendant failures still protect their respective Album subtrees.
    pub fn root_was_listed(&self, root: &Path) -> bool {
        self.coverage
            .listed_directories
            .iter()
            .any(|path| path == root)
    }

    /// Includes legacy/controlled scanner errors, so a caller cannot publish a
    /// partial Album merely by omitting its coverage record.
    pub fn incomplete_paths(&self) -> Vec<&Path> {
        self.coverage
            .incomplete_paths
            .iter()
            .chain(&self.coverage.unvisited_paths)
            .map(PathBuf::as_path)
            .chain(self.issues.iter().filter_map(|issue| {
                matches!(
                    issue.kind,
                    IssueKind::Unreadable { .. }
                        | IssueKind::CueUnreadable { .. }
                        | IssueKind::CueWithoutTracks { .. }
                        | IssueKind::CueTargetMissing { .. }
                )
                .then_some(issue.path.as_path())
            }))
            .collect()
    }

    pub fn album_is_complete(&self, album: &Album) -> bool {
        !album.physical_directories().iter().any(|source| {
            self.incomplete_paths()
                .iter()
                .any(|failed| source.starts_with(failed) || failed.starts_with(source))
        })
    }
}

/// Progress reported while scanning.
pub enum ScanEvent<'a> {
    Album(&'a Album),
    Directory(&'a Path),
}

/// Reads tags for one audio file. Returning `None` means "no tags".
pub trait TagReader {
    fn read(&mut self, path: &Path, format: AudioFormat, stamp: FileStamp) -> Option<TagInfo>;

    /// Existing structure-only readers remain valid. Real readers report failures explicitly.
    fn read_observation(
        &mut self,
        path: &Path,
        format: AudioFormat,
        stamp: FileStamp,
    ) -> TagObservation {
        TagObservation::new(
            self.read(path, format, stamp)
                .map(TagReadResult::success)
                .unwrap_or_else(|| TagReadResult::NoTags(TagInfo::default())),
        )
    }
}

/// Reads every file from disk, no caching.
pub struct DiskTags;

impl TagReader for DiskTags {
    fn read(&mut self, path: &Path, format: AudioFormat, _stamp: FileStamp) -> Option<TagInfo> {
        crate::tags::read(path, format).ok()
    }

    fn read_observation(
        &mut self,
        path: &Path,
        format: AudioFormat,
        _: FileStamp,
    ) -> TagObservation {
        crate::tags::observe(path, format)
    }
}

/// Skips tag reading entirely (structure only).
pub struct NoTags;

impl TagReader for NoTags {
    fn read(&mut self, _: &Path, _: AudioFormat, _: FileStamp) -> Option<TagInfo> {
        None
    }
}

/// Scan a root with no progress reporting and no cancellation.
pub fn scan_root(root: &Path, tags: &mut dyn TagReader) -> ScanResult {
    scan_root_with(root, tags, &AtomicBool::new(false), &mut |_| {})
}

/// Scan a root, reporting each album as soon as it is complete. Setting
/// `cancel` stops at the next folder; the result then has `cancelled` set
/// and holds what was found so far.
pub fn scan_root_with(
    root: &Path,
    tags: &mut dyn TagReader,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(ScanEvent<'_>),
) -> ScanResult {
    scan_root_with_options(root, tags, cancel, progress, &ScanOptions::default())
}

/// Additional rules for a single Library root. Paths identify entire
/// subtrees; invalid paths and the root itself never match.
#[derive(Clone, Debug, Default)]
pub struct ScanOptions {
    pub ignored_paths: Vec<PathBuf>,
}

/// Scan with user-selected subtrees excluded before folder classification,
/// including Disc and artwork folders that are read without recursive visits.
pub fn scan_root_with_options(
    root: &Path,
    tags: &mut dyn TagReader,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(ScanEvent<'_>),
    options: &ScanOptions,
) -> ScanResult {
    let mut scanner = Scanner {
        tags,
        cancel,
        progress,
        ignored_paths: options
            .ignored_paths
            .iter()
            .filter_map(|path| relative_ignored_path(root, path))
            .map(|relative| root.join(relative))
            .collect(),
        out: ScanResult::default(),
    };
    scanner.visit(root, None, true);
    scanner.out.cancelled = cancel.load(Ordering::Relaxed);
    scanner.out
}

// ---------------------------------------------------------------------------
// Folder names

static DISC_DIR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?:cd|disc|disk|sacd|碟)\s*[-_.#]?\s*0*([0-9]{1,3})(?:[^0-9]|$)").unwrap()
});
static DISC_DIR_CN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*第\s*([0-9]{1,3})\s*[碟张]").unwrap());

/// `CD1`, `CD 02`, `Disc 2`, `disk3 - Bonus`, `第2碟` → the disc number.
pub fn disc_number(name: &str) -> Option<u32> {
    DISC_DIR
        .captures(name)
        .or_else(|| DISC_DIR_CN.captures(name))
        .and_then(|c| c[1].parse().ok())
}

/// Subfolders that hold scans and artwork, not music.
pub fn is_attachment_dir(name: &str) -> bool {
    let n = name.trim().to_lowercase();
    matches!(
        n.as_str(),
        "booklet"
            | "booklets"
            | "scan"
            | "scans"
            | "artwork"
            | "artworks"
            | "art"
            | "covers"
            | "cover"
            | "images"
            | "pics"
            | "pictures"
            | "扫图"
            | "封面"
            | "图片"
            | "歌词本"
    )
}

/// NAS bookkeeping and macOS metadata files.
pub fn is_junk(name: &str) -> bool {
    name.starts_with('.')
        || name.starts_with('@')
        || name.starts_with('#')
        || name.eq_ignore_ascii_case("$RECYCLE.BIN")
        || name.eq_ignore_ascii_case("System Volume Information")
        || name.eq_ignore_ascii_case("Thumbs.db")
        || name.eq_ignore_ascii_case("desktop.ini")
}

fn ext_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn stem_of(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Directory listing

#[derive(Default)]
struct Listing {
    audio: Vec<(PathBuf, AudioFormat)>,
    cues: Vec<PathBuf>,
    images: Vec<PathBuf>,
    subdirs: Vec<PathBuf>,
    failures: Vec<(PathBuf, String)>,
}

fn list(dir: &Path) -> std::io::Result<Listing> {
    let mut l = Listing::default();
    for entry in fs::read_dir(dir)? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                l.failures.push((dir.into(), error.to_string()));
                continue;
            }
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_junk(&name) {
            continue;
        }
        let path = entry.path();
        // Follows symlinks: a library may link albums in from elsewhere.
        let meta = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                if !is_attachment_dir(&name) && !IMAGE_EXTENSIONS.contains(&ext_of(&path).as_str())
                {
                    l.failures.push((path, error.to_string()));
                }
                continue;
            }
        };
        if meta.is_dir() {
            l.subdirs.push(path);
            continue;
        }
        let ext = ext_of(&path);
        if let Some(fmt) = AudioFormat::from_extension(&ext) {
            l.audio.push((path, fmt));
        } else if ext == "cue" {
            l.cues.push(path);
        } else if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
            l.images.push(path);
        }
        // Everything else (.log, .pdf, .m4v, .url, .nfo, ...) is ignored.
    }
    l.audio.sort();
    l.cues.sort();
    l.images.sort();
    l.subdirs.sort();
    Ok(l)
}

// ---------------------------------------------------------------------------

struct Scanner<'a> {
    tags: &'a mut dyn TagReader,
    cancel: &'a AtomicBool,
    progress: &'a mut dyn FnMut(ScanEvent<'_>),
    ignored_paths: Vec<PathBuf>,
    out: ScanResult,
}

/// What a visited folder turned out to contain.
enum Found {
    Nothing,
    /// Album paths found at or below the folder.
    Albums(Vec<PathBuf>),
}

impl Scanner<'_> {
    fn list(&mut self, dir: &Path) -> std::io::Result<Listing> {
        let mut listing = match list(dir) {
            Ok(listing) => listing,
            Err(error) => {
                self.out.coverage.incomplete_paths.push(dir.into());
                if !self.out.issues.iter().any(|issue| {
                    issue.path == dir && matches!(issue.kind, IssueKind::Unreadable { .. })
                }) {
                    self.out.issues.push(ScanIssue {
                        path: dir.into(),
                        kind: IssueKind::Unreadable {
                            error: error.to_string(),
                        },
                    });
                }
                return Err(error);
            }
        };
        self.out.coverage.listed_directories.push(dir.into());
        for (path, error) in listing.failures.drain(..) {
            if !self
                .ignored_paths
                .iter()
                .any(|ignored| path.starts_with(ignored))
            {
                self.out.coverage.incomplete_paths.push(path.clone());
                self.out.issues.push(ScanIssue {
                    path,
                    kind: IssueKind::Unreadable { error },
                });
            }
        }
        listing.subdirs.retain(|path| {
            !self
                .ignored_paths
                .iter()
                .any(|ignored| path.starts_with(ignored))
        });
        Ok(listing)
    }

    fn visit(&mut self, dir: &Path, collection: Option<&Path>, is_root: bool) -> Found {
        if self.cancel.load(Ordering::Relaxed) {
            self.out.coverage.unvisited_paths.push(dir.into());
            return Found::Nothing;
        }
        (self.progress)(ScanEvent::Directory(dir));
        if self.cancel.load(Ordering::Relaxed) {
            self.out.coverage.unvisited_paths.push(dir.into());
            return Found::Nothing;
        }
        let listing = match self.list(dir) {
            Ok(l) => l,
            Err(_) => return Found::Nothing,
        };

        let mut disc_dirs: Vec<(u32, PathBuf, Listing)> = Vec::new();
        let mut other_dirs: Vec<PathBuf> = Vec::new();
        let mut attachment_dirs: Vec<PathBuf> = Vec::new();
        for sub in &listing.subdirs {
            let name = name_of(sub);
            if is_attachment_dir(&name) {
                attachment_dirs.push(sub.clone());
                continue;
            }
            // In a library root, `CD1` is just a folder (an album of its own).
            if !is_root
                && let Some(n) = disc_number(&name)
                && let Ok(sl) = self.list(sub)
                && !sl.audio.is_empty()
            {
                disc_dirs.push((n, sub.clone(), sl));
                continue;
            }
            other_dirs.push(sub.clone());
        }

        let is_album = !listing.audio.is_empty() || !disc_dirs.is_empty();
        if is_album && !is_root {
            let album = self.build_album(dir, &listing, disc_dirs, &attachment_dirs, collection);
            let mut found = Vec::new();
            // Albums inside an album folder are kept, but flagged.
            for sub in other_dirs {
                if let Found::Albums(inner) = self.visit(&sub, collection, false) {
                    for p in &inner {
                        self.out.issues.push(ScanIssue {
                            path: dir.to_path_buf(),
                            kind: IssueKind::NestedAlbum { inner: p.clone() },
                        });
                    }
                    found.extend(inner);
                }
            }
            if self.out.album_is_complete(&album) {
                found.insert(0, album.path.clone());
                (self.progress)(ScanEvent::Album(&album));
                self.out.albums.push(album);
            }
            return Found::Albums(found);
        }

        // Loose audio directly in the root is grouped by its album tags.
        if is_root && !listing.audio.is_empty() {
            self.loose_root_albums(dir, &listing.audio);
        }

        // Not an album: maybe a Collection of albums.
        let this_collection = (!is_root).then_some(dir);
        let mut found = Vec::new();
        for sub in other_dirs {
            if let Found::Albums(inner) = self.visit(&sub, this_collection.or(collection), false) {
                found.extend(inner);
            }
        }
        if found.is_empty() {
            return Found::Nothing;
        }
        if !is_root {
            self.out.collections.push(Collection {
                path: dir.to_path_buf(),
                name: name_of(dir),
                albums: found.clone(),
                parent: collection.map(Path::to_path_buf),
            });
        }
        Found::Albums(found)
    }

    fn build_album(
        &mut self,
        dir: &Path,
        listing: &Listing,
        mut disc_dirs: Vec<(u32, PathBuf, Listing)>,
        attachment_dirs: &[PathBuf],
        collection: Option<&Path>,
    ) -> Album {
        let mut sheets: Vec<ObservedCue> = Vec::new();
        let mut discs: Vec<Disc> = Vec::new();

        // The album folder's own audio first, then the disc subfolders.
        for tracks in self.tracks_in(dir, listing, &mut sheets) {
            discs.push(Disc {
                number: 0,
                path: dir.to_path_buf(),
                tracks,
            });
        }
        disc_dirs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        for (_, path, sl) in &disc_dirs {
            for tracks in self.tracks_in(path, sl, &mut sheets) {
                discs.push(Disc {
                    number: 0,
                    path: path.clone(),
                    tracks,
                });
            }
        }
        // Numbering: disc folders keep their own number when they are unique
        // and the album folder has no audio of its own; otherwise 1..n.
        let own = discs.iter().filter(|d| d.path == dir).count();
        let numbers: Vec<u32> = disc_dirs.iter().map(|d| d.0).collect();
        let unique: HashSet<_> = numbers.iter().collect();
        let keep = own == 0 && unique.len() == numbers.len() && discs.len() == numbers.len();
        for (i, d) in discs.iter_mut().enumerate() {
            d.number = if keep { numbers[i] } else { i as u32 + 1 };
        }

        let folder_name = name_of(dir);
        let hints = folder_name::parse(&folder_name);
        let mut album = Album {
            path: dir.to_path_buf(),
            source_directories: vec![dir.to_path_buf()],
            candidates: vec![],
            folder_name,
            discs,
            cover: None,
            metadata: AlbumMetadata::default(),
            collection: collection.map(Path::to_path_buf),
            signature: String::new(),
        };
        let tag_infos = self.read_tags(&mut album);
        let resolved_sheets: Vec<_> = sheets
            .iter()
            .filter(|sheet| sheet.used_for_resolution)
            .map(|sheet| sheet.sheet.clone())
            .collect();
        album.metadata = resolve_metadata(&hints, &resolved_sheets, &tag_infos);
        album.candidates = candidates::collect(&album, &hints, &sheets, &self.out.tag_observations);
        album.cover = find_cover(&listing.images, &disc_dirs, attachment_dirs)
            .map(|p| Cover {
                source: CoverSource::File(p),
            })
            .or_else(|| {
                tag_infos
                    .iter()
                    .find(|(_, t)| t.has_picture)
                    .map(|(p, _)| Cover {
                        source: CoverSource::Embedded(p.clone()),
                    })
            });
        if album
            .metadata
            .title
            .as_ref()
            .is_none_or(|f| f.source == FieldSource::FolderName)
        {
            self.out.issues.push(ScanIssue {
                path: album.path.clone(),
                kind: IssueKind::TitleFromFolderOnly,
            });
        }
        album
    }

    /// Group one folder's audio into discs' worth of tracks. Each CUE + image
    /// pair is one disc; loose files together are one more.
    fn tracks_in(
        &mut self,
        dir: &Path,
        l: &Listing,
        sheets: &mut Vec<ObservedCue>,
    ) -> Vec<Vec<Track>> {
        let mut groups = Vec::new();
        let mut claimed: HashSet<PathBuf> = HashSet::new();
        let mut cue_titles: HashMap<PathBuf, (Option<String>, Option<String>, u32)> =
            HashMap::new();

        for cue_path in &l.cues {
            let bytes = match fs::read(cue_path) {
                Ok(bytes) => bytes,
                Err(error) => {
                    self.out.coverage.incomplete_paths.push(cue_path.clone());
                    self.out.issues.push(ScanIssue {
                        path: cue_path.clone(),
                        kind: IssueKind::CueUnreadable {
                            error: error.to_string(),
                        },
                    });
                    continue;
                }
            };
            let sheet = cue::parse_bytes(&bytes);
            if sheet.track_count() == 0 {
                self.out.issues.push(ScanIssue {
                    path: dir.to_path_buf(),
                    kind: IssueKind::CueWithoutTracks {
                        cue: cue_path.clone(),
                    },
                });
                continue;
            }
            let single_file = sheet.files.len() == 1;
            let mut resolved = Vec::new();
            let mut missing = false;
            for f in &sheet.files {
                match resolve_cue_target(cue_path, &f.name, &l.audio, single_file) {
                    Some(p) => resolved.push(p),
                    None => {
                        missing = true;
                        self.out.issues.push(ScanIssue {
                            path: dir.to_path_buf(),
                            kind: IssueKind::CueTargetMissing {
                                cue: cue_path.clone(),
                                file: f.name.clone(),
                            },
                        });
                    }
                }
            }
            if missing {
                continue;
            }
            let audio_paths = resolved.iter().map(|(path, _)| path.clone()).collect();
            let used_for_resolution = !resolved.iter().any(|path| claimed.contains(&path.0));
            sheets.push(ObservedCue {
                path: cue_path.clone(),
                sheet: sheet.clone(),
                observed_at: crate::tags::observation_time(),
                audio_paths,
                used_for_resolution,
            });
            if !used_for_resolution {
                // Keep alternate evidence without adding duplicate Tracks or changing winners.
                continue;
            }
            if single_file {
                let (image, format) = resolved.remove(0);
                let tracks = &sheet.files[0].tracks;
                let mut out = Vec::new();
                for (i, t) in tracks.iter().enumerate() {
                    let start = t.start.unwrap_or(0);
                    let end = tracks.get(i + 1).and_then(|n| n.start);
                    out.push(Track {
                        number: Some(t.number),
                        title: t.title.clone().map(|v| Field::new(v, FieldSource::Cue)),
                        artist: t.performer.clone().map(|v| Field::new(v, FieldSource::Cue)),
                        format,
                        source: TrackSource::Virtual {
                            image: image.clone(),
                            cue: cue_path.clone(),
                            start,
                            end,
                        },
                        properties: AudioProperties::default(),
                    });
                }
                claimed.insert(image);
                groups.push(out);
            } else {
                // One FILE per track: the files are real tracks, the sheet
                // only contributes titles.
                for ((path, _), f) in resolved.iter().zip(&sheet.files) {
                    if let Some(t) = f.tracks.first() {
                        cue_titles.insert(
                            path.clone(),
                            (t.title.clone(), t.performer.clone(), t.number),
                        );
                    }
                }
            }
        }

        let loose: Vec<Track> = l
            .audio
            .iter()
            .filter(|(p, _)| !claimed.contains(p))
            .map(|(p, fmt)| {
                let cue = cue_titles.remove(p);
                Track {
                    number: cue.as_ref().map(|c| c.2),
                    title: cue
                        .as_ref()
                        .and_then(|c| c.0.clone())
                        .map(|v| Field::new(v, FieldSource::Cue)),
                    artist: cue
                        .as_ref()
                        .and_then(|c| c.1.clone())
                        .map(|v| Field::new(v, FieldSource::Cue)),
                    format: *fmt,
                    source: TrackSource::File { path: p.clone() },
                    properties: AudioProperties::default(),
                }
            })
            .collect();
        if !loose.is_empty() {
            groups.push(loose);
        }
        groups
    }

    /// Read tags once per audio file, fill track fields and properties, and
    /// return the per-file tag info for album-level resolution.
    /// Also sets the album's content signature.
    fn read_tags(&mut self, album: &mut Album) -> Vec<(PathBuf, TagInfo)> {
        let mut seen: BTreeMap<PathBuf, Option<TagInfo>> = BTreeMap::new();
        let mut sizes: Vec<(String, u64)> = Vec::new();
        let base = album.path.clone();
        for disc in &mut album.discs {
            for track in &mut disc.tracks {
                let path = track.source.audio_path().clone();
                let info = seen.entry(path.clone()).or_insert_with(|| {
                    let stamp = FileStamp::of(&path)?;
                    self.out.stamps.push((path.clone(), stamp));
                    let rel = path.strip_prefix(&base).unwrap_or(&path);
                    sizes.push((rel.to_string_lossy().into_owned(), stamp.size));
                    self.observe_tags(&path, track.format, stamp)
                });
                if let Some(info) = info {
                    track.properties = info.properties.clone();
                    if let TrackSource::File { .. } = track.source {
                        merge_field(
                            &mut track.title,
                            info.title.clone().map(|v| Field::new(v, FieldSource::Tag)),
                        );
                        merge_field(
                            &mut track.artist,
                            info.artist.clone().map(|v| Field::new(v, FieldSource::Tag)),
                        );
                        if info.track.is_some() {
                            track.number = info.track;
                        }
                    }
                }
                if let TrackSource::Virtual { start, end, .. } = track.source {
                    // Known CUE boundaries do not depend on readable tags.
                    // The last track needs the image's total duration.
                    track.properties.duration_ms = match end {
                        Some(e) => Some((e.saturating_sub(start)) as u64 * 1000 / 75),
                        None => track
                            .properties
                            .duration_ms
                            .map(|d| d.saturating_sub(start as u64 * 1000 / 75)),
                    };
                }
            }
            disc.tracks.sort_by_key(|t| t.number.unwrap_or(u32::MAX));
        }
        album.signature = content_signature(&mut sizes);
        seen.into_iter()
            .filter_map(|(p, t)| t.map(|t| (p, t)))
            .collect()
    }
}

impl Scanner<'_> {
    fn observe_tags(
        &mut self,
        path: &Path,
        format: AudioFormat,
        stamp: FileStamp,
    ) -> Option<TagInfo> {
        let observation = self.tags.read_observation(path, format, stamp);
        if let Some(reason) = observation.result.reason() {
            self.out.issues.push(ScanIssue {
                path: path.to_path_buf(),
                kind: IssueKind::TagRead {
                    status: observation.result.status().into(),
                    reason: reason.into(),
                    retained: observation.last_success.is_some(),
                },
            });
        }
        let info = observation.info().cloned();
        self.out
            .tag_observations
            .insert(path.to_path_buf(), observation);
        info
    }

    /// Audio files directly in a library root: one album per (album artist,
    /// album) tag pair; files without an album tag become issues.
    fn loose_root_albums(&mut self, root: &Path, audio: &[(PathBuf, AudioFormat)]) {
        type Group = Vec<(PathBuf, AudioFormat, FileStamp, TagInfo)>;
        let mut groups: BTreeMap<(String, String), Group> = BTreeMap::new();
        for (path, format) in audio {
            if self.cancel.load(Ordering::Relaxed) {
                return;
            }
            let Some(stamp) = FileStamp::of(path) else {
                continue;
            };
            self.out.stamps.push((path.clone(), stamp));
            let info = self.observe_tags(path, *format, stamp);
            match info {
                Some(info) if info.album.is_some() => {
                    let key = (
                        info.album_artist.clone().unwrap_or_default(),
                        info.album.clone().unwrap_or_default(),
                    );
                    groups
                        .entry(key)
                        .or_default()
                        .push((path.clone(), *format, stamp, info));
                }
                _ => self.out.issues.push(ScanIssue {
                    path: root.to_path_buf(),
                    kind: IssueKind::LooseUntagged { file: path.clone() },
                }),
            }
        }
        for ((artist, title), files) in groups {
            let safe = |s: &str| s.replace(['/', '\\'], "_");
            let artist_dir = if artist.is_empty() {
                "_".to_string()
            } else {
                safe(&artist)
            };
            let path = root.join(LOOSE_DIR).join(artist_dir).join(safe(&title));
            let mut discs: BTreeMap<u32, Vec<Track>> = BTreeMap::new();
            let mut sizes = Vec::new();
            let mut infos = Vec::new();
            for (file, format, stamp, info) in files {
                sizes.push((name_of(&file), stamp.size));
                discs
                    .entry(info.disc.unwrap_or(1))
                    .or_default()
                    .push(Track {
                        number: info.track,
                        title: info.title.clone().map(|v| Field::new(v, FieldSource::Tag)),
                        artist: info.artist.clone().map(|v| Field::new(v, FieldSource::Tag)),
                        format,
                        source: TrackSource::File { path: file.clone() },
                        properties: info.properties.clone(),
                    });
                infos.push((file, info));
            }
            let discs = discs
                .into_iter()
                .map(|(number, mut tracks)| {
                    tracks.sort_by_key(|t| t.number.unwrap_or(u32::MAX));
                    Disc {
                        number,
                        path: root.to_path_buf(),
                        tracks,
                    }
                })
                .collect();
            let mut album = Album {
                path,
                source_directories: vec![root.to_path_buf()],
                candidates: vec![],
                folder_name: title,
                discs,
                cover: infos
                    .iter()
                    .find(|(_, t)| t.has_picture)
                    .map(|(p, _)| Cover {
                        source: CoverSource::Embedded(p.clone()),
                    }),
                metadata: resolve_metadata(&FolderHints::default(), &[], &infos),
                collection: None,
                signature: content_signature(&mut sizes),
            };
            album.candidates = candidates::collect(
                &album,
                &FolderHints::default(),
                &[],
                &self.out.tag_observations,
            );
            (self.progress)(ScanEvent::Album(&album));
            self.out.albums.push(album);
        }
    }
}

/// Find the audio file a CUE `FILE` line refers to. Rips often name a WAV
/// that was later compressed to APE/FLAC, so the stem alone also matches.
fn resolve_cue_target(
    cue_path: &Path,
    name: &str,
    audio: &[(PathBuf, AudioFormat)],
    single_file: bool,
) -> Option<(PathBuf, AudioFormat)> {
    // Some sheets carry a Windows path; only the last component matters.
    let wanted = name
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(name)
        .to_lowercase();
    let wanted_stem = Path::new(&wanted)
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let by_name = audio
        .iter()
        .find(|(p, _)| name_of(p).to_lowercase() == wanted);
    let by_stem = || audio.iter().find(|(p, _)| stem_of(p) == wanted_stem);
    // `CDImage.ape.cue` or `CDImage.cue` next to `CDImage.ape`.
    let by_cue_name = || {
        if !single_file {
            return None;
        }
        let cue_stem = stem_of(cue_path);
        audio
            .iter()
            .find(|(p, _)| stem_of(p) == cue_stem || name_of(p).to_lowercase() == cue_stem)
    };
    by_name.or_else(by_stem).or_else(by_cue_name).cloned()
}

const COVER_STEMS: &[&str] = &["cover", "folder", "front", "albumart", "封面", "正面"];

fn cover_rank(path: &Path) -> Option<usize> {
    let stem = stem_of(path);
    if let Some(i) = COVER_STEMS.iter().position(|s| stem == *s) {
        return Some(i);
    }
    if stem.contains("front") || stem.contains("cover") || stem.contains("封面") {
        return Some(COVER_STEMS.len());
    }
    None
}

fn best_cover(images: &[PathBuf]) -> Option<PathBuf> {
    images
        .iter()
        .filter_map(|p| cover_rank(p).map(|r| (r, p)))
        .min_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(b.1)))
        .map(|(_, p)| p.clone())
}

fn find_cover(
    images: &[PathBuf],
    disc_dirs: &[(u32, PathBuf, Listing)],
    attachment_dirs: &[PathBuf],
) -> Option<PathBuf> {
    if let Some(p) = best_cover(images) {
        return Some(p);
    }
    for (_, _, l) in disc_dirs {
        if let Some(p) = best_cover(&l.images) {
            return Some(p);
        }
    }
    for a in attachment_dirs {
        if let Ok(l) = list(a)
            && let Some(p) = best_cover(&l.images)
        {
            return Some(p);
        }
    }
    // A single unnamed image next to the music is almost always the cover.
    if images.len() == 1 {
        return Some(images[0].clone());
    }
    None
}

/// Most common non-empty value, ties broken by first appearance.
fn majority<'a>(values: impl Iterator<Item = &'a str>) -> Option<String> {
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for v in values {
        match counts.iter_mut().find(|(k, _)| *k == v) {
            Some(c) => c.1 += 1,
            None => counts.push((v, 1)),
        }
    }
    let max = counts.iter().map(|c| c.1).max()?;
    counts
        .into_iter()
        .find(|c| c.1 == max)
        .map(|c| c.0.to_string())
}

/// Combine folder-name hints, CUE sheets and tags into album fields.
/// Priority: tags > CUE > folder name (user edits are applied by the index).
pub fn resolve_metadata(
    hints: &FolderHints,
    sheets: &[CueSheet],
    tags: &[(PathBuf, TagInfo)],
) -> AlbumMetadata {
    use FieldSource::*;
    let mut m = AlbumMetadata::default();
    let f = |v: Option<String>, s| v.map(|v| Field::new(v, s));

    // Folder name.
    merge_field(&mut m.title, f(hints.title.clone(), FolderName));
    merge_field(&mut m.album_artist, f(hints.artist.clone(), FolderName));
    merge_field(&mut m.year, hints.year.map(|y| Field::new(y, FolderName)));
    merge_field(
        &mut m.catalog_number,
        f(hints.catalog_number.clone(), FolderName),
    );
    merge_field(&mut m.edition, f(hints.edition.clone(), FolderName));

    // CUE sheets (genre deliberately ignored: genre comes from tags only).
    if let Some(s) = sheets.first() {
        merge_field(&mut m.title, f(s.title.clone(), Cue));
        merge_field(&mut m.album_artist, f(s.performer.clone(), Cue));
        let mut performers = Vec::new();
        for sheet in sheets {
            for track in sheet.files.iter().flat_map(|file| &file.tracks) {
                if let Some(performer) = track
                    .performer
                    .as_deref()
                    .or(sheet.performer.as_deref())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    && !performers.contains(&performer)
                {
                    performers.push(performer);
                }
            }
        }
        merge_field(
            &mut m.performers,
            f(
                if performers.is_empty() {
                    s.performer.clone()
                } else {
                    Some(performers.join("; "))
                },
                Cue,
            ),
        );
        merge_field(
            &mut m.composer,
            f(s.composer.clone().or_else(|| s.songwriter.clone()), Cue),
        );
        merge_field(&mut m.catalog_number, f(s.catalog.clone(), Cue));
        let year = s
            .date
            .as_deref()
            .and_then(|d| d.get(..4))
            .and_then(|y| y.parse::<u16>().ok());
        merge_field(&mut m.year, year.map(|y| Field::new(y, Cue)));
    }

    // Tags: the value most tracks agree on.
    let t = || tags.iter().map(|(_, t)| t);
    let pick =
        |get: fn(&TagInfo) -> Option<&String>| majority(t().filter_map(get).map(String::as_str));
    merge_field(&mut m.title, f(pick(|t| t.album.as_ref()), Tag));
    let album_artist = pick(|t| t.album_artist.as_ref());
    merge_field(
        &mut m.album_artist,
        f(
            album_artist.clone().or_else(|| pick(|t| t.artist.as_ref())),
            Tag,
        ),
    );
    merge_field(&mut m.genre, f(pick(|t| t.genre.as_ref()), Tag));
    merge_field(&mut m.composer, f(pick(|t| t.composer.as_ref()), Tag));
    merge_field(&mut m.conductor, f(pick(|t| t.conductor.as_ref()), Tag));
    merge_field(
        &mut m.catalog_number,
        f(pick(|t| t.catalog_number.as_ref()), Tag),
    );
    let year = majority(
        t().filter_map(|t| t.year)
            .map(|y| y.to_string())
            .collect::<Vec<_>>()
            .iter()
            .map(String::as_str),
    );
    merge_field(
        &mut m.year,
        year.and_then(|y| y.parse().ok())
            .map(|y| Field::new(y, Tag)),
    );
    // Performers: the distinct track artists, in order.
    let mut performers: Vec<&str> = Vec::new();
    for a in t().filter_map(|t| t.artist.as_deref()) {
        if !performers.contains(&a) {
            performers.push(a);
        }
    }
    if !performers.is_empty() {
        merge_field(
            &mut m.performers,
            Some(Field::new(performers.join("; "), Tag)),
        );
    }
    m
}
