//! Source observations accompany the existing deterministic metadata resolver.
use crate::{Field, FieldSource, TrackSource};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SourceStatus {
    Current,
    Unconfirmed { reason: String, attempted_at: i64 },
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
