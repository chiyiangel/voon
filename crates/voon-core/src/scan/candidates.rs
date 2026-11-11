use super::ObservedCue;
use crate::{
    Album, FieldSource, MetadataCandidate, SourceStatus, TrackSource, folder_name::FolderHints,
    tags::TagObservation,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone)]
struct CandidateSource<'a> {
    source: FieldSource,
    path: &'a Path,
    tracks: Vec<TrackSource>,
    observed_at: i64,
    status: SourceStatus,
}

impl CandidateSource<'_> {
    fn append<'a>(
        &self,
        candidates: &mut Vec<MetadataCandidate>,
        fields: impl IntoIterator<Item = (&'a str, Option<String>)>,
    ) {
        for (name, value) in fields {
            if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
                candidates.push(MetadataCandidate {
                    name: name.into(),
                    value,
                    source: self.source,
                    source_path: self.path.into(),
                    tracks: self.tracks.clone(),
                    observed_at: self.observed_at,
                    status: self.status.clone(),
                });
            }
        }
    }
}

pub(super) fn collect(
    album: &Album,
    hints: &FolderHints,
    sheets: &[ObservedCue],
    observations: &BTreeMap<PathBuf, TagObservation>,
) -> Vec<MetadataCandidate> {
    let mut candidates = vec![];
    let tracks: Vec<_> = album.tracks().map(|track| track.source.clone()).collect();
    CandidateSource {
        source: FieldSource::FolderName,
        path: &album.path,
        tracks: tracks.clone(),
        observed_at: crate::tags::observation_time(),
        status: SourceStatus::Current,
    }
    .append(
        &mut candidates,
        [
            ("title", hints.title.clone()),
            ("album_artist", hints.artist.clone()),
            ("year", hints.year.map(|year| year.to_string())),
            ("catalog_number", hints.catalog_number.clone()),
            ("edition", hints.edition.clone()),
        ],
    );
    for observation in sheets {
        let sheet = &observation.sheet;
        let source = CandidateSource {
            source: FieldSource::Cue,
            path: &observation.path,
            tracks: tracks
                .iter()
                .filter(|track| observation.audio_paths.contains(track.audio_path()))
                .cloned()
                .collect(),
            observed_at: observation.observed_at,
            status: SourceStatus::Current,
        };
        source.append(
            &mut candidates,
            [
                ("title", sheet.title.clone()),
                ("album_artist", sheet.performer.clone()),
                (
                    "year",
                    sheet
                        .date
                        .as_deref()
                        .and_then(|date| date.get(..4))
                        .and_then(|year| year.parse::<u16>().ok())
                        .map(|year| year.to_string()),
                ),
                (
                    "composer",
                    sheet.composer.clone().or_else(|| sheet.songwriter.clone()),
                ),
                ("catalog_number", sheet.catalog.clone()),
            ],
        );
        for (file, audio) in sheet.files.iter().zip(&observation.audio_paths) {
            for cue_track in &file.tracks {
                let scope = source
                    .tracks
                    .iter()
                    .filter(|track| {
                        track.audio_path() == audio
                            && match track {
                                TrackSource::Virtual { start, .. } => {
                                    *start == cue_track.start.unwrap_or(0)
                                }
                                TrackSource::File { .. } => true,
                            }
                    })
                    .cloned()
                    .collect();
                CandidateSource {
                    tracks: scope,
                    ..source.clone()
                }
                .append(
                    &mut candidates,
                    [(
                        "performers",
                        cue_track
                            .performer
                            .clone()
                            .or_else(|| sheet.performer.clone()),
                    )],
                );
            }
        }
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
                attempted_at: Some(observation.observed_at),
            })
            .unwrap_or(SourceStatus::Current);
        CandidateSource {
            source: FieldSource::Tag,
            path,
            tracks: scope,
            observed_at,
            status,
        }
        .append(
            &mut candidates,
            [
                ("title", info.album.clone()),
                (
                    "album_artist",
                    info.album_artist.clone().or_else(|| info.artist.clone()),
                ),
                ("year", info.year.map(|year| year.to_string())),
                ("genre", info.genre.clone()),
                ("catalog_number", info.catalog_number.clone()),
                ("composer", info.composer.clone()),
                ("performers", info.artist.clone()),
                ("conductor", info.conductor.clone()),
            ],
        );
    }
    candidates
}
