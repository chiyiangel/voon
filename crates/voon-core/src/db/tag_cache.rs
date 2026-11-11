use super::{Index, Result, p};
use crate::scan::FileStamp;
use crate::tags::{SuccessfulTags, TagInfo, TagObservation, TagReadResult};
use rusqlite::{OptionalExtension, params};
use std::path::Path;

pub(super) const MIGRATION: &str = r#"
CREATE TABLE tag_observation (
    path TEXT PRIMARY KEY,
    size INTEGER NOT NULL,
    mtime INTEGER NOT NULL,
    status TEXT NOT NULL,
    reason TEXT,
    observed_at INTEGER NOT NULL,
    success_json TEXT,
    success_at INTEGER
);
"#;

pub(super) const READ_ORDER_MIGRATION: &str =
    "ALTER TABLE tag_observation ADD COLUMN read_started_at INTEGER NOT NULL DEFAULT 0;";

pub(super) fn next_read_order() -> i64 {
    use std::sync::atomic::{AtomicI64, Ordering};
    static LAST_START: AtomicI64 = AtomicI64::new(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX - 1))
        .unwrap_or(0);
    LAST_START.fetch_max(now, Ordering::SeqCst);
    LAST_START.fetch_add(1, Ordering::SeqCst).saturating_add(1)
}

impl Index {
    pub fn album_tag_observations(
        &self,
        album: &Path,
    ) -> Result<Vec<(std::path::PathBuf, TagObservation)>> {
        let Some(album) = self.album(album)? else {
            return Ok(vec![]);
        };
        let paths: std::collections::BTreeSet<_> = album
            .tracks()
            .map(|track| track.source.audio_path().clone())
            .collect();
        paths
            .into_iter()
            .filter_map(|path| match self.tag_observation(&path) {
                Ok(Some(observation)) => Some(Ok((path, observation))),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    }

    /// Current source attempt and its last successful data, including genuine read times.
    pub fn tag_observation(&self, path: &Path) -> Result<Option<TagObservation>> {
        self.read_observation(path, None)
    }

    pub(super) fn cached_observation(
        &self,
        path: &Path,
        stamp: FileStamp,
    ) -> Result<Option<TagObservation>> {
        self.read_observation(path, Some(stamp))
    }

    fn read_observation(
        &self,
        path: &Path,
        stamp: Option<FileStamp>,
    ) -> Result<Option<TagObservation>> {
        self.conn
            .query_row(
                "SELECT status,reason,observed_at,success_json,success_at FROM tag_observation
             WHERE path=?1 AND (?2 IS NULL OR (size=?2 AND mtime=?3))",
                params![
                    p(path),
                    stamp.map(|s| s.size as i64),
                    stamp.map(|s| s.mtime)
                ],
                |row| {
                    let status: String = row.get(0)?;
                    let reason: Option<String> = row.get(1)?;
                    let observed_at = row.get(2)?;
                    let info = row
                        .get::<_, Option<String>>(3)?
                        .map(|json| {
                            serde_json::from_str::<TagInfo>(&json).map_err(|error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    3,
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            })
                        })
                        .transpose()?;
                    let success_at: Option<i64> = row.get(4)?;
                    let reason = reason.unwrap_or_default();
                    let result = match status.as_str() {
                        "Tagged" => TagReadResult::Tagged(info.clone().unwrap_or_default()),
                        "NoTags" => TagReadResult::NoTags(info.clone().unwrap_or_default()),
                        "Unsupported" => TagReadResult::Unsupported(reason),
                        "IoFailure" => TagReadResult::IoFailure(reason),
                        _ => TagReadResult::ParseFailure(reason),
                    };
                    let last_success = result
                        .reason()
                        .is_some()
                        .then(|| {
                            Some(SuccessfulTags {
                                info: info?,
                                observed_at: success_at?,
                            })
                        })
                        .flatten();
                    Ok(TagObservation {
                        result,
                        observed_at,
                        last_success,
                    })
                },
            )
            .optional()
    }

    pub(super) fn cache_observation(
        &self,
        path: &Path,
        stamp: FileStamp,
        observation: &TagObservation,
        read_started_at: i64,
    ) -> Result<bool> {
        let json = observation
            .info()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        let changed = self.conn.execute(
            "INSERT INTO tag_observation(path,size,mtime,status,reason,observed_at,success_json,success_at,read_started_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)
             ON CONFLICT(path) DO UPDATE SET size=excluded.size,mtime=excluded.mtime,
             status=excluded.status,reason=excluded.reason,observed_at=excluded.observed_at,
             success_json=excluded.success_json,success_at=excluded.success_at,
             read_started_at=excluded.read_started_at
             WHERE excluded.read_started_at >= tag_observation.read_started_at",
            params![p(path), stamp.size as i64, stamp.mtime, observation.result.status(),
                observation.result.reason(), observation.observed_at, json, observation.data_observed_at(), read_started_at],
        )?;
        Ok(changed != 0)
    }
}
