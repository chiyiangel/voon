//! Reproducible read-only acceptance run against an explicitly selected Library root.
//! Usage: cargo run -p voon-core --example reliability_scan -- INDEX ROOT REPORT.json
use std::{
    error::Error,
    path::{Path, PathBuf},
};
use voon_core::{
    db::{CachedTags, Index},
    scan::{DiskTags, scan_root},
};

fn local_output(path: &Path, root: &Path) -> Result<(), Box<dyn Error>> {
    let parent = path
        .parent()
        .ok_or("output needs a parent directory")?
        .canonicalize()?;
    if parent.starts_with(root) || path.canonicalize().is_ok_and(|path| path.starts_with(root)) {
        return Err("acceptance output must be outside the music library".into());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let [database, root, report] = args.as_slice() else {
        return Err("usage: reliability_scan INDEX ROOT REPORT.json".into());
    };
    let root = root.canonicalize()?;
    local_output(database, &root)?;
    local_output(report, &root)?;
    let mut index = Index::open(database)?;
    let (scan, reads) = {
        let mut tags = CachedTags::new(&index, DiskTags);
        let scan = scan_root(&root, &mut tags);
        (scan, tags.reads)
    };
    index.store_scan(&root, &scan)?;
    let albums = index
        .albums()?
        .iter()
        .map(|row| index.album(&row.path))
        .collect::<voon_core::db::Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let observations = scan.tag_observations.iter().map(|(path, observation)| serde_json::json!({
        "path": path, "status": observation.result.status(), "reason": observation.result.reason(),
        "observed_at": observation.observed_at, "data_observed_at": observation.data_observed_at(),
        "retained_success": observation.last_success.is_some(),
    })).collect::<Vec<_>>();
    let attention = index
        .attention_items()?
        .iter()
        .map(|item| {
            serde_json::json!({
                "path": item.path, "kind": item.kind, "explanation": item.explanation,
            })
        })
        .collect::<Vec<_>>();
    let snapshot = serde_json::json!({
        "root": root, "schema_version": index.schema_version()?, "actual_tag_reads": reads,
        "albums": albums, "observations": observations, "attention": attention,
        "cancelled": scan.cancelled, "incomplete_paths": scan.incomplete_paths(),
    });
    std::fs::write(report, serde_json::to_vec_pretty(&snapshot)?)?;
    println!(
        "albums={} tracks={} actual_tag_reads={} issues={}",
        albums.len(),
        albums
            .iter()
            .map(|album| album.tracks().count())
            .sum::<usize>(),
        reads,
        attention.len()
    );
    Ok(())
}
