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
        let mut candidates = crate::metadata_candidates::for_album(&album);
        let issues = self.album_scan_issues(path)?;
        for candidate in &mut candidates {
            if let Some(issue) = issues.iter().find(|issue| {
                candidate.source_path.starts_with(&issue.path)
                    || issue.path.starts_with(&candidate.source_path)
            }) {
                candidate.status = crate::SourceStatus::Unconfirmed {
                    reason: issue.explanation.clone(),
                    attempted_at: None,
                };
            }
        }
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
                let candidates = candidates
                    .iter()
                    .filter(|candidate| candidate.name == *name)
                    .cloned()
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
