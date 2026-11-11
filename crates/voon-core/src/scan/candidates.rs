use super::ObservedCue;
use crate::{
    Album, FieldSource, MetadataCandidate, SourceStatus, folder_name::FolderHints,
    tags::TagObservation,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub(super) fn collect(
    album: &Album,
    hints: &FolderHints,
    sheets: &[ObservedCue],
    observations: &BTreeMap<PathBuf, TagObservation>,
) -> Vec<MetadataCandidate> {
    let mut candidates = vec![];
    let tracks: Vec<_> = album.tracks().map(|track| track.source.clone()).collect();
    let mut add = |name: &str,
                   value: Option<String>,
                   source,
                   path: &Path,
                   scope: Vec<crate::TrackSource>,
                   observed_at,
                   status: SourceStatus| {
        if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
            candidates.push(MetadataCandidate {
                name: name.into(),
                value,
                source,
                source_path: path.into(),
                tracks: scope,
                observed_at,
                status,
            });
        }
    };
    let now = crate::tags::observation_time();
    add(
        "title",
        hints.title.clone(),
        FieldSource::FolderName,
        &album.path,
        tracks.clone(),
        now,
        SourceStatus::Current,
    );
    add(
        "year",
        hints.year.map(|year| year.to_string()),
        FieldSource::FolderName,
        &album.path,
        tracks.clone(),
        now,
        SourceStatus::Current,
    );
    for observation in sheets {
        let scope: Vec<_> = tracks
            .iter()
            .filter(|track| observation.audio_paths.contains(track.audio_path()))
            .cloned()
            .collect();
        add(
            "title",
            observation.sheet.title.clone(),
            FieldSource::Cue,
            &observation.path,
            scope.clone(),
            observation.observed_at,
            SourceStatus::Current,
        );
        add(
            "year",
            observation
                .sheet
                .date
                .as_deref()
                .and_then(|date| date.get(..4))
                .and_then(|year| year.parse::<u16>().ok())
                .map(|year| year.to_string()),
            FieldSource::Cue,
            &observation.path,
            scope,
            observation.observed_at,
            SourceStatus::Current,
        );
    }
    for (path, observation) in observations {
        let scope: Vec<_> = tracks
            .iter()
            .filter(|track| track.audio_path() == path)
            .cloned()
            .collect();
        if scope.is_empty() {
            continue;
        }
        let Some(info) = observation.info() else {
            continue;
        };
        let Some(observed_at) = observation.data_observed_at() else {
            continue;
        };
        let status = observation
            .result
            .reason()
            .map(|reason| SourceStatus::Unconfirmed {
                reason: reason.into(),
                attempted_at: observation.observed_at,
            })
            .unwrap_or(SourceStatus::Current);
        add(
            "title",
            info.album.clone(),
            FieldSource::Tag,
            path,
            scope.clone(),
            observed_at,
            status.clone(),
        );
        add(
            "year",
            info.year.map(|year| year.to_string()),
            FieldSource::Tag,
            path,
            scope,
            observed_at,
            status,
        );
    }
    candidates
}
