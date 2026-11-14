//! Bounded recovery: tag failures never enter the directory walker.
use super::*;

/// Read precisely the requested audio sources. Related CUE evidence may be
/// consulted by the tag decoder; no directory structure is inferred here.
pub fn retry_tag_paths(
    root: &Path,
    paths: &[PathBuf],
    tags: &mut dyn TagReader,
    cancel: &AtomicBool,
) -> ScanResult {
    let mut out = ScanResult::default();
    for path in paths.iter().collect::<std::collections::BTreeSet<_>>() {
        if cancel.load(Ordering::Relaxed) {
            out.cancelled = true;
            break;
        }
        if relative_ignored_path(root, path).is_none() {
            continue;
        }
        let Some(format) = AudioFormat::from_extension(&ext_of(path)) else {
            continue;
        };
        // Missing/inaccessible files must produce another failed attempt, not
        // silently disappear or erase the last successful metadata.
        let stamp = FileStamp::of(path).unwrap_or(FileStamp { size: 0, mtime: 0 });
        if let Some(observation) = tags.observe(path, format, stamp) {
            out.stamps.push((path.clone(), stamp));
            out.tag_observations.insert(path.clone(), observation);
        }
    }
    out
}

/// Retry structural failures from the nearest known physical album (so CD
/// folders keep their identity), or the failed directory when never indexed.
/// A root-level file requires only a shallow root listing, never its children.
pub fn scan_scopes_with_options(
    root: &Path,
    paths: &[PathBuf],
    known_albums: &[Album],
    tags: &mut dyn TagReader,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(ScanEvent<'_>),
    options: &ScanOptions,
) -> ScanResult {
    let mut scopes = Vec::new();
    for path in paths {
        if path != root && relative_ignored_path(root, path).is_none() {
            continue;
        }
        let known = known_albums
            .iter()
            .filter(|album| !album.path.starts_with(root.join(LOOSE_DIR)))
            .filter(|album| path.starts_with(&album.path))
            .max_by_key(|album| album.path.components().count());
        let start = if let Some(album) = known {
            album.path.clone()
        } else if AudioFormat::from_extension(&ext_of(path)).is_some() || ext_of(path) == "cue" {
            let mut dir = path.parent().unwrap_or(root);
            // Preserve multi-disc identity on the first successful retry too.
            if dir != root && disc_number(&name_of(dir)).is_some() && dir.parent() != Some(root) {
                dir = dir.parent().unwrap_or(dir);
            }
            dir.to_path_buf()
        } else if path != root
            && disc_number(&name_of(path)).is_some()
            && path.parent() != Some(root)
        {
            path.parent().unwrap_or(path).to_path_buf()
        } else {
            path.clone()
        };
        scopes.push((start, path == root));
    }
    scopes.sort();
    scopes.dedup();
    let ignored_paths: Vec<_> = options
        .ignored_paths
        .iter()
        .filter_map(|path| relative_ignored_path(root, path))
        .map(|path| root.join(path))
        .collect();
    let mut scanner = Scanner {
        cached_stamps: None,
        shallow_root: false,
        tags,
        cancel,
        progress,
        ignored_paths,
        out: ScanResult::default(),
    };
    let mut visited: Vec<PathBuf> = vec![];
    for (start, whole_root) in scopes {
        if visited.iter().any(|parent| start.starts_with(parent))
            || scanner
                .ignored_paths
                .iter()
                .any(|ignored| start.starts_with(ignored))
        {
            continue;
        }
        scanner.shallow_root = start == root && !whole_root;
        let collection = known_albums
            .iter()
            .find(|album| album.path == start)
            .map(|album| album.collection.as_deref())
            .unwrap_or_else(|| {
                start
                    .parent()
                    .filter(|parent| *parent != root && parent.starts_with(root))
            });
        scanner.visit(&start, collection, start == root);
        if !scanner.shallow_root {
            visited.push(start);
        }
    }
    scanner.out.cancelled = cancel.load(Ordering::Relaxed);
    scanner.out
}

/// Rebuild root-file tag groups from local observations only. The internal
/// coverage marker is used for grouping, never published as filesystem proof.
pub(crate) fn regroup_cached_loose(
    root: &Path,
    stamps: BTreeMap<PathBuf, FileStamp>,
    observations: &BTreeMap<PathBuf, TagObservation>,
) -> ScanResult {
    struct SnapshotTags<'a>(&'a BTreeMap<PathBuf, TagObservation>);
    impl TagReader for SnapshotTags<'_> {
        fn read(&mut self, path: &Path, _: AudioFormat, _: FileStamp) -> Option<TagInfo> {
            self.0.get(path).and_then(TagObservation::info).cloned()
        }
        fn observe(&mut self, path: &Path, _: AudioFormat, _: FileStamp) -> Option<TagObservation> {
            self.0.get(path).cloned()
        }
    }
    let audio: Vec<_> = stamps
        .keys()
        .filter_map(|path| {
            AudioFormat::from_extension(&ext_of(path)).map(|format| (path.clone(), format))
        })
        .collect();
    let mut tags = SnapshotTags(observations);
    let cancel = AtomicBool::new(false);
    let mut progress = |_: ScanEvent<'_>| {};
    let mut scanner = Scanner {
        shallow_root: true,
        cached_stamps: Some(stamps),
        tags: &mut tags,
        cancel: &cancel,
        progress: &mut progress,
        ignored_paths: vec![],
        out: ScanResult::default(),
    };
    scanner.out.coverage.listed_directories.push(root.into());
    scanner.loose_root_albums(root, &audio);
    scanner.out.coverage = ScanCoverage::default();
    scanner.out
}

/// Re-project cached structure using new tag observations, without stat'ing
/// sibling audio, opening CUEs, or touching artwork folders.
pub(crate) fn refresh_album_tags(
    album: &mut Album,
    observations: &BTreeMap<PathBuf, TagObservation>,
) {
    let infos: Vec<_> = observations
        .iter()
        .filter_map(|(path, observation)| {
            observation.info().map(|info| (path.clone(), info.clone()))
        })
        .collect();
    let mut metadata = AlbumMetadata::default();
    // Keep resolved CUE values (including combined performers) before falling
    // back to the retained source candidates for formerly tag-owned fields.
    for (name, value, source) in album.metadata.entries() {
        if source != FieldSource::Tag {
            metadata.apply(name, &value, source);
        }
    }
    for candidate in &album.candidates {
        if candidate.source != FieldSource::Tag {
            metadata.apply(&candidate.name, &candidate.value, candidate.source);
        }
    }
    for (name, value, source) in
        resolve_metadata(&folder_name::parse(&album.folder_name), &[], &infos).entries()
    {
        metadata.apply(name, &value, source);
    }
    album.metadata = metadata;
    for disc in &mut album.discs {
        for track in &mut disc.tracks {
            let Some(info) = observations
                .get(track.source.audio_path())
                .and_then(TagObservation::info)
            else {
                continue;
            };
            track.properties = info.properties.clone();
            match track.source {
                TrackSource::File { .. } => {
                    if track
                        .title
                        .as_ref()
                        .is_some_and(|field| field.source == FieldSource::Tag)
                    {
                        track.title = None;
                    }
                    if track
                        .artist
                        .as_ref()
                        .is_some_and(|field| field.source == FieldSource::Tag)
                    {
                        track.artist = None;
                    }
                    merge_field(
                        &mut track.title,
                        info.title
                            .clone()
                            .map(|value| Field::new(value, FieldSource::Tag)),
                    );
                    merge_field(
                        &mut track.artist,
                        info.artist
                            .clone()
                            .map(|value| Field::new(value, FieldSource::Tag)),
                    );
                    if info.track.is_some() {
                        track.number = info.track;
                    }
                }
                TrackSource::Virtual { start, end, .. } => {
                    track.properties.duration_ms = match end {
                        Some(end) => Some(end.saturating_sub(start) as u64 * 1000 / 75),
                        None => info
                            .properties
                            .duration_ms
                            .map(|duration| duration.saturating_sub(start as u64 * 1000 / 75)),
                    };
                }
            }
        }
    }
    album
        .candidates
        .retain(|candidate| candidate.source != FieldSource::Tag);
    album.candidates.extend(candidates::collect(
        album,
        &FolderHints::default(),
        &[],
        observations,
    ));
    if album
        .cover
        .as_ref()
        .is_none_or(|cover| matches!(cover.source, CoverSource::Embedded(_)))
    {
        album.cover = infos
            .iter()
            .find(|(_, info)| info.has_picture)
            .map(|(path, _)| Cover {
                source: CoverSource::Embedded(path.clone()),
            });
    }
}
