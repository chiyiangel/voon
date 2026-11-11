//! Local structural choices are replayed over raw scans; music files stay untouched.
use super::{Index, Result, covers, details, p};
use crate::{
    ALBUM_FIELDS, Album, AlbumMetadata, Cover, Disc, FieldSource, ScanResult, Track, TrackSource,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    path::{Path, PathBuf},
};

pub(super) const MIGRATION: &str = r#"
CREATE TABLE structure_raw (
    path TEXT PRIMARY KEY,
    root_id INTEGER NOT NULL REFERENCES library_root(id) ON DELETE CASCADE,
    album TEXT NOT NULL
);
CREATE TABLE structure_operation (
    id INTEGER PRIMARY KEY,
    root_id INTEGER NOT NULL REFERENCES library_root(id) ON DELETE CASCADE,
    plan TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT 1
);
"#;

pub(super) const PERSIST_ROOT_MIGRATION: &str = r#"
CREATE TABLE structure_operation_saved (
    id INTEGER PRIMARY KEY,
    root_path TEXT NOT NULL,
    plan TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT 1
);
INSERT INTO structure_operation_saved(id,root_path,plan,active)
SELECT id, json_extract(plan,'$.root'), plan, active FROM structure_operation;
DROP TABLE structure_operation;
ALTER TABLE structure_operation_saved RENAME TO structure_operation;
"#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscRef {
    pub album: PathBuf,
    pub disc: usize,
}

#[derive(Clone, Debug)]
pub enum SplitMode {
    Discs,
    SelectedTracks(Vec<TrackRef>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TrackRef {
    pub disc: usize,
    pub track: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructureOperation {
    pub id: i64,
    pub kind: String,
    pub inputs: Vec<PathBuf>,
    pub outputs: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
enum OperationKind {
    #[default]
    Merge,
    SplitDiscs,
    SplitTracks,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RawAlbum {
    root_id: i64,
    album: Album,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct DiscKey {
    album: PathBuf,
    path: PathBuf,
    #[serde(default)]
    anchor: Option<TrackKey>,
    #[serde(default)]
    anchors: Vec<TrackKey>,
    #[serde(default)]
    disambiguate: bool,
    /// Logical pieces of the same image/CUE need their frame positions.
    /// Physical Discs retain their directory or audio/CUE identity instead.
    #[serde(default)]
    track_subset: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct TrackKey {
    disc: PathBuf,
    audio: PathBuf,
    cue: Option<PathBuf>,
    start: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct UserData {
    edits: BTreeMap<String, String>,
    tags: BTreeSet<i64>,
    collections: BTreeMap<i64, i64>,
    cover: Option<Cover>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Operation {
    #[serde(default)]
    kind: OperationKind,
    root: PathBuf,
    root_id: i64,
    #[serde(default)]
    roots: Vec<PathBuf>,
    inputs: Vec<PathBuf>,
    primary: PathBuf,
    disc_order: Vec<DiscKey>,
    #[serde(default)]
    selected: Vec<TrackKey>,
    outputs: Vec<PathBuf>,
    fallback: Vec<RawAlbum>,
    initial: BTreeMap<PathBuf, UserData>,
}

impl Index {
    pub fn last_structure_operation(&self) -> Result<Option<StructureOperation>> {
        let found = self
            .conn
            .query_row(
                "SELECT id,plan FROM structure_operation WHERE active=1 ORDER BY id DESC LIMIT 1",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        found
            .map(|(id, json)| {
                let operation: Operation = decode(&json)?;
                let kind = match operation.kind {
                    OperationKind::Merge => "合并",
                    OperationKind::SplitDiscs => "按碟拆分",
                    OperationKind::SplitTracks => "按曲目拆分",
                };
                Ok(StructureOperation {
                    id,
                    kind: kind.into(),
                    inputs: operation.inputs,
                    outputs: operation.outputs,
                })
            })
            .transpose()
    }

    pub fn has_structure_operations(&self, root: &Path) -> Result<bool> {
        let mut statement = self
            .conn
            .prepare("SELECT plan FROM structure_operation WHERE active=1")?;
        for json in statement.query_map([], |row| row.get::<_, String>(0))? {
            let operation: Operation = decode(&json?)?;
            if operation.root == root || operation.roots.iter().any(|path| path == root) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Albums are local index records. Disc order must name every input Disc
    /// exactly once; an empty order uses each input's current Disc order.
    pub fn merge_albums(
        &mut self,
        inputs: &[PathBuf],
        primary: &Path,
        order: &[DiscRef],
    ) -> Result<PathBuf> {
        if inputs.len() < 2
            || inputs.iter().collect::<HashSet<_>>().len() != inputs.len()
            || !inputs.iter().any(|path| path == primary)
        {
            return Err(invalid(
                "请选择至少两张不同专辑，并指定其中一张作为主信息源",
            ));
        }
        let tx = self.conn.transaction()?;
        ensure_renames_decided(&tx, inputs.iter().map(PathBuf::as_path))?;
        bootstrap_raw(&tx)?;
        let mut sources = Vec::new();
        for path in inputs {
            let album = details::stored_album(&tx, path, false)?
                .ok_or_else(|| invalid("所选专辑已不存在，请重新选择"))?;
            let root_id = tx.query_row(
                "SELECT root_id FROM album WHERE path=?1",
                [p(path)],
                |row| row.get(0),
            )?;
            sources.push(RawAlbum { root_id, album });
        }
        let primary_source = sources
            .iter()
            .find(|source| source.album.path == primary)
            .unwrap();
        let root_id = primary_source.root_id;
        let root: PathBuf = tx
            .query_row(
                "SELECT path FROM library_root WHERE id=?1",
                [root_id],
                |row| row.get::<_, String>(0),
            )?
            .into();
        let default_order: Vec<_> = sources
            .iter()
            .flat_map(|source| {
                source
                    .album
                    .discs
                    .iter()
                    .map(|disc| disc_key(&source.album, disc))
            })
            .collect();
        let disc_order = if order.is_empty() {
            default_order.clone()
        } else {
            order
                .iter()
                .map(|reference| {
                    let source = sources
                        .iter()
                        .find(|source| source.album.path == reference.album)
                        .ok_or_else(|| invalid("碟片不属于所选专辑"))?;
                    let disc = source
                        .album
                        .discs
                        .get(reference.disc)
                        .ok_or_else(|| invalid("碟片顺序已变化，请重新选择"))?;
                    Ok(disc_key(&source.album, disc))
                })
                .collect::<Result<Vec<_>>>()?
        };
        let proposed: HashSet<_> = disc_order.iter().cloned().collect();
        if proposed.len() != disc_order.len() || proposed != default_order.into_iter().collect() {
            return Err(invalid("碟片顺序必须包含每张碟，且不能重复"));
        }
        let mut roots = BTreeSet::new();
        for source in &sources {
            let source_root: String = tx.query_row(
                "SELECT path FROM library_root WHERE id=?1",
                [source.root_id],
                |row| row.get(0),
            )?;
            roots.insert(PathBuf::from(source_root));
        }
        let mut previous = tx.prepare("SELECT plan FROM structure_operation WHERE active=1")?;
        for json in previous.query_map([], |row| row.get::<_, String>(0))? {
            let previous: Operation = decode(&json?)?;
            if previous.outputs.iter().any(|path| inputs.contains(path)) {
                roots.extend(
                    previous
                        .roots
                        .into_iter()
                        .chain(std::iter::once(previous.root)),
                );
            }
        }
        drop(previous);
        tx.execute(
            "INSERT INTO structure_operation(root_path,plan) VALUES (?1,'')",
            [p(&root)],
        )?;
        let id = tx.last_insert_rowid();
        let output = root
            .join(".voon-structure")
            .join(id.to_string())
            .join("merged");
        let data = merged_user_data(&tx, &sources, primary)?;
        write_user_data(&tx, &output, &data)?;
        let operation = Operation {
            kind: OperationKind::Merge,
            root,
            root_id,
            roots: roots.into_iter().collect(),
            inputs: inputs.to_vec(),
            primary: primary.to_path_buf(),
            disc_order,
            selected: Vec::new(),
            outputs: vec![output.clone()],
            fallback: sources,
            initial: BTreeMap::from([(output.clone(), data)]),
        };
        tx.execute(
            "UPDATE structure_operation SET plan=?2 WHERE id=?1",
            params![id, encode(&operation)?],
        )?;
        replay(&tx)?;
        tx.commit()?;
        Ok(output)
    }

    pub fn split_album(&mut self, input: &Path, mode: SplitMode) -> Result<Vec<PathBuf>> {
        let tx = self.conn.transaction()?;
        ensure_renames_decided(&tx, std::iter::once(input))?;
        bootstrap_raw(&tx)?;
        let album = details::stored_album(&tx, input, false)?
            .ok_or_else(|| invalid("所选专辑已不存在，请重新选择"))?;
        let (kind, selected) = match mode {
            SplitMode::Discs => {
                if album.discs.len() < 2 {
                    return Err(invalid("此专辑不足两张碟，请选择按曲目拆分"));
                }
                (OperationKind::SplitDiscs, Vec::new())
            }
            SplitMode::SelectedTracks(references) => {
                if references.is_empty()
                    || references.len() >= album.tracks().count()
                    || references.iter().collect::<BTreeSet<_>>().len() != references.len()
                {
                    return Err(invalid("请选择部分曲目，并至少留下一首给另一张专辑"));
                }
                let selected = references
                    .iter()
                    .map(|reference| {
                        let disc = album
                            .discs
                            .get(reference.disc)
                            .ok_or_else(|| invalid("碟片已变化，请重新选择"))?;
                        let track = disc
                            .tracks
                            .get(reference.track)
                            .ok_or_else(|| invalid("曲目已变化，请重新选择"))?;
                        Ok(track_key(&disc.path, track))
                    })
                    .collect::<Result<Vec<_>>>()?;
                (OperationKind::SplitTracks, selected)
            }
        };
        let root_id = tx.query_row(
            "SELECT root_id FROM album WHERE path=?1",
            [p(input)],
            |row| row.get(0),
        )?;
        let root: PathBuf = tx
            .query_row(
                "SELECT path FROM library_root WHERE id=?1",
                [root_id],
                |row| row.get::<_, String>(0),
            )?
            .into();
        let mut roots = BTreeSet::from([root.clone()]);
        let mut previous = tx.prepare("SELECT plan FROM structure_operation WHERE active=1")?;
        for json in previous.query_map([], |row| row.get::<_, String>(0))? {
            let previous: Operation = decode(&json?)?;
            if previous.outputs.iter().any(|path| path == input) {
                roots.extend(
                    previous
                        .roots
                        .into_iter()
                        .chain(std::iter::once(previous.root)),
                );
            }
        }
        drop(previous);
        tx.execute(
            "INSERT INTO structure_operation(root_path,plan) VALUES (?1,'')",
            [p(&root)],
        )?;
        let id = tx.last_insert_rowid();
        let output_count = if matches!(kind, OperationKind::SplitDiscs) {
            album.discs.len()
        } else {
            2
        };
        let outputs = (0..output_count)
            .map(|position| {
                root.join(".voon-structure")
                    .join(id.to_string())
                    .join(format!("part-{}", position + 1))
            })
            .collect::<Vec<_>>();
        let disc_order = album
            .discs
            .iter()
            .map(|disc| disc_key(&album, disc))
            .collect();
        let operation = Operation {
            kind,
            root,
            root_id,
            roots: roots.into_iter().collect(),
            inputs: vec![input.into()],
            primary: input.into(),
            disc_order,
            selected,
            outputs: outputs.clone(),
            fallback: vec![RawAlbum { root_id, album }],
            initial: BTreeMap::new(),
        };
        tx.execute(
            "UPDATE structure_operation SET plan=?2 WHERE id=?1",
            params![id, encode(&operation)?],
        )?;
        replay(&tx)?;
        tx.commit()?;
        Ok(outputs)
    }

    /// Undo the latest structural choice. Later edits on its results are
    /// carried back to the input Albums; conflicting result edits reject undo.
    pub fn undo_structure(&mut self) -> Result<Option<Vec<PathBuf>>> {
        let tx = self.conn.transaction()?;
        let found = tx
            .query_row(
                "SELECT id,plan FROM structure_operation WHERE active=1 ORDER BY id DESC LIMIT 1",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let Some((id, json)) = found else {
            return Ok(None);
        };
        let operation: Operation = decode(&json)?;
        preserve_later_edits(&tx, &operation)?;
        for output in &operation.outputs {
            write_user_data(&tx, output, &UserData::default())?;
        }
        tx.execute("UPDATE structure_operation SET active=0 WHERE id=?1", [id])?;
        replay(&tx)?;
        tx.commit()?;
        Ok(Some(operation.inputs))
    }
}

fn ensure_renames_decided<'a>(
    conn: &Connection,
    inputs: impl IntoIterator<Item = &'a Path>,
) -> Result<()> {
    let mut statement =
        conn.prepare("SELECT EXISTS(SELECT 1 FROM rename_candidate WHERE new_path=?1)")?;
    for path in inputs {
        if statement.query_row([p(path)], |row| row.get::<_, bool>(0))? {
            return Err(invalid("请先确认或忽略改名候选，再调整分组"));
        }
    }
    Ok(())
}

pub(super) fn record_scan_and_replay(
    tx: &Transaction<'_>,
    root_id: i64,
    _root: &Path,
    scan: &ScanResult,
    complete: bool,
) -> Result<()> {
    bootstrap_raw(tx)?;
    if complete {
        tx.execute("DELETE FROM structure_raw WHERE root_id = ?1", [root_id])?;
    }
    for album in &scan.albums {
        tx.execute(
            "INSERT INTO structure_raw(path,root_id,album) VALUES (?1,?2,?3)
            ON CONFLICT(path) DO UPDATE SET root_id=excluded.root_id, album=excluded.album",
            params![super::p(&album.path), root_id, encode(album)?],
        )?;
    }
    let active: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM structure_operation WHERE active=1)",
        [],
        |row| row.get(0),
    )?;
    if active {
        replay(tx)?;
    }
    Ok(())
}

/// Last accepted physical Albums, before local merge/split operations.
pub(super) fn physical_albums(conn: &Connection, root_id: i64) -> Result<Vec<Album>> {
    bootstrap_raw(conn)?;
    let mut statement =
        conn.prepare("SELECT album FROM structure_raw WHERE root_id=?1 ORDER BY path")?;
    statement
        .query_map([root_id], |row| row.get::<_, String>(0))?
        .map(|json| decode(&json?))
        .collect()
}

fn bootstrap_raw(conn: &Connection) -> Result<()> {
    let mut statement = conn.prepare("SELECT a.path,a.root_id FROM album a WHERE NOT EXISTS(SELECT 1 FROM structure_raw r WHERE r.path=a.path)")?;
    let missing = statement
        .query_map([], |row| {
            Ok((
                PathBuf::from(row.get::<_, String>(0)?),
                row.get::<_, i64>(1)?,
            ))
        })?
        .collect::<Result<Vec<_>>>()?;
    drop(statement);
    for (path, root_id) in missing {
        if path
            .components()
            .any(|component| component.as_os_str() == ".voon-structure")
        {
            continue;
        }
        if let Some(album) = details::stored_album(conn, &path, false)? {
            conn.execute(
                "INSERT OR IGNORE INTO structure_raw(path,root_id,album) VALUES (?1,?2,?3)",
                params![p(&path), root_id, encode(&album)?],
            )?;
        }
    }
    Ok(())
}

pub(super) fn replay(conn: &Connection) -> Result<()> {
    bootstrap_raw(conn)?;
    let mut roots = conn.prepare("SELECT path,id FROM library_root")?;
    let roots = roots
        .query_map([], |row| {
            Ok((
                PathBuf::from(row.get::<_, String>(0)?),
                row.get::<_, i64>(1)?,
            ))
        })?
        .collect::<Result<BTreeMap<_, _>>>()?;
    let mut raw = conn.prepare("SELECT root_id,album FROM structure_raw ORDER BY path")?;
    let mut albums = BTreeMap::new();
    for row in raw.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })? {
        let (root_id, json) = row?;
        let album: Album = decode(&json)?;
        albums.insert(album.path.clone(), RawAlbum { root_id, album });
    }
    drop(raw);
    let mut operations =
        conn.prepare("SELECT id,plan FROM structure_operation WHERE active=1 ORDER BY id")?;
    let operations = operations
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>>>()?;
    for (id, json) in operations {
        let mut operation: Operation = decode(&json)?;
        let Some(&root_id) = roots.get(&operation.root) else {
            continue;
        };
        if operation.roots.iter().any(|root| !roots.contains_key(root)) {
            continue;
        }
        let sources: Vec<_> = operation
            .inputs
            .iter()
            .filter_map(|path| albums.get(path).cloned())
            .collect();
        let outputs = build_outputs(conn, id, &mut operation, &sources)?;
        for input in &operation.inputs {
            albums.remove(input);
        }
        for output in outputs {
            conn.execute(
                "INSERT OR IGNORE INTO album_added(root_path,album_path) VALUES (?1,?2)",
                params![p(&operation.root), p(&output.path)],
            )?;
            albums.insert(
                output.path.clone(),
                RawAlbum {
                    root_id,
                    album: output,
                },
            );
        }
        operation.fallback = sources;
        conn.execute(
            "UPDATE structure_operation SET plan=?2 WHERE id=?1",
            params![id, encode(&operation)?],
        )?;
    }
    conn.execute("DELETE FROM album", [])?;
    for source in albums.into_values() {
        details::store_album(conn, source.root_id, &source.album)?;
    }
    Ok(())
}

fn build_outputs(
    conn: &Connection,
    id: i64,
    operation: &mut Operation,
    sources: &[RawAlbum],
) -> Result<Vec<Album>> {
    if sources.is_empty() {
        return Ok(Vec::new());
    }
    if matches!(operation.kind, OperationKind::Merge) {
        normalize_disc_order(&mut operation.disc_order, sources, &operation.fallback);
        return Ok(vec![merged_album(id, operation, sources)?]);
    }
    let source = sources
        .first()
        .ok_or_else(|| invalid("专辑快照不存在，暂时无法拆分"))?;
    let data = read_user_data(conn, &operation.primary)?;
    let title = data
        .edits
        .get("title")
        .map(String::as_str)
        .unwrap_or_else(|| source.album.display_title());
    if matches!(operation.kind, OperationKind::SplitTracks) {
        let mut outputs = Vec::new();
        for position in 0..2 {
            let mut discs = Vec::new();
            for disc in &source.album.discs {
                let mut group = disc.clone();
                group.tracks.retain(|track| {
                    operation.selected.contains(&track_key(&disc.path, track)) == (position == 0)
                });
                if !group.tracks.is_empty() {
                    group.number = discs.len() as u32 + 1;
                    discs.push(group);
                }
            }
            let name = format!(
                "{title} · {}",
                if position == 0 {
                    "选中曲目"
                } else {
                    "其余曲目"
                }
            );
            outputs.push(split_output(
                conn,
                id,
                operation,
                source,
                &data,
                position,
                (name, discs),
            )?);
        }
        return Ok(outputs);
    }
    normalize_disc_order(&mut operation.disc_order, sources, &operation.fallback);
    while operation.outputs.len() < operation.disc_order.len() {
        operation.outputs.push(
            operation
                .root
                .join(".voon-structure")
                .join(id.to_string())
                .join(format!("part-{}", operation.outputs.len() + 1)),
        );
    }
    let mut outputs = Vec::new();
    for (position, key) in operation.disc_order.clone().iter().enumerate() {
        let Some(disc) = source
            .album
            .discs
            .iter()
            .find(|disc| disc_matches(key, &source.album, disc))
        else {
            continue;
        };
        let name = format!("{title} · 碟 {}", disc.number);
        let mut disc = disc.clone();
        disc.number = 1;
        outputs.push(split_output(
            conn,
            id,
            operation,
            source,
            &data,
            position,
            (name, vec![disc]),
        )?);
    }
    Ok(outputs)
}

fn split_output(
    conn: &Connection,
    id: i64,
    operation: &mut Operation,
    source: &RawAlbum,
    data: &UserData,
    position: usize,
    result: (String, Vec<Disc>),
) -> Result<Album> {
    let (name, discs) = result;
    let mut album = source.album.clone();
    album.path = operation.outputs[position].clone();
    album.folder_name = name;
    album.discs = discs;
    let directories = source
        .album
        .physical_directories()
        .into_iter()
        .map(Path::to_path_buf)
        .collect::<Vec<_>>();
    album.source_directories = if album.tracks().next().is_none() {
        directories.clone()
    } else {
        directories
            .iter()
            .filter(|directory| {
                album.tracks().any(|track| {
                    track.source.audio_path().starts_with(directory)
                        && !directories.iter().any(|other| {
                            other != *directory
                                && other.starts_with(directory)
                                && track.source.audio_path().starts_with(other)
                        })
                })
            })
            .cloned()
            .collect()
    };
    album.signature = format!("structure:{id}:{position}");
    if !operation.initial.contains_key(&album.path) {
        let mut initial = data.clone();
        initial
            .edits
            .insert("title".into(), album.folder_name.clone());
        write_user_data(conn, &album.path, &initial)?;
        operation.initial.insert(album.path.clone(), initial);
    }
    Ok(album)
}

fn merged_album(id: i64, operation: &Operation, sources: &[RawAlbum]) -> Result<Album> {
    let primary = sources
        .iter()
        .find(|source| source.album.path == operation.primary)
        .or_else(|| sources.first())
        .ok_or_else(|| invalid("主专辑快照不存在，暂时无法应用结构选择"))?;
    let ordered: Vec<_> = std::iter::once(primary)
        .chain(
            sources
                .iter()
                .filter(|source| source.album.path != primary.album.path),
        )
        .collect();
    let mut metadata = AlbumMetadata::default();
    for name in ALBUM_FIELDS {
        if let Some((_, value, source)) = ordered
            .iter()
            .flat_map(|source| source.album.metadata.entries())
            .find(|(field, value, _)| field == name && !value.trim().is_empty())
        {
            metadata.apply(name, &value, source);
        }
    }
    let keys = operation.disc_order.clone();
    let mut discs = Vec::new();
    for key in keys {
        if let Some(disc) = sources
            .iter()
            .find(|source| source.album.path == key.album)
            .and_then(|source| {
                source
                    .album
                    .discs
                    .iter()
                    .find(|disc| disc_matches(&key, &source.album, disc))
            })
        {
            let mut disc = disc.clone();
            disc.number = discs.len() as u32 + 1;
            discs.push(disc);
        }
    }
    let mut source_directories = Vec::new();
    for source in &ordered {
        for directory in source.album.physical_directories() {
            if !source_directories
                .iter()
                .any(|existing| existing == directory)
            {
                source_directories.push(directory.to_path_buf());
            }
        }
    }
    Ok(Album {
        path: operation.outputs[0].clone(),
        source_directories,
        folder_name: primary.album.folder_name.clone(),
        discs,
        cover: ordered.iter().find_map(|source| source.album.cover.clone()),
        metadata,
        collection: ordered
            .iter()
            .find_map(|source| source.album.collection.clone()),
        signature: format!("structure:{id}"),
    })
}

fn track_key(disc: &Path, track: &Track) -> TrackKey {
    let (cue, start) = match &track.source {
        TrackSource::File { .. } => (None, None),
        TrackSource::Virtual { cue, start, .. } => (Some(cue.clone()), Some(*start)),
    };
    TrackKey {
        disc: disc.into(),
        audio: track.source.audio_path().clone(),
        cue,
        start,
    }
}

fn disc_key(album: &Album, disc: &Disc) -> DiscKey {
    let disambiguate = album
        .discs
        .iter()
        .filter(|other| other.path == disc.path)
        .count()
        > 1;
    DiscKey {
        album: album.path.clone(),
        path: disc.path.clone(),
        anchor: None,
        anchors: if disambiguate {
            disc.tracks
                .iter()
                .map(|track| track_key(&disc.path, track))
                .collect()
        } else {
            Vec::new()
        },
        disambiguate,
        track_subset: is_virtual_track_subset(album, disc),
    }
}

fn disc_matches(key: &DiscKey, album: &Album, disc: &Disc) -> bool {
    if key.album != album.path || key.path != disc.path {
        return false;
    }
    if !key.disambiguate {
        return true;
    }
    disc.tracks.iter().any(|track| {
        let current = track_key(&disc.path, track);
        key.anchors.iter().chain(key.anchor.iter()).any(|anchor| {
            anchor.audio == current.audio
                && anchor.cue == current.cue
                && (!key.track_subset || anchor.start == current.start)
        })
    })
}

fn is_virtual_track_subset(album: &Album, disc: &Disc) -> bool {
    disc.tracks.iter().any(|track| {
        let TrackSource::Virtual { image, cue, .. } = &track.source else { return false; };
        album.discs.iter().filter(|other| other.path == disc.path && other.tracks.iter().any(|track| {
            matches!(&track.source, TrackSource::Virtual { image: other_image, cue: other_cue, .. } if other_image == image && other_cue == cue)
        })).take(2).count() > 1
    })
}

fn normalize_disc_order(keys: &mut Vec<DiscKey>, sources: &[RawAlbum], previous: &[RawAlbum]) {
    for key in keys.iter_mut() {
        let prior = previous
            .iter()
            .find(|source| source.album.path == key.album);
        let current = sources.iter().find(|source| source.album.path == key.album);
        if !key.track_subset {
            key.track_subset = prior.into_iter().chain(current).any(|source| {
                source.album.discs.iter().any(|disc| {
                    disc.path == key.path && is_virtual_track_subset(&source.album, disc)
                })
            });
        }
        let prior_count = prior
            .map(|source| {
                source
                    .album
                    .discs
                    .iter()
                    .filter(|disc| disc.path == key.path)
                    .count()
            })
            .unwrap_or_default();
        let current_count = current
            .map(|source| {
                source
                    .album
                    .discs
                    .iter()
                    .filter(|disc| disc.path == key.path)
                    .count()
            })
            .unwrap_or_default();
        if !key.disambiguate && (prior_count > 1 || current_count > 1) {
            key.disambiguate = true;
            if let Some(anchor) = key.anchor.take() {
                key.anchors.push(anchor);
            }
            if key.anchors.is_empty()
                && let Some(disc) = prior
                    .or(current)
                    .and_then(|source| source.album.discs.iter().find(|disc| disc.path == key.path))
            {
                key.anchors = disc
                    .tracks
                    .iter()
                    .map(|track| track_key(&disc.path, track))
                    .collect();
            }
        }
        if !key.disambiguate {
            key.anchor = None;
            key.anchors.clear();
        }
    }
    for source in sources {
        for disc in &source.album.discs {
            if !keys
                .iter()
                .any(|key| disc_matches(key, &source.album, disc))
            {
                keys.push(disc_key(&source.album, disc));
            }
        }
    }
}

fn merged_user_data(conn: &Connection, sources: &[RawAlbum], primary: &Path) -> Result<UserData> {
    let mut resolved = Vec::new();
    let ordered = sources
        .iter()
        .filter(|source| source.album.path == primary)
        .chain(sources.iter().filter(|source| source.album.path != primary));
    for source in ordered {
        resolved.push((
            details::stored_album(conn, &source.album.path, true)?
                .unwrap_or_else(|| source.album.clone()),
            read_user_data(conn, &source.album.path)?,
        ));
    }
    let mut data = UserData::default();
    for name in ALBUM_FIELDS {
        if let Some((_, value, source)) = resolved
            .iter()
            .flat_map(|(album, _)| album.metadata.entries())
            .find(|(field, value, _)| field == name && !value.trim().is_empty())
            && source == FieldSource::UserEdit
        {
            data.edits.insert((*name).into(), value);
        }
    }
    for (_, source) in &resolved {
        data.tags.extend(&source.tags);
        for (&collection, &position) in &source.collections {
            data.collections
                .entry(collection)
                .and_modify(|current| *current = (*current).min(position))
                .or_insert(position);
        }
    }
    data.cover = resolved
        .iter()
        .find(|(album, _)| album.cover.is_some())
        .and_then(|(_, source)| source.cover.clone());
    Ok(data)
}

fn read_user_data(conn: &Connection, path: &Path) -> Result<UserData> {
    let mut statement = conn.prepare("SELECT name,value FROM user_edit WHERE album_path=?1")?;
    let edits = statement
        .query_map([p(path)], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_>>()?;
    let mut statement = conn.prepare("SELECT tag_id FROM album_user_tag WHERE album_path=?1")?;
    let tags = statement
        .query_map([p(path)], |row| row.get(0))?
        .collect::<Result<_>>()?;
    let mut statement = conn.prepare(
        "SELECT collection_id,position FROM manual_collection_album WHERE album_path=?1",
    )?;
    let collections = statement
        .query_map([p(path)], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_>>()?;
    Ok(UserData {
        edits,
        tags,
        collections,
        cover: covers::read_user_cover(conn, path)?,
    })
}

fn write_user_data(conn: &Connection, path: &Path, data: &UserData) -> Result<()> {
    for table in ["user_edit", "album_user_tag", "manual_collection_album"] {
        conn.execute(
            &format!("DELETE FROM {table} WHERE album_path=?1"),
            [p(path)],
        )?;
    }
    for (name, value) in &data.edits {
        conn.execute(
            "INSERT INTO user_edit(album_path,name,value) VALUES (?1,?2,?3)",
            params![p(path), name, value],
        )?;
    }
    for tag in &data.tags {
        conn.execute("INSERT INTO album_user_tag(album_path,tag_id) SELECT ?1,?2 WHERE EXISTS(SELECT 1 FROM user_tag WHERE id=?2)",params![p(path),tag])?;
    }
    for (collection, position) in &data.collections {
        conn.execute("INSERT INTO manual_collection_album(album_path,collection_id,position) SELECT ?1,?2,?3 WHERE EXISTS(SELECT 1 FROM manual_collection WHERE id=?2)",params![p(path),collection,position])?;
    }
    covers::write_user_cover(conn, path, data.cover.as_ref())
}

fn preserve_later_edits(conn: &Connection, operation: &Operation) -> Result<()> {
    let mut edits = BTreeMap::new();
    let mut tags = BTreeMap::new();
    let mut collections = BTreeMap::new();
    let mut cover = None;
    for path in &operation.outputs {
        let initial = operation.initial.get(path).cloned().unwrap_or_default();
        let current = read_user_data(conn, path)?;
        for name in initial
            .edits
            .keys()
            .chain(current.edits.keys())
            .collect::<BTreeSet<_>>()
        {
            if initial.edits.get(name) != current.edits.get(name) {
                combine(
                    &mut edits,
                    name.clone(),
                    current.edits.get(name).cloned(),
                    "拆分结果存在不同的后续字段编辑，请先统一后再撤销",
                )?;
            }
        }
        for &tag in initial.tags.union(&current.tags) {
            if initial.tags.contains(&tag) != current.tags.contains(&tag) {
                combine(
                    &mut tags,
                    tag,
                    current.tags.contains(&tag),
                    "拆分结果存在相冲突的后续标签，请先统一后再撤销",
                )?;
            }
        }
        for &collection in initial
            .collections
            .keys()
            .chain(current.collections.keys())
            .collect::<BTreeSet<_>>()
        {
            if initial.collections.get(&collection) != current.collections.get(&collection) {
                combine(
                    &mut collections,
                    collection,
                    current.collections.get(&collection).copied(),
                    "拆分结果存在相冲突的合集顺序，请先统一后再撤销",
                )?;
            }
        }
        if initial.cover != current.cover {
            if cover.as_ref().is_some_and(|cover| cover != &current.cover) {
                return Err(invalid("拆分结果存在不同的后续封面选择，请先统一后再撤销"));
            }
            cover = Some(current.cover);
        }
    }
    for path in &operation.inputs {
        let mut data = read_user_data(conn, path)?;
        for (name, value) in &edits {
            if let Some(value) = value {
                data.edits.insert(name.clone(), value.clone());
            } else {
                data.edits.remove(name);
            }
        }
        for (&tag, &present) in &tags {
            if present {
                data.tags.insert(tag);
            } else {
                data.tags.remove(&tag);
            }
        }
        for (&collection, position) in &collections {
            if let Some(position) = position {
                data.collections.insert(collection, *position);
            } else {
                data.collections.remove(&collection);
            }
        }
        if let Some(cover) = &cover {
            data.cover = cover.clone();
        }
        write_user_data(conn, path, &data)?;
    }
    Ok(())
}

fn combine<K: Ord, V: PartialEq>(
    values: &mut BTreeMap<K, V>,
    key: K,
    value: V,
    message: &str,
) -> Result<()> {
    if values.get(&key).is_some_and(|existing| existing != &value) {
        return Err(invalid(message));
    }
    values.insert(key, value);
    Ok(())
}

fn invalid(message: &str) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(message)))
}

/// Called only after the user confirms a physical rename. Rewrite path fields,
/// leaving every metadata value and the user's structural choice intact.
pub(super) fn confirm_rename(conn: &Connection, old: &Path, new: &Path) -> Result<()> {
    let mut statement =
        conn.prepare("SELECT id,plan FROM structure_operation WHERE active=1 ORDER BY id")?;
    let operations = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>>>()?;
    drop(statement);
    for (id, json) in operations {
        let mut operation: Operation = decode(&json)?;
        for path in &mut operation.inputs {
            rebase(path, old, new);
        }
        rebase(&mut operation.primary, old, new);
        for key in &mut operation.disc_order {
            rebase(&mut key.album, old, new);
            rebase(&mut key.path, old, new);
            for anchor in key.anchor.iter_mut().chain(key.anchors.iter_mut()) {
                rebase_track_key(anchor, old, new);
            }
        }
        for key in &mut operation.selected {
            rebase_track_key(key, old, new);
        }
        for fallback in &mut operation.fallback {
            let album = &mut fallback.album;
            rebase(&mut album.path, old, new);
            for directory in &mut album.source_directories {
                rebase(directory, old, new);
            }
            if let Some(collection) = &mut album.collection {
                rebase(collection, old, new);
            }
            if let Some(cover) = &mut album.cover {
                rebase_cover(cover, old, new);
            }
            for disc in &mut album.discs {
                rebase(&mut disc.path, old, new);
                for track in &mut disc.tracks {
                    match &mut track.source {
                        TrackSource::File { path } => rebase(path, old, new),
                        TrackSource::Virtual { image, cue, .. } => {
                            rebase(image, old, new);
                            rebase(cue, old, new);
                        }
                    }
                }
            }
        }
        for (path, initial) in &mut operation.initial {
            if let Some(cover) = &mut initial.cover {
                rebase_cover(cover, old, new);
            }
            if let Some(mut cover) = covers::read_user_cover(conn, path)? {
                rebase_cover(&mut cover, old, new);
                covers::write_user_cover(conn, path, Some(&cover))?;
            }
        }
        conn.execute(
            "UPDATE structure_operation SET plan=?2 WHERE id=?1",
            params![id, encode(&operation)?],
        )?;
    }
    replay(conn)
}

fn rebase(path: &mut PathBuf, old: &Path, new: &Path) {
    if let Ok(relative) = path.strip_prefix(old) {
        *path = new.join(relative);
    }
}
fn rebase_track_key(key: &mut TrackKey, old: &Path, new: &Path) {
    rebase(&mut key.disc, old, new);
    rebase(&mut key.audio, old, new);
    if let Some(cue) = &mut key.cue {
        rebase(cue, old, new);
    }
}
fn rebase_cover(cover: &mut Cover, old: &Path, new: &Path) {
    match &mut cover.source {
        crate::CoverSource::File(path)
        | crate::CoverSource::Embedded(path)
        | crate::CoverSource::EmbeddedPicture { path, .. } => rebase(path, old, new),
    }
}

fn encode<T: serde::Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

fn decode<T: serde::de::DeserializeOwned>(text: &str) -> Result<T> {
    serde_json::from_str(text).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}
