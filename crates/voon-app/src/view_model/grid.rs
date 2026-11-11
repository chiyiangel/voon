//! Responsive row access and shared cover resources for album browsing.

use super::AppState;
use gpui::{Context, Entity, RenderImage, Subscription, prelude::*};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    ops::Range,
    sync::Arc,
};
use voon_core::Cover;
use voon_core::db::AlbumRow;

pub const GRID_PADDING: f32 = 18.;
pub const GRID_GAP: f32 = 16.;
pub const CARD_MIN_WIDTH: f32 = 122.;

#[derive(Clone, Copy, Debug)]
pub struct GridLayout {
    pub columns: usize,
    pub rows: usize,
    pub card_width: f32,
}

impl GridLayout {
    fn range(self, row: usize, album_count: usize) -> Range<usize> {
        let start = row.saturating_mul(self.columns).min(album_count);
        start..start.saturating_add(self.columns).min(album_count)
    }
}

impl AppState {
    pub fn cover_revision(&self) -> u64 {
        self.cover_revision
    }

    pub(super) fn refresh_covers(&mut self) {
        self.cover_revision = self.cover_revision.wrapping_add(1);
    }

    pub fn grid_layout(&self, available_width: f32) -> GridLayout {
        let content_width = (available_width - 2. * GRID_PADDING).max(1.);
        let columns = ((content_width + GRID_GAP) / (CARD_MIN_WIDTH + GRID_GAP))
            .floor()
            .max(1.) as usize;
        GridLayout {
            columns,
            rows: self.browsed_albums().len().div_ceil(columns),
            card_width: ((content_width - GRID_GAP * (columns - 1) as f32) / columns as f32)
                .max(1.),
        }
    }

    pub fn grid_row(&self, row: usize, layout: GridLayout) -> &[AlbumRow] {
        &self.browsed_albums()[layout.range(row, self.browsed_albums().len())]
    }
}

#[derive(Clone)]
pub enum CoverStatus {
    Loading,
    Ready(Arc<RenderImage>),
    Unavailable,
}

struct CachedCover {
    identity: u64,
    status: CoverStatus,
    used: u64,
    started: bool,
}

/// A bounded thumbnail cache. Only requested (visible) covers are queued; up to
/// four background reads/decode jobs run together, away from GPUI rendering.
#[derive(Default)]
pub struct CoverCache {
    entries: HashMap<Cover, CachedCover>,
    queue: VecDeque<Cover>,
    active: HashSet<u64>,
    clock: u64,
    revision: u64,
    _state_subscription: Option<Subscription>,
}

impl CoverCache {
    /// Bind the shared cache to successful library reloads and root reconnection.
    /// `Default` remains available for standalone thumbnail consumers.
    pub fn new(state: &Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let revision = state.read(cx).cover_revision();
        let subscription = cx.observe(state, |this, state, cx| {
            let revision = state.read(cx).cover_revision();
            if this.revision != revision {
                this.revision = revision;
                this.entries.clear();
                this.queue.clear();
                // In-flight jobs still occupy their slots until completion.
                // Their revision/identity prevents them populating new entries.
                cx.notify();
            }
        });
        Self {
            revision,
            _state_subscription: Some(subscription),
            ..Self::default()
        }
    }

    pub fn status(&self, cover: &Cover) -> Option<CoverStatus> {
        self.entries.get(cover).map(|entry| entry.status.clone())
    }

    pub fn request(&mut self, cover: Cover, cx: &mut Context<Self>) -> CoverStatus {
        self.clock += 1;
        if let Some(entry) = self.entries.get_mut(&cover) {
            entry.used = self.clock;
            return entry.status.clone();
        }
        if self.entries.len() >= 256
            && let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.used)
                .map(|(key, _)| key.clone())
        {
            self.entries.remove(&oldest);
            self.queue.retain(|queued| queued != &oldest);
        }
        self.entries.insert(
            cover.clone(),
            CachedCover {
                identity: self.clock,
                status: CoverStatus::Loading,
                used: self.clock,
                started: false,
            },
        );
        self.queue.push_back(cover);
        self.start_next(cx);
        CoverStatus::Loading
    }

    fn start_next(&mut self, cx: &mut Context<Self>) {
        while self.active.len() < 4 {
            let Some(cover) = self.queue.pop_front() else {
                break;
            };
            let Some(entry) = self.entries.get_mut(&cover) else {
                continue;
            };
            if entry.started || !matches!(entry.status, CoverStatus::Loading) {
                continue;
            }
            entry.started = true;
            let identity = entry.identity;
            let revision = self.revision;
            self.active.insert(identity);
            let source = cover.clone();
            let task = cx.background_spawn(async move {
                let bytes = voon_core::tags::read_cover(&source).ok()?;
                let decoded = image::load_from_memory(&bytes).ok()?;
                let decoded = if decoded.width() > 256 || decoded.height() > 256 {
                    decoded.thumbnail(256, 256)
                } else {
                    decoded
                };
                let mut pixels = decoded.into_rgba8();
                for pixel in pixels.as_chunks_mut::<4>().0 {
                    pixel.swap(0, 2);
                }
                Some(Arc::new(RenderImage::new([image::Frame::new(pixels)])))
            });
            cx.spawn(async move |this, cx| {
                let image = task.await;
                let _ = this.update(cx, |this, cx| {
                    if !this.active.remove(&identity) {
                        return;
                    }
                    if this.revision == revision
                        && let Some(entry) = this.entries.get_mut(&cover)
                        && entry.identity == identity
                    {
                        entry.status = image
                            .map(CoverStatus::Ready)
                            .unwrap_or(CoverStatus::Unavailable);
                    }
                    this.start_next(cx);
                    cx.notify();
                });
            })
            .detach();
        }
    }
}
