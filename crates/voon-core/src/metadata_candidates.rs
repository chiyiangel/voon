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
        candidate.tracks.clear();
        for scope in &original.tracks {
            for track in album.tracks() {
                if let Some(intersection) = intersect_scope(scope, &track.source) {
                    append_scope(&mut candidate.tracks, intersection);
                }
            }
        }
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
                append_scope(&mut previous.tracks, track);
            }
        } else {
            candidates.push(candidate);
        }
    }
    candidates
}

fn intersect_scope(source: &TrackSource, displayed: &TrackSource) -> Option<TrackSource> {
    if source.audio_path() != displayed.audio_path() {
        return None;
    }
    match (source, displayed) {
        (TrackSource::File { .. }, _) => Some(displayed.clone()),
        (_, TrackSource::File { .. }) => Some(source.clone()),
        (
            TrackSource::Virtual {
                image,
                cue,
                start,
                end,
            },
            TrackSource::Virtual {
                start: other_start,
                end: other_end,
                ..
            },
        ) => {
            let start = (*start).max(*other_start);
            let end = match (end, other_end) {
                (Some(left), Some(right)) => Some((*left).min(*right)),
                (left, right) => left.or(*right),
            };
            if end.is_some_and(|end| end <= start) {
                return None;
            }
            Some(TrackSource::Virtual {
                image: image.clone(),
                cue: cue.clone(),
                start,
                end,
            })
        }
    }
}

fn append_scope(scopes: &mut Vec<TrackSource>, mut scope: TrackSource) {
    while let Some((position, merged)) = scopes
        .iter()
        .enumerate()
        .find_map(|(position, old)| merge_spans(old, &scope).map(|merged| (position, merged)))
    {
        scopes.remove(position);
        scope = merged;
    }
    if !scopes.contains(&scope) {
        scopes.push(scope);
    }
}

fn merge_spans(left: &TrackSource, right: &TrackSource) -> Option<TrackSource> {
    let (
        TrackSource::Virtual {
            image,
            cue,
            start,
            end,
        },
        TrackSource::Virtual {
            image: other_image,
            cue: other_cue,
            start: other_start,
            end: other_end,
        },
    ) = (left, right)
    else {
        return None;
    };
    if image != other_image
        || cue != other_cue
        || (*start).max(*other_start) > end.unwrap_or(u32::MAX).min(other_end.unwrap_or(u32::MAX))
    {
        return None;
    }
    Some(TrackSource::Virtual {
        image: image.clone(),
        cue: cue.clone(),
        start: (*start).min(*other_start),
        end: match (end, other_end) {
            (Some(left), Some(right)) => Some((*left).max(*right)),
            _ => None,
        },
    })
}
