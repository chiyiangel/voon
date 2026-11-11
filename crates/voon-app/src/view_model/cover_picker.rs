use super::{AppState, Result};
use gpui::Context;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
};
use voon_core::{Album, Cover, covers::CoverCandidate};

pub type CoverLoadResult = std::result::Result<Vec<CoverCandidate>, String>;

/// The filesystem boundary can deliver deterministic candidates in GPUI tests.
pub trait CoverLoader {
    fn start(
        &self,
        album: Album,
        ignored: Vec<PathBuf>,
        cancel: Arc<AtomicBool>,
    ) -> Receiver<CoverLoadResult>;
}

pub struct DiskCoverLoader;
impl CoverLoader for DiskCoverLoader {
    fn start(
        &self,
        album: Album,
        ignored: Vec<PathBuf>,
        cancel: Arc<AtomicBool>,
    ) -> Receiver<CoverLoadResult> {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let result = voon_core::covers::candidates_with_cancel(&album, &ignored, &cancel)
                .map_err(|error| format!("无法读取候选封面：{error}"));
            let _ = sender.send(result);
        });
        receiver
    }
}

#[derive(Clone)]
pub struct CoverPickerDraft {
    pub album_path: PathBuf,
    pub title: String,
    pub automatic: Option<Cover>,
    pub selected: Option<Cover>,
    pub candidates: Vec<CoverCandidate>,
    pub loading: bool,
    pub error: Option<String>,
}

struct Session {
    receiver: Receiver<CoverLoadResult>,
    cancel: Arc<AtomicBool>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

pub(super) struct CoverPickerState {
    draft: Option<CoverPickerDraft>,
    pending: Option<Session>,
    loader: Box<dyn CoverLoader>,
}
impl Default for CoverPickerState {
    fn default() -> Self {
        Self {
            draft: None,
            pending: None,
            loader: Box::new(DiskCoverLoader),
        }
    }
}

impl AppState {
    pub fn with_cover_loader(mut self, loader: Box<dyn CoverLoader>) -> Self {
        self.cover_picker.loader = loader;
        self
    }
    pub fn cover_picker(&self) -> Option<&CoverPickerDraft> {
        self.cover_picker.draft.as_ref()
    }

    pub fn begin_cover_picker(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let album = self.album_detail().cloned().ok_or("请先选择专辑")?;
        let ignored = self.ignored_image_paths()?;
        self.cover_picker.draft = Some(CoverPickerDraft {
            album_path: album.path.clone(),
            title: album.display_title().to_string(),
            automatic: self.index.automatic_cover(&album.path)?,
            selected: self.index.user_cover(&album.path)?,
            candidates: vec![],
            loading: true,
            error: None,
        });
        let cancel = Arc::new(AtomicBool::new(false));
        let receiver = self
            .cover_picker
            .loader
            .start(album, ignored, cancel.clone());
        self.cover_picker.pending = Some(Session { receiver, cancel });
        cx.notify();
        Ok(())
    }

    pub fn select_cover_candidate(
        &mut self,
        cover: Option<Cover>,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        let draft = self.cover_picker.draft.as_mut().ok_or("请先打开封面选择")?;
        if let Some(cover) = &cover
            && !draft
                .candidates
                .iter()
                .any(|candidate| &candidate.cover == cover)
        {
            return Err("请从当前专辑的候选封面中选择".into());
        }
        draft.selected = cover;
        cx.notify();
        Ok(())
    }

    pub fn cancel_cover_picker(&mut self, cx: &mut Context<Self>) {
        self.cover_picker.pending = None;
        self.cover_picker.draft = None;
        cx.notify();
    }

    pub fn apply_cover_choice(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let draft = self.cover_picker.draft.as_ref().ok_or("请先打开封面选择")?;
        let result = if let Some(cover) = &draft.selected {
            self.index.set_user_cover(&draft.album_path, cover)
        } else {
            self.index.clear_user_cover(&draft.album_path)
        };
        if let Err(error) = result {
            if let Some(draft) = &mut self.cover_picker.draft {
                draft.error = Some(error.to_string());
            }
            cx.notify();
            return Err(error.into());
        }
        self.reload_library(cx)?;
        self.cancel_cover_picker(cx);
        Ok(())
    }

    pub(super) fn poll_cover_picker(&mut self, cx: &mut Context<Self>) {
        let Some(pending) = &self.cover_picker.pending else {
            return;
        };
        let result = match pending.receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("候选封面加载意外结束，请重新打开".into()),
        };
        self.cover_picker.pending = None;
        if let Some(draft) = &mut self.cover_picker.draft {
            draft.loading = false;
            match result {
                Ok(candidates) => draft.candidates = candidates,
                Err(error) => draft.error = Some(error),
            }
        }
        cx.notify();
    }
}
