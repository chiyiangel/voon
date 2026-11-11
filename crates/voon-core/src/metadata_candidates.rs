//! Source observations accompany the existing deterministic metadata resolver.
use crate::{Field, FieldSource, TrackSource};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SourceStatus {
    Current,
    Unconfirmed {
        reason: String,
        attempted_at: Option<i64>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MetadataCandidate {
    pub name: String,
    pub value: String,
    pub source: FieldSource,
    pub source_path: PathBuf,
    /// Exact whole-file or CUE-span identities to which this source applies.
    pub tracks: Vec<TrackSource>,
    pub observed_at: i64,
    pub status: SourceStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldComparison {
    pub name: String,
    pub effective: Option<Field<String>>,
    pub automatic: Option<Field<String>>,
    pub user_value: Option<String>,
    pub candidates: Vec<MetadataCandidate>,
}

pub(crate) fn for_album(album: &crate::Album) -> Vec<MetadataCandidate> {
    let mut candidates: Vec<MetadataCandidate> = vec![];
    for original in &album.candidates {
        let mut candidate = original.clone();
        candidate
            .tracks
            .retain(|source| album.tracks().any(|track| track.source == *source));
        if candidate.tracks.is_empty() {
            continue;
        }
        if let Some(previous) = candidates.iter_mut().find(|previous| {
            previous.name == candidate.name
                && previous.value == candidate.value
                && previous.source == candidate.source
                && previous.source_path == candidate.source_path
                && previous.observed_at == candidate.observed_at
                && previous.status == candidate.status
        }) {
            for track in candidate.tracks {
                if !previous.tracks.contains(&track) {
                    previous.tracks.push(track);
                }
            }
        } else {
            candidates.push(candidate);
        }
    }
    candidates
}
