//! Read-only cover discovery. File access belongs on a background worker.
use crate::{
    Album, Cover, CoverSource,
    scan::{IMAGE_EXTENSIONS, is_attachment_dir, is_junk},
};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverCandidate {
    pub cover: Cover,
    pub label: String,
}

pub fn candidates(album: &Album, ignored: &[PathBuf]) -> std::io::Result<Vec<CoverCandidate>> {
    candidates_with_cancel(album, ignored, &AtomicBool::new(false))
}

/// Discover image files in the Album/Disc directories and attachment subtrees.
/// Child symbolic links are never followed. Cancellation returns the files
/// already found without continuing into the remaining directories.
pub fn discover_images(
    album: &Album,
    ignored: &[PathBuf],
    cancel: &AtomicBool,
) -> std::io::Result<Vec<PathBuf>> {
    let mut directories: Vec<_> = album
        .physical_directories()
        .into_iter()
        .map(|path| (path.to_path_buf(), false))
        .collect();
    directories.extend(album.discs.iter().map(|disc| (disc.path.clone(), false)));
    let mut visited = BTreeSet::new();
    let mut files = BTreeSet::new();
    while let Some((directory, recursive)) = directories.pop() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if ignored.iter().any(|ignored| directory.starts_with(ignored))
            || !visited.insert(directory.clone())
        {
            continue;
        }
        for entry in std::fs::read_dir(directory)? {
            if cancel.load(Ordering::Relaxed) {
                return Ok(files.into_iter().collect());
            }
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_junk(&name) || ignored.iter().any(|ignored| path.starts_with(ignored)) {
                continue;
            }
            let kind = entry.file_type()?;
            if kind.is_dir() && (recursive || is_attachment_dir(&name)) {
                directories.push((path, true));
            } else if kind.is_file()
                && path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| {
                        IMAGE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
                    })
            {
                files.insert(path);
            }
        }
    }
    Ok(files.into_iter().collect())
}

pub fn candidates_with_cancel(
    album: &Album,
    ignored: &[PathBuf],
    cancel: &AtomicBool,
) -> std::io::Result<Vec<CoverCandidate>> {
    let files = discover_images(album, ignored, cancel)?;
    if cancel.load(Ordering::Relaxed) {
        return Ok(vec![]);
    }
    let mut result: Vec<_> = files
        .into_iter()
        .map(|path| CoverCandidate {
            label: path
                .strip_prefix(&album.path)
                .unwrap_or(&path)
                .display()
                .to_string(),
            cover: Cover {
                source: CoverSource::File(path),
            },
        })
        .collect();
    let audio: BTreeSet<_> = album
        .tracks()
        .map(|track| track.source.audio_path().clone())
        .collect();
    for path in audio {
        if cancel.load(Ordering::Relaxed) {
            return Ok(vec![]);
        }
        if ignored.iter().any(|ignored| path.starts_with(ignored)) {
            continue;
        }
        // A broken/untagged audio file must not hide the other candidates.
        let count = crate::tags::embedded_cover_count(&path).unwrap_or(0);
        for index in 0..count {
            result.push(CoverCandidate {
                label: format!(
                    "{} · 内嵌封面 {}",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    index + 1
                ),
                cover: Cover {
                    source: CoverSource::EmbeddedPicture {
                        path: path.clone(),
                        index: index as u32,
                    },
                },
            });
        }
    }
    Ok(result)
}
