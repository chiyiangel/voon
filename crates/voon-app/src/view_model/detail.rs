//! Selected Album details are read from the local index, independent of NAS health.
use gpui::Context;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
};
use voon_core::{Album, covers::discover_images};

use super::{AppState, Result};
use crate::platform::{NativeSystemOpener, PathAction, SystemOpener};

pub(super) struct DetailState {
    pub(super) album: Option<Album>,
    images: Vec<PathBuf>,
    image_error: Option<String>,
    pending_images: Option<AttachmentSession>,
    opener: Arc<dyn SystemOpener>,
    operations: Vec<Receiver<std::result::Result<(), String>>>,
}

impl Default for DetailState {
    fn default() -> Self {
        Self {
            album: None,
            images: Vec::new(),
            image_error: None,
            pending_images: None,
            opener: Arc::new(NativeSystemOpener),
            operations: Vec::new(),
        }
    }
}

struct AttachmentSession {
    receiver: Receiver<std::result::Result<Vec<PathBuf>, String>>,
    cancel: Arc<AtomicBool>,
}
impl Drop for AttachmentSession {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl AppState {
    /// Absolute ignore paths also cover secondary physical sources of a local
    /// merge; its logical Album key may belong to only the primary root.
    pub(super) fn ignored_image_paths(&self) -> Result<Vec<PathBuf>> {
        let mut ignored = Vec::new();
        for root in &self.roots {
            ignored.extend(
                self.index
                    .ignored_paths(&root.path)?
                    .into_iter()
                    .map(|path| root.path.join(path)),
            );
        }
        Ok(ignored)
    }

    pub fn album_detail(&self) -> Option<&Album> {
        self.detail.album.as_ref()
    }
    pub fn attachment_images(&self) -> &[PathBuf] {
        &self.detail.images
    }
    pub fn attachment_images_loading(&self) -> bool {
        self.detail.pending_images.is_some()
    }
    pub fn attachment_images_error(&self) -> Option<&str> {
        self.detail.image_error.as_deref()
    }
    pub fn file_operations_pending(&self) -> bool {
        !self.detail.operations.is_empty()
    }
    pub fn with_system_opener(mut self, opener: Arc<dyn SystemOpener>) -> Self {
        self.detail.opener = opener;
        self
    }

    pub fn refresh_selected_album(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let album = self
            .selected_album
            .as_ref()
            .map(|path| {
                self.index.album(path).map(|album| {
                    album.or_else(|| self.scan_session.as_ref()?.preview_album(path).cloned())
                })
            })
            .transpose()?
            .flatten();
        let changed = self.detail.album.as_ref().map(|album| &album.path)
            != album.as_ref().map(|album| &album.path);
        self.detail.album = album;
        if changed {
            self.reload_attachment_images(cx)?;
        }
        cx.notify();
        Ok(())
    }

    pub(super) fn clear_detail(&mut self) {
        self.edit_draft = None;
        self.detail.album = None;
        self.detail.images.clear();
        self.detail.image_error = None;
        self.detail.pending_images = None;
    }

    pub fn reload_attachment_images(&mut self, cx: &mut Context<Self>) -> Result<()> {
        self.detail.images.clear();
        self.detail.image_error = None;
        self.detail.pending_images = None;
        let Some(album) = self.detail.album.clone() else {
            return Ok(());
        };
        let ignored = self.ignored_image_paths()?;
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.detail.pending_images = Some(AttachmentSession {
            receiver,
            cancel: cancel.clone(),
        });
        std::thread::spawn(move || {
            let images = discover_images(&album, &ignored, &cancel)
                .map_err(|error| format!("无法读取附件图片：{error}"));
            let _ = sender.send(images);
        });
        cx.notify();
        Ok(())
    }

    pub fn reveal_selected_album(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let album = self.album_detail().ok_or("请先选择专辑")?;
        let path = album_directory(album).ok_or("此专辑没有可显示的目录")?;
        self.open_system_path(path, PathAction::Reveal, cx);
        Ok(())
    }

    pub fn open_attachment_image(&mut self, path: PathBuf, cx: &mut Context<Self>) -> Result<()> {
        if !self.detail.images.contains(&path) {
            return Err("请从当前专辑的附件图片中选择".into());
        }
        self.open_system_path(path, PathAction::Open, cx);
        Ok(())
    }

    pub(crate) fn open_system_path(
        &mut self,
        path: PathBuf,
        action: PathAction,
        cx: &mut Context<Self>,
    ) {
        let opener = self.detail.opener.clone();
        let (sender, receiver) = mpsc::channel();
        self.detail.operations.push(receiver);
        std::thread::spawn(move || {
            let result = opener
                .open(&path, action)
                .map_err(|error| format!("无法打开 {}：{error}", path.display()));
            let _ = sender.send(result);
        });
        cx.notify();
    }

    pub(super) fn poll_detail(&mut self, cx: &mut Context<Self>) {
        let images = self
            .detail
            .pending_images
            .as_ref()
            .map(|pending| pending.receiver.try_recv());
        let mut changed = false;
        match images {
            Some(Ok(images)) => {
                self.detail.pending_images = None;
                match images {
                    Ok(images) => self.detail.images = images,
                    Err(error) => self.detail.image_error = Some(error),
                }
                changed = true;
            }
            Some(Err(TryRecvError::Disconnected)) => {
                self.detail.pending_images = None;
                self.detail.image_error = Some("附件图片加载意外结束，请重试".into());
                changed = true;
            }
            Some(Err(TryRecvError::Empty)) | None => {}
        }
        self.detail
            .operations
            .retain(|operation| match operation.try_recv() {
                Ok(result) => {
                    if let Err(error) = result {
                        self.error = Some(error);
                    }
                    changed = true;
                    false
                }
                Err(TryRecvError::Disconnected) => {
                    self.error = Some("系统文件操作意外结束".into());
                    changed = true;
                    false
                }
                Err(TryRecvError::Empty) => true,
            });
        if changed {
            cx.notify();
        }
    }
}

fn album_directory(album: &Album) -> Option<PathBuf> {
    album
        .physical_directories()
        .first()
        .map(|path| path.to_path_buf())
}
