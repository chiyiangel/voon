//! Background filesystem scanning, observed through the application state.
use super::{AppState, Result, ScanState};
use gpui::Context;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
};
use voon_core::{
    Album, AudioFormat, ScanResult,
    db::{AlbumRow, CachedTags, Index},
    scan::{DiskTags, FileStamp, ScanEvent, ScanOptions, TagReader},
};

pub enum ScanUpdate {
    Directory(PathBuf),
    Album(Box<Album>),
    Finished(ScanResult, usize),
    /// The root vanished or could not be listed; this is never a full snapshot.
    Unavailable(ScanResult),
    Failed(String),
}

/// Tests can deliver scanner events without scheduling native filesystem work.
pub trait Scanner {
    fn start(&self, root: PathBuf, cancel: Arc<AtomicBool>) -> Receiver<ScanUpdate>;
}

pub struct DiskScanner {
    database_path: Option<PathBuf>,
}
impl DiskScanner {
    pub fn new(database_path: Option<PathBuf>) -> Self {
        Self { database_path }
    }
}
impl Scanner for DiskScanner {
    fn start(&self, root: PathBuf, cancel: Arc<AtomicBool>) -> Receiver<ScanUpdate> {
        let (sender, receiver) = mpsc::channel();
        let database_path = self.database_path.clone();
        std::thread::spawn(move || {
            use std::os::unix::fs::MetadataExt;
            let device_before = root.metadata().ok().map(|metadata| metadata.dev());
            let index = match database_path.map(|path| Index::open(&path)).transpose() {
                Ok(index) => index,
                Err(error) => {
                    let _ = sender.send(ScanUpdate::Failed(format!("无法打开本地索引：{error}")));
                    return;
                }
            };
            let options = match index
                .as_ref()
                .map(|index| index.ignored_paths(&root))
                .transpose()
            {
                Ok(ignored_paths) => ScanOptions {
                    ignored_paths: ignored_paths.unwrap_or_default(),
                },
                Err(error) => {
                    let _ = sender.send(ScanUpdate::Failed(format!("无法读取忽略规则：{error}")));
                    return;
                }
            };
            let mut progress = |event: ScanEvent<'_>| {
                let update = match event {
                    ScanEvent::Directory(path) => ScanUpdate::Directory(path.into()),
                    ScanEvent::Album(album) => ScanUpdate::Album(Box::new(album.clone())),
                };
                if sender.send(update).is_err() {
                    cancel.store(true, Ordering::Relaxed);
                }
            };
            let (scan, reads) = if let Some(index) = index {
                let mut tags = CachedTags::new(&index, DiskTags);
                let scan = voon_core::scan::scan_root_with_options(
                    &root,
                    &mut tags,
                    &cancel,
                    &mut progress,
                    &options,
                );
                (scan, tags.reads)
            } else {
                let mut tags = CountingTags {
                    inner: DiskTags,
                    reads: 0,
                };
                let scan = voon_core::scan::scan_root_with_options(
                    &root,
                    &mut tags,
                    &cancel,
                    &mut progress,
                    &options,
                );
                (scan, tags.reads)
            };
            let device_after = root.metadata().ok().map(|metadata| metadata.dev());
            let unavailable = !root.is_dir()
                || device_before != device_after
                || scan.issues.iter().any(|issue| {
                    issue.path == root
                        && matches!(issue.kind, voon_core::scan::IssueKind::Unreadable { .. })
                });
            let update = if unavailable {
                ScanUpdate::Unavailable(scan)
            } else {
                ScanUpdate::Finished(scan, reads)
            };
            let _ = sender.send(update);
        });
        receiver
    }
}

struct CountingTags<R> {
    inner: R,
    reads: usize,
}
impl<R: TagReader> TagReader for CountingTags<R> {
    fn read(
        &mut self,
        path: &Path,
        format: AudioFormat,
        stamp: FileStamp,
    ) -> Option<voon_core::tags::TagInfo> {
        self.reads += 1;
        self.inner.read(path, format, stamp)
    }

    fn read_observation(
        &mut self,
        path: &Path,
        format: AudioFormat,
        stamp: FileStamp,
    ) -> voon_core::tags::TagObservation {
        self.reads += 1;
        self.inner.read_observation(path, format, stamp)
    }
}

pub(super) struct ScanSession {
    root: PathBuf,
    receiver: Receiver<ScanUpdate>,
    cancel: Arc<AtomicBool>,
    previews: HashMap<PathBuf, Album>,
    suppress_previews: bool,
}

pub(super) struct PendingEmptyScan {
    root: PathBuf,
    scan: ScanResult,
    reads: usize,
}
impl ScanSession {
    pub(super) fn preview_album(&self, path: &Path) -> Option<&Album> {
        self.previews.get(path)
    }

    fn discard_late_results(&self, scan: &mut ScanResult) {
        if self.cancel.load(Ordering::Relaxed) {
            scan.cancelled = true;
            scan.albums = self.previews.values().cloned().collect();
            scan.collections.retain(|collection| {
                collection
                    .albums
                    .iter()
                    .any(|path| self.previews.contains_key(path))
            });
        }
    }
}
impl Drop for ScanSession {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl AppState {
    pub fn with_scanner(mut self, scanner: Box<dyn Scanner>) -> Self {
        self.scanner = scanner;
        self
    }

    pub fn add_root(&mut self, root: PathBuf, cx: &mut Context<Self>) -> Result<()> {
        if self.index_open_problem().is_some() {
            return Err("请先新建可用的本地索引，再添加音乐库重新扫描".into());
        }
        if self
            .scan_session
            .as_ref()
            .is_some_and(|session| !session.cancel.load(Ordering::Relaxed))
        {
            return Err("请等待当前扫描结束，或先取消扫描".into());
        }
        let root = std::fs::canonicalize(root)?;
        if !root.is_dir() {
            return Err("请选择音乐文件夹".into());
        }
        if self
            .index
            .database_path()
            .and_then(|path| path.canonicalize().ok())
            .is_some_and(|path| path.starts_with(&root))
        {
            return Err("音乐库不能包含 Voon 的本地索引，请选择独立的音乐文件夹".into());
        }
        if self
            .roots
            .iter()
            .any(|existing| root.starts_with(&existing.path) || existing.path.starts_with(&root))
        {
            return Err("音乐库根目录不能重复或相互包含".into());
        }
        self.index.add_root(&root)?;
        self.reload_library(cx)?;
        self.start_scan(root, cx)
    }

    pub fn start_scan(&mut self, root: PathBuf, cx: &mut Context<Self>) -> Result<()> {
        if self
            .scan_session
            .as_ref()
            .is_some_and(|session| !session.cancel.load(Ordering::Relaxed))
        {
            return Err("已有扫描正在进行".into());
        }
        if !self.roots.iter().any(|existing| existing.path == root) {
            return Err("音乐库根目录不存在".into());
        }
        if let Some(previous) = &self.scan_session {
            let completed = ScanResult {
                albums: previous.previews.values().cloned().collect(),
                cancelled: true,
                ..Default::default()
            };
            self.index.store_partial_scan(&previous.root, &completed)?;
            // Dropping this receiver isolates the next task from every late
            // event of the cancelled worker, even if it cannot stop an I/O yet.
            self.scan_session = None;
            self.reload_library(cx)?;
        }
        self.pending_empty_scan = None;
        if self.offline_roots().contains(&root) {
            self.scan_state = ScanState::Missing { root };
            cx.notify();
            return Ok(());
        }
        self.forget_root_availability(&root);
        let suppress_previews = self.index.has_structure_operations(&root)?;
        let cancel = Arc::new(AtomicBool::new(false));
        let receiver = self.scanner.start(root.clone(), cancel.clone());
        self.scan_state = ScanState::Scanning {
            root: root.clone(),
            current_directory: root.clone(),
            discovered: 0,
        };
        self.scan_session = Some(ScanSession {
            root,
            receiver,
            cancel,
            previews: HashMap::new(),
            suppress_previews,
        });
        self.error = None;
        cx.notify();
        Ok(())
    }

    pub fn cancel_scan(&mut self, cx: &mut Context<Self>) {
        if let Some(scan) = &self.scan_session {
            scan.cancel.store(true, Ordering::Relaxed);
        }
        cx.notify();
    }

    fn preserve_interrupted_discoveries(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let Some(session) = self.scan_session.take() else {
            return Ok(());
        };
        let completed = ScanResult {
            albums: session.previews.values().cloned().collect(),
            cancelled: true,
            ..Default::default()
        };
        let stored = self.index.store_partial_scan(&session.root, &completed);
        let reloaded = self.reload_library(cx);
        stored?;
        reloaded
    }

    /// Drain only a bounded batch so a fast scan cannot starve UI input.
    pub fn poll_scan(&mut self, cx: &mut Context<Self>) {
        self.poll_root_availability(cx);
        self.prefetch_covers(cx);
        self.poll_detail(cx);
        self.poll_cover_picker(cx);
        for _ in 0..64 {
            let Some(session) = &self.scan_session else {
                return;
            };
            let root = session.root.clone();
            match session.receiver.try_recv() {
                Ok(ScanUpdate::Directory(path)) => {
                    if session.cancel.load(Ordering::Relaxed) {
                        continue;
                    }
                    if let ScanState::Scanning {
                        current_directory, ..
                    } = &mut self.scan_state
                    {
                        *current_directory = path;
                    }
                }
                Ok(ScanUpdate::Album(album)) => {
                    if session.cancel.load(Ordering::Relaxed) {
                        continue;
                    }
                    if let ScanState::Scanning { discovered, .. } = &mut self.scan_state {
                        *discovered += 1;
                    }
                    if !session.suppress_previews
                        && !self
                            .albums
                            .iter()
                            .any(|existing| existing.path == album.path)
                    {
                        self.albums.push(AlbumRow::from_album(&album));
                        self.refresh_browse();
                    }
                    if let Some(session) = &mut self.scan_session {
                        session.previews.insert(album.path.clone(), *album);
                    }
                }
                Ok(ScanUpdate::Finished(mut scan, reads)) => {
                    session.discard_late_results(&mut scan);
                    let cancelled = scan.cancelled || session.cancel.load(Ordering::Relaxed);
                    self.scan_session = None;
                    if !cancelled
                        && scan.albums.is_empty()
                        && scan.incomplete_paths().is_empty()
                        && scan.root_was_listed(&root)
                    {
                        match self.index.has_scanned_albums(&root) {
                            Ok(true) => {
                                self.scan_state = ScanState::Empty { root: root.clone() };
                                self.pending_empty_scan =
                                    Some(PendingEmptyScan { root, scan, reads });
                                cx.notify();
                                return;
                            }
                            Ok(false) => {}
                            Err(error) => {
                                self.error = Some(format!("无法核对上次扫描，已保留索引：{error}"));
                                self.scan_state = ScanState::Idle;
                                cx.notify();
                                return;
                            }
                        }
                    }
                    let stored = if cancelled {
                        self.index.store_partial_scan(&root, &scan)
                    } else {
                        self.index.store_scan(&root, &scan)
                    };
                    match stored.map_err(|error| error.to_string()) {
                        Ok(()) => {
                            self.scan_state = if cancelled {
                                ScanState::Cancelled {
                                    root,
                                    discovered: scan.albums.len(),
                                }
                            } else {
                                ScanState::Done {
                                    root,
                                    discovered: scan.albums.len(),
                                    reads,
                                }
                            };
                            if let Err(error) = self.reload_library(cx) {
                                self.error = Some(error.to_string());
                            }
                        }
                        Err(error) => {
                            self.error = Some(format!("无法保存扫描结果：{error}"));
                            self.scan_state = ScanState::Idle;
                        }
                    }
                }
                Ok(ScanUpdate::Failed(error)) => {
                    self.scan_state = ScanState::Idle;
                    self.error = Some(match self.preserve_interrupted_discoveries(cx) {
                        Ok(()) => error,
                        Err(save_error) => format!("{error}；无法保存已完成专辑：{save_error}"),
                    });
                }
                Ok(ScanUpdate::Unavailable(mut scan)) => {
                    session.discard_late_results(&mut scan);
                    self.scan_session = None;
                    if let Err(error) = self.index.store_partial_scan(&root, &scan) {
                        self.error = Some(format!("无法保存部分扫描结果：{error}"));
                    }
                    self.mark_root_unavailable(&root);
                    self.scan_state = ScanState::Missing { root };
                    if let Err(error) = self.reload_library(cx) {
                        self.error = Some(error.to_string());
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.scan_state = ScanState::Idle;
                    self.error = Some(match self.preserve_interrupted_discoveries(cx) {
                        Ok(()) => "扫描意外结束，已保留已完成专辑，请重试".into(),
                        Err(error) => format!("扫描意外结束，无法保存已完成专辑：{error}"),
                    });
                    break;
                }
            }
            cx.notify();
        }
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// A readable but empty mountpoint is ambiguous; replacing its cached
    /// albums requires this explicit local-index action.
    pub fn confirm_empty_scan(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let pending = self
            .pending_empty_scan
            .as_ref()
            .ok_or("没有等待确认的空扫描")?;
        self.index.store_scan(&pending.root, &pending.scan)?;
        self.scan_state = ScanState::Done {
            root: pending.root.clone(),
            discovered: 0,
            reads: pending.reads,
        };
        self.pending_empty_scan = None;
        self.reload_library(cx)
    }

    pub fn keep_previous_scan(&mut self, cx: &mut Context<Self>) {
        self.pending_empty_scan = None;
        self.scan_state = ScanState::Idle;
        cx.notify();
    }
    pub fn report_error(&mut self, error: impl std::fmt::Display, cx: &mut Context<Self>) {
        self.error = Some(error.to_string());
        cx.notify();
    }
    pub fn settings_open(&self) -> bool {
        self.settings_open
    }
    pub fn show_settings(&mut self, open: bool, cx: &mut Context<Self>) {
        self.settings_open = open;
        cx.notify();
    }
    pub fn pending_root_removal(&self) -> Option<&Path> {
        self.pending_root_removal.as_deref()
    }
    pub fn request_root_removal(&mut self, root: Option<PathBuf>, cx: &mut Context<Self>) {
        self.pending_root_removal = root;
        cx.notify();
    }
    pub fn confirm_root_removal(&mut self, cx: &mut Context<Self>) -> Result<()> {
        if let Some(root) = self.pending_root_removal.take() {
            if self
                .pending_empty_scan
                .as_ref()
                .is_some_and(|pending| pending.root == root)
            {
                self.pending_empty_scan = None;
                self.scan_state = ScanState::Idle;
            }
            if self
                .scan_session
                .as_ref()
                .is_some_and(|scan| scan.root == root)
            {
                self.scan_session = None;
                self.scan_state = ScanState::Idle;
            }
            self.index.remove_root(&root)?;
            self.forget_root_availability(&root);
            self.reload_library(cx)?;
        }
        Ok(())
    }
}
