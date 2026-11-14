//! Commit file-level metadata recovery without treating it as a directory scan.
use super::*;
use crate::scan::{refresh_album_tags, regroup_cached_loose};
use std::collections::BTreeMap;

impl Index {
    /// Cached physical structure, before user merge/split operations.
    pub fn scan_albums(&self, root: &Path) -> Result<Vec<Album>> {
        use rusqlite::OptionalExtension;
        let id = self
            .conn
            .query_row(
                "SELECT id FROM library_root WHERE path=?1",
                [p(root)],
                |row| row.get(0),
            )
            .optional()?;
        match id {
            Some(id) => structure::physical_albums(&self.conn, id),
            None => Ok(vec![]),
        }
    }

    /// Update only metadata facts for attempted files. In particular this must
    /// not clear CUE/directory failures or replace a whole Album's issue scope.
    pub fn store_tag_retry(&mut self, root: &Path, scan: &ScanResult) -> Result<()> {
        if scan.cancelled || scan.tag_observations.is_empty() {
            return Ok(());
        }
        let root_id = self.add_root(root)?;
        let previous = structure::physical_albums(&self.conn, root_id)?;
        let observations = &scan.tag_observations;
        let mut updated = ScanResult::default();
        for mut album in previous
            .iter()
            .filter(|album| {
                !album.path.starts_with(root.join(LOOSE_DIR))
                    && album
                        .tracks()
                        .any(|track| observations.contains_key(track.source.audio_path()))
            })
            .cloned()
        {
            let mut sources = BTreeMap::new();
            for path in album.tracks().map(|track| track.source.audio_path()) {
                if let Some(observation) = observations
                    .get(path)
                    .cloned()
                    .or(self.tag_observation(path)?)
                {
                    sources.insert(path.clone(), observation);
                }
            }
            refresh_album_tags(&mut album, &sources);
            updated.albums.push(album);
        }
        let loose = observations.keys().any(|path| path.parent() == Some(root));
        let loose_paths: std::collections::BTreeSet<_> = previous
            .iter()
            .filter(|album| album.path.starts_with(root.join(LOOSE_DIR)))
            .flat_map(|album| {
                album
                    .tracks()
                    .map(|track| track.source.audio_path().clone())
            })
            .chain(
                observations
                    .keys()
                    .filter(|path| path.parent() == Some(root))
                    .cloned(),
            )
            .collect();
        if loose {
            // Include known ungrouped failures too; a successful retry can
            // finally supply their album tag. No source directory is listed.
            let mut statement = self
                .conn
                .prepare("SELECT path,size,mtime FROM tag_observation")?;
            let rows = statement.query_map([], |row| {
                Ok((
                    PathBuf::from(row.get::<_, String>(0)?),
                    FileStamp {
                        size: row.get::<_, i64>(1)? as u64,
                        mtime: row.get(2)?,
                    },
                ))
            })?;
            let mut stamps = BTreeMap::new();
            let mut sources = BTreeMap::new();
            for row in rows {
                let (path, stamp) = row?;
                if !loose_paths.contains(&path) {
                    continue;
                }
                if let Some(observation) = observations
                    .get(&path)
                    .cloned()
                    .or(self.tag_observation(&path)?)
                {
                    sources.insert(path.clone(), observation);
                    stamps.insert(path, stamp);
                }
            }
            for (path, stamp) in &scan.stamps {
                if path.parent() == Some(root) {
                    stamps.insert(path.clone(), *stamp);
                    if let Some(observation) = observations.get(path) {
                        sources.insert(path.clone(), observation.clone());
                    }
                }
            }
            let regrouped = regroup_cached_loose(root, stamps, &sources);
            updated.albums.extend(regrouped.albums);
            updated
                .issues
                .extend(regrouped.issues.into_iter().filter(|issue| {
                    matches!(issue.kind, crate::scan::IssueKind::LooseUntagged { .. })
                }));
        }
        let tx = self.conn.transaction()?;
        if loose {
            for album in &previous {
                if album.path.starts_with(root.join(LOOSE_DIR))
                    && !updated.albums.iter().any(|new| new.path == album.path)
                {
                    tx.execute(
                        "DELETE FROM album WHERE root_id=?1 AND path=?2",
                        params![root_id, p(&album.path)],
                    )?;
                    tx.execute(
                        "DELETE FROM structure_raw WHERE root_id=?1 AND path=?2",
                        params![root_id, p(&album.path)],
                    )?;
                }
            }
            for path in &loose_paths {
                let (_, detail) = attention::describe(&crate::scan::ScanIssue {
                    path: root.into(),
                    kind: crate::scan::IssueKind::LooseUntagged { file: path.clone() },
                });
                tx.execute("DELETE FROM scan_issue WHERE root_id=?1 AND kind='LooseUntagged' AND detail=?2", params![root_id, detail])?;
            }
        }
        for (path, observation) in observations {
            tx.execute("DELETE FROM scan_issue WHERE root_id=?1 AND path=?2 AND kind IN ('TagIoFailure','TagParseFailure','TagUnsupported')", params![root_id, p(path)])?;
            if let Some((failure, reason)) = observation.result.failure() {
                updated.issues.push(crate::scan::ScanIssue {
                    path: path.clone(),
                    kind: crate::scan::IssueKind::TagRead {
                        failure,
                        reason: reason.into(),
                        retained: observation.last_success.is_some(),
                    },
                });
            }
        }
        browse::record_album_added(&tx, root, &updated.albums)?;
        for album in &updated.albums {
            details::store_album(&tx, root_id, album)?;
        }
        for issue in &updated.issues {
            let (kind, detail) = attention::describe(issue);
            tx.execute(
                "INSERT INTO scan_issue(root_id,path,kind,detail) VALUES (?1,?2,?3,?4)",
                params![root_id, p(&issue.path), kind, detail],
            )?;
        }
        structure::record_scan_and_replay(&tx, root_id, root, &updated, false)?;
        tx.commit()
    }
}
