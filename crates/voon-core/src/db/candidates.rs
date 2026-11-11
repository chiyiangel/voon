use super::{Index, Result};
use crate::{ALBUM_FIELDS, Field, FieldComparison, FieldSource};
use std::path::Path;

pub(super) const MIGRATION: &str =
    "ALTER TABLE album ADD COLUMN candidates_json TEXT NOT NULL DEFAULT '[]';";

impl Index {
    pub fn metadata_comparison(&self, path: &Path) -> Result<Vec<FieldComparison>> {
        let Some(album) = self.album(path)? else {
            return Ok(vec![]);
        };
        let automatic = self.automatic_metadata(path)?.unwrap_or_default();
        let effective = album.metadata.entries();
        let automatic = automatic.entries();
        Ok(ALBUM_FIELDS
            .iter()
            .map(|name| {
                let effective = effective
                    .iter()
                    .find(|(field, _, _)| field == name)
                    .map(|(_, value, source)| Field::new(value.clone(), *source));
                let automatic = automatic
                    .iter()
                    .find(|(field, _, _)| field == name)
                    .map(|(_, value, source)| Field::new(value.clone(), *source));
                let user_value = effective
                    .as_ref()
                    .filter(|field| field.source == FieldSource::UserEdit)
                    .map(|field| field.value.clone());
                let candidates = album
                    .candidates
                    .iter()
                    .filter(|candidate| candidate.name == *name)
                    .filter_map(|candidate| {
                        let mut candidate = candidate.clone();
                        candidate
                            .tracks
                            .retain(|source| album.tracks().any(|track| track.source == *source));
                        (!candidate.tracks.is_empty()).then_some(candidate)
                    })
                    .collect();
                FieldComparison {
                    name: (*name).into(),
                    effective,
                    automatic,
                    user_value,
                    candidates,
                }
            })
            .collect())
    }
}
