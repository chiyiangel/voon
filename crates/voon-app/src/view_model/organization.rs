//! Cached local tags, manual collections and multi-album selection.

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    path::{Path, PathBuf},
};

use gpui::Context;
use voon_core::db::{AlbumRow, Index, ManualCollectionRow, UserTagRow};

use super::{AppState, Navigation, Result, SortOrder};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrganizationScope {
    Tag(i64),
    ManualCollection(i64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrganizationDialog {
    NewTag,
    AddTag,
    RemoveTag,
    NewCollection,
    AddToCollection,
    Rename(OrganizationScope),
    Delete(OrganizationScope),
}

#[derive(Default)]
pub(super) struct OrganizationState {
    selected: BTreeSet<PathBuf>,
    anchor: Option<PathBuf>,
    tags: Vec<UserTagRow>,
    collections: Vec<ManualCollectionRow>,
    scope: Option<OrganizationScope>,
    members: HashSet<PathBuf>,
    pub(super) rank: HashMap<PathBuf, usize>,
    dialog: Option<OrganizationDialog>,
    draft: String,
}

impl OrganizationState {
    fn refresh_scope(&mut self) {
        let paths = match self.scope {
            Some(OrganizationScope::Tag(id)) => self
                .tags
                .iter()
                .find(|tag| tag.id == id)
                .map(|tag| &tag.albums),
            Some(OrganizationScope::ManualCollection(id)) => self
                .collections
                .iter()
                .find(|collection| collection.id == id)
                .map(|collection| &collection.albums),
            None => None,
        };
        self.members = paths.into_iter().flatten().cloned().collect();
        self.rank = paths
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(rank, path)| (path.clone(), rank))
            .collect();
    }

    pub(super) fn load(index: &Index) -> Result<Self> {
        Ok(Self {
            tags: index.user_tag_catalog()?,
            collections: index.manual_collections()?,
            ..Self::default()
        })
    }

    pub(super) fn select_single(&mut self, path: Option<&Path>) {
        self.selected.clear();
        if let Some(path) = path {
            self.selected.insert(path.to_path_buf());
        }
        self.anchor = path.map(Path::to_path_buf);
    }
}

impl AppState {
    /// All navigation entry points release the primary item, batch selection
    /// and detail together. Grid/list mode changes deliberately keep them.
    pub(super) fn clear_album_selection(&mut self) {
        self.selected_album = None;
        self.organization.select_single(None);
        self.clear_detail();
    }

    pub fn organization_dialog(&self) -> Option<OrganizationDialog> {
        self.organization.dialog
    }
    pub fn organization_draft(&self) -> &str {
        &self.organization.draft
    }
    pub fn set_organization_draft(&mut self, value: String, cx: &mut Context<Self>) {
        self.organization.draft = value;
        cx.notify();
    }

    pub fn open_organization_dialog(
        &mut self,
        dialog: OrganizationDialog,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        self.organization.draft = match dialog {
            OrganizationDialog::Rename(scope) => self
                .organization_name(scope)
                .ok_or("名称不存在")?
                .to_string(),
            _ => String::new(),
        };
        self.organization.dialog = Some(dialog);
        self.error = None;
        cx.notify();
        Ok(())
    }

    pub fn close_organization_dialog(&mut self, cx: &mut Context<Self>) {
        self.organization.dialog = None;
        self.organization.draft.clear();
        cx.notify();
    }

    pub fn confirm_organization_dialog(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let Some(dialog) = self.organization.dialog else {
            return Ok(());
        };
        let name = self.organization.draft.trim().to_string();
        match dialog {
            OrganizationDialog::NewTag => {
                self.index.create_user_tag(&name)?;
            }
            OrganizationDialog::AddTag => {
                self.set_tag_on_selection(&name, true, cx)?;
            }
            OrganizationDialog::RemoveTag => {
                self.set_tag_on_selection(&name, false, cx)?;
            }
            OrganizationDialog::NewCollection => {
                let collection = self.index.create_manual_collection(&name)?;
                let selected = self.selected_album_paths();
                if !selected.is_empty() {
                    self.index
                        .set_manual_collection_for_albums(collection, &selected, true)?;
                }
            }
            OrganizationDialog::AddToCollection => {
                let collection = self
                    .organization
                    .collections
                    .iter()
                    .find(|collection| collection.name == name)
                    .ok_or("请选择已有手动合集")?
                    .id;
                self.set_selected_manual_collection(collection, true, cx)?;
            }
            OrganizationDialog::Rename(OrganizationScope::Tag(id)) => {
                let old = self
                    .organization_name(OrganizationScope::Tag(id))
                    .ok_or("标签不存在")?
                    .to_string();
                self.index.rename_user_tag(&old, &name)?;
            }
            OrganizationDialog::Rename(OrganizationScope::ManualCollection(id)) => {
                self.index.rename_manual_collection(id, &name)?
            }
            OrganizationDialog::Delete(OrganizationScope::Tag(id)) => {
                let name = self
                    .organization_name(OrganizationScope::Tag(id))
                    .ok_or("标签不存在")?
                    .to_string();
                self.index.delete_user_tag(&name)?;
            }
            OrganizationDialog::Delete(OrganizationScope::ManualCollection(id)) => {
                self.index.delete_manual_collection(id)?
            }
        }
        self.organization_changed(cx)?;
        self.close_organization_dialog(cx);
        Ok(())
    }

    pub fn organization_name(&self, scope: OrganizationScope) -> Option<&str> {
        match scope {
            OrganizationScope::Tag(id) => self
                .organization
                .tags
                .iter()
                .find(|tag| tag.id == id)
                .map(|tag| tag.name.as_str()),
            OrganizationScope::ManualCollection(id) => self
                .organization
                .collections
                .iter()
                .find(|collection| collection.id == id)
                .map(|collection| collection.name.as_str()),
        }
    }

    pub fn organization_album_count(&self, scope: OrganizationScope) -> usize {
        let members = match scope {
            OrganizationScope::Tag(id) => self
                .organization
                .tags
                .iter()
                .find(|tag| tag.id == id)
                .map(|tag| &tag.albums),
            OrganizationScope::ManualCollection(id) => self
                .organization
                .collections
                .iter()
                .find(|collection| collection.id == id)
                .map(|collection| &collection.albums),
        };
        members
            .map(|members| {
                self.albums
                    .iter()
                    .filter(|album| members.contains(&album.path))
                    .count()
            })
            .unwrap_or_default()
    }

    pub fn album_user_tags(&self, path: &Path) -> Vec<&UserTagRow> {
        self.organization
            .tags
            .iter()
            .filter(|tag| tag.albums.iter().any(|member| member == path))
            .collect()
    }
    pub fn album_manual_collections(&self, path: &Path) -> Vec<&ManualCollectionRow> {
        self.organization
            .collections
            .iter()
            .filter(|collection| collection.albums.iter().any(|member| member == path))
            .collect()
    }

    pub fn set_tag_for_album(
        &mut self,
        path: &Path,
        name: &str,
        present: bool,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        self.index
            .set_user_tag_for_albums(&[path.to_path_buf()], name, present)?;
        self.organization_changed(cx)
    }

    pub fn set_manual_collection_for_album(
        &mut self,
        collection: i64,
        path: &Path,
        present: bool,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        self.index
            .set_manual_collection_for_albums(collection, &[path.to_path_buf()], present)?;
        self.organization_changed(cx)
    }

    pub fn create_manual_collection(&mut self, name: &str, cx: &mut Context<Self>) -> Result<i64> {
        let collection = self.index.create_manual_collection(name)?;
        self.organization_changed(cx)?;
        Ok(collection)
    }

    pub fn set_selected_manual_collection(
        &mut self,
        collection: i64,
        present: bool,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        let selected = self.selected_album_paths();
        if selected.is_empty() {
            return Err("请先选择专辑".into());
        }
        self.index
            .set_manual_collection_for_albums(collection, &selected, present)?;
        self.organization_changed(cx)
    }

    pub fn move_manual_album(
        &mut self,
        collection: i64,
        path: &Path,
        offset: isize,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        let mut members = self
            .organization
            .collections
            .iter()
            .find(|candidate| candidate.id == collection)
            .ok_or("手动合集不存在")?
            .albums
            .clone();
        let visible: Vec<_> = members
            .iter()
            .enumerate()
            .filter(|(_, path)| self.albums.iter().any(|album| &album.path == *path))
            .map(|(position, path)| (position, path.clone()))
            .collect();
        let position = visible
            .iter()
            .position(|(_, member)| member == path)
            .ok_or("专辑不在手动合集中")?;
        let destination = position
            .saturating_add_signed(offset)
            .min(visible.len().saturating_sub(1));
        members.swap(visible[position].0, visible[destination].0);
        self.index
            .set_manual_collection_order(collection, &members)?;
        self.sort_order = SortOrder::Manual;
        self.reset_sort_direction();
        self.organization_changed(cx)
    }

    pub fn user_tag_catalog(&self) -> &[UserTagRow] {
        &self.organization.tags
    }
    pub fn manual_collections(&self) -> &[ManualCollectionRow] {
        &self.organization.collections
    }
    pub fn organization_scope(&self) -> Option<OrganizationScope> {
        self.organization.scope
    }
    pub fn selected_album_paths(&self) -> Vec<PathBuf> {
        self.organization.selected.iter().cloned().collect()
    }
    pub fn is_album_selected(&self, path: &Path) -> bool {
        self.organization.selected.contains(path)
    }

    pub fn click_album(
        &mut self,
        path: PathBuf,
        extend: bool,
        range: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.albums.iter().any(|album| album.path == path) {
            return;
        }
        if !extend && !range {
            self.select_album(Some(path), cx);
            return;
        }
        let anchor = self.organization.anchor.clone();
        let mut selected = self.organization.selected.clone();
        if range {
            let from = anchor.as_ref().and_then(|anchor| {
                self.browsed_albums()
                    .iter()
                    .position(|album| &album.path == anchor)
            });
            let to = self
                .browsed_albums()
                .iter()
                .position(|album| album.path == path);
            if !extend {
                selected.clear();
            }
            if let (Some(from), Some(to)) = (from, to) {
                selected.extend(
                    self.browsed_albums()[from.min(to)..=from.max(to)]
                        .iter()
                        .map(|album| album.path.clone()),
                );
            } else {
                selected.insert(path.clone());
            }
        } else if !selected.remove(&path) {
            selected.insert(path.clone());
        }
        let primary = if selected.contains(&path) {
            Some(path)
        } else {
            selected.iter().next_back().cloned()
        };
        self.select_album(primary, cx);
        self.organization.selected = selected;
        self.organization.anchor = anchor.or_else(|| self.selected_album.clone());
        cx.notify();
    }

    pub fn select_organization(
        &mut self,
        scope: Option<OrganizationScope>,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        match scope {
            Some(OrganizationScope::Tag(id)) => {
                if !self.organization.tags.iter().any(|tag| tag.id == id) {
                    return Err("标签不存在".into());
                }
                self.navigation = Navigation::UserTags;
            }
            Some(OrganizationScope::ManualCollection(id)) => {
                if !self
                    .organization
                    .collections
                    .iter()
                    .any(|collection| collection.id == id)
                {
                    return Err("手动合集不存在".into());
                }
                self.navigation = Navigation::ManualCollections;
            }
            None => {}
        }
        self.organization.scope = scope;
        self.clear_album_selection();
        self.organization.refresh_scope();
        if matches!(scope, Some(OrganizationScope::ManualCollection(_))) {
            self.sort_order = SortOrder::Manual;
            self.reset_sort_direction();
        } else if self.sort_order == SortOrder::Manual {
            self.sort_order = SortOrder::RecentlyAdded;
            self.reset_sort_direction();
        }
        self.refresh_browse();
        cx.notify();
        Ok(())
    }

    pub fn set_tag_on_selection(
        &mut self,
        name: &str,
        present: bool,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        let selected = self.selected_album_paths();
        if selected.is_empty() {
            return Err("请先选择专辑".into());
        }
        self.index
            .set_user_tag_for_albums(&selected, name, present)?;
        self.organization_changed(cx)
    }

    fn organization_changed(&mut self, cx: &mut Context<Self>) -> Result<()> {
        self.refresh_organization()?;
        self.refresh_browse();
        cx.notify();
        Ok(())
    }

    pub(super) fn refresh_organization(&mut self) -> Result<()> {
        let tags = self.index.user_tag_catalog()?;
        let collections = self.index.manual_collections()?;
        self.organization.tags = tags;
        self.organization.collections = collections;
        self.organization
            .selected
            .retain(|path| self.albums.iter().any(|album| &album.path == path));
        let exists = match self.organization.scope {
            Some(OrganizationScope::Tag(id)) => {
                self.organization.tags.iter().any(|tag| tag.id == id)
            }
            Some(OrganizationScope::ManualCollection(id)) => self
                .organization
                .collections
                .iter()
                .any(|collection| collection.id == id),
            None => true,
        };
        if !exists {
            self.organization.scope = None;
            if self.sort_order == SortOrder::Manual {
                self.sort_order = SortOrder::RecentlyAdded;
                self.reset_sort_direction();
            }
        }
        self.organization.refresh_scope();
        Ok(())
    }

    pub(super) fn matches_organization(&self, album: &AlbumRow) -> bool {
        match (self.navigation, self.organization.scope) {
            (Navigation::UserTags, Some(OrganizationScope::Tag(_)))
            | (Navigation::ManualCollections, Some(OrganizationScope::ManualCollection(_))) => {
                self.organization.members.contains(&album.path)
            }
            _ => true,
        }
    }
}
