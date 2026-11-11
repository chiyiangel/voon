//! Confirmed local grouping changes; drafts never mutate the index.
use super::{AppState, Navigation, Result};
use gpui::Context;
use std::{collections::BTreeSet, path::PathBuf};
use voon_core::{
    Album,
    db::{DiscRef, Index, SplitMode, StructureOperation, TrackRef},
};

#[derive(Clone, Debug)]
pub enum StructureDraft {
    Merge {
        albums: Vec<Album>,
        primary: PathBuf,
        order: Vec<DiscRef>,
    },
    Split {
        album: Box<Album>,
        by_tracks: bool,
        tracks: BTreeSet<TrackRef>,
    },
    Undo(StructureOperation),
}

pub(super) struct StructureState {
    pub draft: Option<StructureDraft>,
    pub last: Option<StructureOperation>,
}

impl StructureState {
    pub fn load(index: &Index) -> Result<Self> {
        Ok(Self {
            draft: None,
            last: index.last_structure_operation()?,
        })
    }
}

impl AppState {
    pub fn structure_draft(&self) -> Option<&StructureDraft> {
        self.structure.draft.as_ref()
    }
    pub fn last_structure_operation(&self) -> Option<&StructureOperation> {
        self.structure.last.as_ref()
    }

    fn ensure_structure_idle(&self) -> Result<()> {
        if self.scan_session.is_some() {
            Err("请等待扫描结束，再调整专辑分组".into())
        } else {
            Ok(())
        }
    }

    pub fn begin_merge(&mut self, cx: &mut Context<Self>) -> Result<()> {
        self.ensure_structure_idle()?;
        let paths = self.selected_album_paths();
        if paths.len() < 2 {
            return Err("请在音乐库中用 ⌘ / Ctrl 点击选择至少两张专辑".into());
        }
        let albums = paths
            .iter()
            .map(|path| {
                self.index
                    .album(path)?
                    .ok_or_else(|| "专辑已不存在，请重新选择".into())
            })
            .collect::<Result<Vec<_>>>()?;
        let order = albums
            .iter()
            .flat_map(|album| {
                (0..album.discs.len()).map(|disc| DiscRef {
                    album: album.path.clone(),
                    disc,
                })
            })
            .collect();
        self.structure.draft = Some(StructureDraft::Merge {
            primary: paths[0].clone(),
            albums,
            order,
        });
        self.error = None;
        cx.notify();
        Ok(())
    }

    pub fn choose_merge_primary(&mut self, primary: PathBuf, cx: &mut Context<Self>) -> Result<()> {
        let Some(StructureDraft::Merge {
            albums,
            primary: current,
            ..
        }) = &mut self.structure.draft
        else {
            return Err("尚未开始合并".into());
        };
        if !albums.iter().any(|album| album.path == primary) {
            return Err("主信息源必须属于本次合并".into());
        }
        *current = primary;
        cx.notify();
        Ok(())
    }

    pub fn move_merge_disc(
        &mut self,
        from: usize,
        offset: isize,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        let Some(StructureDraft::Merge { order, .. }) = &mut self.structure.draft else {
            return Err("尚未开始合并".into());
        };
        let to = from.saturating_add_signed(offset);
        if from < order.len() && to < order.len() {
            order.swap(from, to);
        }
        cx.notify();
        Ok(())
    }

    pub fn begin_split(&mut self, cx: &mut Context<Self>) -> Result<()> {
        self.ensure_structure_idle()?;
        let path = self.selected_album.as_ref().ok_or("请先选择要拆分的专辑")?;
        let album = self.index.album(path)?.ok_or("专辑已不存在")?;
        self.structure.draft = Some(StructureDraft::Split {
            album: Box::new(album),
            by_tracks: false,
            tracks: BTreeSet::new(),
        });
        self.error = None;
        cx.notify();
        Ok(())
    }

    pub fn set_split_by_tracks(&mut self, value: bool, cx: &mut Context<Self>) -> Result<()> {
        let Some(StructureDraft::Split { by_tracks, .. }) = &mut self.structure.draft else {
            return Err("尚未开始拆分".into());
        };
        *by_tracks = value;
        cx.notify();
        Ok(())
    }

    pub fn toggle_split_track(&mut self, track: TrackRef, cx: &mut Context<Self>) -> Result<()> {
        let Some(StructureDraft::Split { album, tracks, .. }) = &mut self.structure.draft else {
            return Err("尚未开始拆分".into());
        };
        if album
            .discs
            .get(track.disc)
            .and_then(|disc| disc.tracks.get(track.track))
            .is_none()
        {
            return Err("曲目不存在".into());
        }
        if !tracks.remove(&track) {
            tracks.insert(track);
        }
        cx.notify();
        Ok(())
    }

    pub fn begin_structure_undo(&mut self, cx: &mut Context<Self>) -> Result<()> {
        self.ensure_structure_idle()?;
        let last = self
            .index
            .last_structure_operation()?
            .ok_or("没有可以撤销的合并或拆分")?;
        self.structure.draft = Some(StructureDraft::Undo(last));
        self.error = None;
        cx.notify();
        Ok(())
    }

    pub fn cancel_structure(&mut self, cx: &mut Context<Self>) {
        self.structure.draft = None;
        self.error = None;
        cx.notify();
    }

    pub fn confirm_structure(&mut self, cx: &mut Context<Self>) -> Result<()> {
        self.ensure_structure_idle()?;
        let draft = self
            .structure
            .draft
            .clone()
            .ok_or("没有等待确认的分组操作")?;
        let results = match draft {
            StructureDraft::Merge {
                albums,
                primary,
                order,
            } => {
                let inputs = albums
                    .into_iter()
                    .map(|album| album.path)
                    .collect::<Vec<_>>();
                vec![self.index.merge_albums(&inputs, &primary, &order)?]
            }
            StructureDraft::Split {
                album,
                by_tracks,
                tracks,
            } => self.index.split_album(
                &album.path,
                if by_tracks {
                    SplitMode::SelectedTracks(tracks.into_iter().collect())
                } else {
                    SplitMode::Discs
                },
            )?,
            StructureDraft::Undo(expected) => {
                if self
                    .index
                    .last_structure_operation()?
                    .is_none_or(|last| last.id != expected.id)
                {
                    return Err("分组历史已变化，请重新打开撤销确认".into());
                }
                self.index.undo_structure()?.unwrap_or_default()
            }
        };
        self.structure.draft = None;
        self.error = None;
        self.reload_library(cx)?;
        self.navigate(Navigation::AllAlbums, cx);
        self.settings_open = false;
        self.select_album(
            results
                .into_iter()
                .find(|path| self.albums.iter().any(|album| album.path == *path)),
            cx,
        );
        Ok(())
    }
}
