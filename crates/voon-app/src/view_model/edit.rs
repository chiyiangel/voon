//! An edit draft stays bound to one Album path until saved or cancelled.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use gpui::Context;
use voon_core::{ALBUM_FIELDS, FieldSource};

use super::{AppState, Result};

#[derive(Clone, Debug)]
pub struct EditField {
    name: &'static str,
    value: String,
    source: Option<FieldSource>,
    original: String,
    original_source: Option<FieldSource>,
    automatic: String,
    automatic_source: Option<FieldSource>,
}
impl EditField {
    pub fn name(&self) -> &'static str {
        self.name
    }
    pub fn value(&self) -> &str {
        &self.value
    }
    pub fn source(&self) -> Option<FieldSource> {
        self.source
    }
}

#[derive(Clone, Debug)]
pub struct AlbumEditDraft {
    album: PathBuf,
    fields: Vec<EditField>,
    changes: BTreeMap<String, Option<String>>,
    error: Option<String>,
}
impl AlbumEditDraft {
    pub fn album_path(&self) -> &Path {
        &self.album
    }
    pub fn fields(&self) -> &[EditField] {
        &self.fields
    }
    pub fn field(&self, name: &str) -> Option<&EditField> {
        self.fields.iter().find(|field| field.name == name)
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

impl AppState {
    pub fn album_edit_draft(&self) -> Option<&AlbumEditDraft> {
        self.edit_draft.as_ref()
    }

    pub fn begin_album_edit(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let album = self.album_detail().ok_or("请先选择专辑")?;
        let automatic = self
            .index
            .automatic_metadata(&album.path)?
            .ok_or("请等待扫描保存后再编辑此专辑")?
            .entries();
        let current = album.metadata.entries();
        let fields = ALBUM_FIELDS
            .iter()
            .map(|name| {
                let original = current.iter().find(|(field, _, _)| field == name);
                let automatic = automatic.iter().find(|(field, _, _)| field == name);
                let value = original
                    .map(|(_, value, _)| value.clone())
                    .unwrap_or_default();
                let source = original.map(|(_, _, source)| *source);
                EditField {
                    name,
                    original: value.clone(),
                    value,
                    source,
                    original_source: source,
                    automatic: automatic
                        .map(|(_, value, _)| value.clone())
                        .unwrap_or_default(),
                    automatic_source: automatic.map(|(_, _, source)| *source),
                }
            })
            .collect();
        self.edit_draft = Some(AlbumEditDraft {
            album: album.path.clone(),
            fields,
            changes: BTreeMap::new(),
            error: None,
        });
        cx.notify();
        Ok(())
    }

    pub fn set_album_edit_field(
        &mut self,
        name: &str,
        value: String,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        let draft = self.edit_draft.as_mut().ok_or("没有正在编辑的专辑")?;
        let field = draft
            .fields
            .iter_mut()
            .find(|field| field.name == name)
            .ok_or("不支持的专辑字段")?;
        field.value = value;
        if field.value.trim() == field.original.trim() {
            draft.changes.remove(name);
            field.source = field.original_source;
        } else {
            let value = (!field.value.trim().is_empty()).then(|| field.value.clone());
            field.source = if value.is_some() {
                Some(FieldSource::UserEdit)
            } else {
                field.automatic_source
            };
            draft.changes.insert(name.into(), value);
        }
        draft.error = None;
        cx.notify();
        Ok(())
    }

    pub fn restore_album_edit_field(&mut self, name: &str, cx: &mut Context<Self>) -> Result<()> {
        let draft = self.edit_draft.as_mut().ok_or("没有正在编辑的专辑")?;
        let field = draft
            .fields
            .iter_mut()
            .find(|field| field.name == name)
            .ok_or("不支持的专辑字段")?;
        field.value = field.automatic.clone();
        field.source = field.automatic_source;
        draft.changes.insert(name.into(), None);
        draft.error = None;
        cx.notify();
        Ok(())
    }

    pub fn cancel_album_edit(&mut self, cx: &mut Context<Self>) {
        self.edit_draft = None;
        cx.notify();
    }

    pub fn save_album_edit(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let draft = self.edit_draft.as_ref().ok_or("没有正在编辑的专辑")?;
        let path = draft.album.clone();
        if self.selected_album.as_deref() != Some(path.as_path()) {
            self.edit_draft = None;
            cx.notify();
            return Err("专辑选择已变化，请重新打开编辑".into());
        }
        let changes = draft
            .changes
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect::<Vec<_>>();
        if let Err(error) = self.index.save_user_edits(&path, &changes) {
            self.edit_draft.as_mut().unwrap().error = Some(error.to_string());
            cx.notify();
            return Err(error.into());
        }
        self.edit_draft = None;
        self.reload_library(cx)
    }
}
