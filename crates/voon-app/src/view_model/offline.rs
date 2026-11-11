//! Root checks run outside the UI thread and at most once every five seconds.
use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, TryRecvError},
    time::{Duration, Instant},
};

use gpui::Context;

use super::{AppState, ScanState};

const CHECK_INTERVAL: Duration = Duration::from_secs(5);

pub(super) struct RootAvailability {
    missing: Vec<PathBuf>,
    next_check: Instant,
    pending: Option<Receiver<Vec<(PathBuf, bool)>>>,
}

impl Default for RootAvailability {
    fn default() -> Self {
        Self {
            missing: Vec::new(),
            next_check: Instant::now(),
            pending: None,
        }
    }
}

impl AppState {
    pub fn offline_roots(&self) -> &[PathBuf] {
        &self.availability.missing
    }

    /// Explicit reconnect checks immediately; normal UI polling is throttled.
    pub fn refresh_root_availability(&mut self, cx: &mut Context<Self>) {
        if self.availability.pending.is_some() || self.roots.is_empty() {
            return;
        }
        let roots: Vec<_> = self.roots.iter().map(|root| root.path.clone()).collect();
        let (sender, receiver) = mpsc::channel();
        self.availability.pending = Some(receiver);
        self.availability.next_check = Instant::now() + CHECK_INTERVAL;
        std::thread::spawn(move || {
            let checked = roots
                .into_iter()
                .map(|root| {
                    let available = root.is_dir();
                    (root, available)
                })
                .collect();
            let _ = sender.send(checked);
        });
        cx.notify();
    }

    pub(super) fn mark_root_unavailable(&mut self, root: &Path) {
        self.availability.pending = None;
        self.availability.next_check = Instant::now() + CHECK_INTERVAL;
        if !self.availability.missing.iter().any(|path| path == root) {
            self.availability.missing.push(root.to_path_buf());
        }
    }

    pub(super) fn forget_root_availability(&mut self, root: &Path) {
        self.availability.pending = None;
        self.availability.next_check = Instant::now() + CHECK_INTERVAL;
        let was_missing = self.availability.missing.iter().any(|path| path == root);
        self.availability.missing.retain(|path| path != root);
        if was_missing {
            self.refresh_covers();
        }
    }

    pub(super) fn poll_root_availability(&mut self, cx: &mut Context<Self>) {
        let checked = self.availability.pending.as_ref().map(Receiver::try_recv);
        match checked {
            Some(Ok(checked)) => {
                self.availability.pending = None;
                for (root, available) in checked {
                    if !self.roots.iter().any(|current| current.path == root) {
                        continue;
                    }
                    if available {
                        self.forget_root_availability(&root);
                    } else {
                        self.mark_root_unavailable(&root);
                    }
                }
                if self.scan_session.is_none() {
                    if let Some(root) = self.availability.missing.first() {
                        self.scan_state = ScanState::Missing { root: root.clone() };
                    } else if matches!(self.scan_state, ScanState::Missing { .. }) {
                        self.scan_state = ScanState::Idle;
                    }
                }
                cx.notify();
            }
            Some(Err(TryRecvError::Disconnected)) => self.availability.pending = None,
            Some(Err(TryRecvError::Empty)) | None => {}
        }
        if Instant::now() >= self.availability.next_check {
            self.refresh_root_availability(cx);
        }
    }
}
