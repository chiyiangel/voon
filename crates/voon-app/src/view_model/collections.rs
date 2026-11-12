use super::{AppState, Navigation, Result};
use gpui::Context;
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    path::{Path, PathBuf},
};
use voon_core::{
    Collection,
    db::{AlbumRow, Index},
};

#[derive(Default)]
pub(super) struct CollectionState {
    sidebar_collapsed: bool,
    tree: Vec<Collection>,
    children: BTreeMap<Option<PathBuf>, Vec<usize>>,
    expanded: BTreeSet<PathBuf>,
    selected: Option<PathBuf>,
    members: Option<HashSet<PathBuf>>,
}

pub struct CollectionRow {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub album_count: usize,
    pub expanded: bool,
    pub has_children: bool,
    pub selected: bool,
}

impl CollectionState {
    pub(super) fn load(index: &Index) -> Result<Self> {
        let mut state = Self::default();
        state.replace_tree(index.collection_tree()?);
        Ok(state)
    }

    pub(super) fn replace_tree(&mut self, tree: Vec<Collection>) {
        let paths: HashSet<_> = tree
            .iter()
            .map(|collection| collection.path.clone())
            .collect();
        self.children.clear();
        for (position, collection) in tree.iter().enumerate() {
            let parent = collection
                .parent
                .clone()
                .filter(|parent| paths.contains(parent));
            self.children.entry(parent).or_default().push(position);
        }
        self.expanded.retain(|path| paths.contains(path));
        self.tree = tree;
        if let Some(path) = self.selected.clone() {
            if let Some(collection) = self.tree.iter().find(|collection| collection.path == path) {
                self.members = Some(collection.albums.iter().cloned().collect());
            } else {
                self.clear_selection();
            }
        }
    }

    pub(super) fn clear_selection(&mut self) {
        self.selected = None;
        self.members = None;
    }
}

impl AppState {
    pub fn collections_sidebar_expanded(&self) -> bool {
        !self.collections.sidebar_collapsed
    }

    pub fn toggle_collections_sidebar(&mut self, cx: &mut Context<Self>) {
        self.collections.sidebar_collapsed = !self.collections.sidebar_collapsed;
        if !self.collections.sidebar_collapsed {
            self.navigate(Navigation::Collections, cx);
        }
        cx.notify();
    }

    pub fn selected_collection(&self) -> Option<&Collection> {
        let selected = self.collections.selected.as_ref()?;
        self.collections
            .tree
            .iter()
            .find(|collection| &collection.path == selected)
    }

    pub fn collection_rows(&self) -> Vec<CollectionRow> {
        let mut rows = vec![];
        let mut stack: Vec<_> = self
            .collections
            .children
            .get(&None)
            .into_iter()
            .flatten()
            .rev()
            .map(|&position| (position, 0))
            .collect();
        let mut visited = BTreeSet::new();
        while let Some((position, depth)) = stack.pop() {
            let collection = &self.collections.tree[position];
            if !visited.insert(position) {
                continue;
            }
            let expanded = self.collections.expanded.contains(&collection.path);
            let children = self
                .collections
                .children
                .get(&Some(collection.path.clone()));
            rows.push(CollectionRow {
                path: collection.path.clone(),
                name: collection.name.clone(),
                depth,
                album_count: collection.albums.len(),
                expanded,
                has_children: children.is_some_and(|children| !children.is_empty()),
                selected: self.collections.selected.as_ref() == Some(&collection.path),
            });
            if expanded && let Some(children) = children {
                stack.extend(children.iter().rev().map(|&position| (position, depth + 1)));
            }
        }
        rows
    }

    pub fn toggle_collection(&mut self, path: &Path, cx: &mut Context<Self>) {
        if !self.collections.expanded.remove(path) {
            self.collections.expanded.insert(path.into());
        }
        cx.notify();
    }

    pub fn select_collection(
        &mut self,
        path: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        if let Some(path) = path {
            let collection = self
                .collections
                .tree
                .iter()
                .find(|collection| collection.path == path)
                .ok_or("套装不存在")?;
            self.collections.members = Some(collection.albums.iter().cloned().collect());
            self.collections.selected = Some(path);
        } else {
            self.collections.clear_selection();
        }
        self.navigation = Navigation::Collections;
        self.clear_album_selection();
        self.refresh_browse();
        cx.notify();
        Ok(())
    }

    pub(super) fn matches_collection(&self, album: &AlbumRow) -> bool {
        self.collections
            .members
            .as_ref()
            .is_none_or(|members| members.contains(&album.path))
    }
}
