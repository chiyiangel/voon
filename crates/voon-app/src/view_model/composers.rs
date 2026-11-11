use super::{AppState, Navigation, Result};
use gpui::Context;
use std::collections::BTreeMap;
use voon_core::db::AlbumRow;

pub struct ComposerOption {
    pub name: String,
    pub count: usize,
}

#[derive(Default)]
pub(super) struct ComposerState {
    selected: Option<String>,
    options: Vec<ComposerOption>,
}

impl ComposerState {
    pub(super) fn clear_selection(&mut self) {
        self.selected = None;
    }
}

fn name(album: &AlbumRow) -> Option<&str> {
    album
        .metadata
        .composer
        .as_ref()
        .map(|field| field.value.trim())
        .filter(|name| !name.is_empty())
}

impl AppState {
    pub fn composer_options(&self) -> &[ComposerOption] {
        &self.composers.options
    }
    pub fn selected_composer(&self) -> Option<&str> {
        self.composers.selected.as_deref()
    }

    pub fn select_composer(
        &mut self,
        composer: Option<String>,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        if let Some(composer) = &composer
            && !self
                .albums
                .iter()
                .any(|album| name(album) == Some(composer.as_str()))
        {
            return Err("作曲家不存在".into());
        }
        self.composers.selected = composer;
        self.navigation = Navigation::Composers;
        self.clear_album_selection();
        self.refresh_browse();
        cx.notify();
        Ok(())
    }

    pub(super) fn matches_composer(&self, album: &AlbumRow) -> bool {
        self.composers
            .selected
            .as_deref()
            .is_none_or(|composer| name(album) == Some(composer))
    }

    pub(super) fn refresh_composer_counts(&mut self) {
        let mut counts = BTreeMap::new();
        for album in &self.albums {
            if let Some(name) = name(album) {
                let count = counts.entry(name.to_string()).or_insert(0);
                if self.matches_collection(album)
                    && self.matches_search(album)
                    && self.matches_facets(album)
                    && self.matches_organization(album)
                {
                    *count += 1;
                }
            }
        }
        if let Some(selected) = &self.composers.selected {
            counts.entry(selected.clone()).or_insert(0);
        }
        self.composers.options = counts
            .into_iter()
            .map(|(name, count)| ComposerOption { name, count })
            .collect();
    }
}
