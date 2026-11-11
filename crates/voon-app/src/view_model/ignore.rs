//! Root-scoped ignore rules; changing a rule affects the next scan.

use std::path::{Component, Path, PathBuf};

use gpui::Context;

use super::{AppState, Result};

impl AppState {
    pub fn ignored_paths(&self, root: &Path) -> Result<Vec<PathBuf>> {
        Ok(self.index.ignored_paths(root)?)
    }

    pub fn add_ignored_path(
        &mut self,
        root: &Path,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        self.require_ignore_root(root)?;
        if path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err("请选择音乐库内的子文件夹，路径不能包含 ..".into());
        }
        let path = if path.is_absolute() {
            path
        } else {
            root.join(path)
        }
        .canonicalize()?;
        if !path.is_dir() {
            return Err("请选择要忽略的子文件夹".into());
        }
        self.index.add_ignored_path(root, &path)?;
        cx.notify();
        Ok(())
    }

    /// Removing a saved rule works while the NAS is offline.
    pub fn remove_ignored_path(
        &mut self,
        root: &Path,
        path: &Path,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        self.require_ignore_root(root)?;
        self.index.remove_ignored_path(root, path)?;
        cx.notify();
        Ok(())
    }

    fn require_ignore_root(&self, root: &Path) -> Result<()> {
        if !self.roots.iter().any(|existing| existing.path == root) {
            return Err("音乐库根目录不存在".into());
        }
        Ok(())
    }
}
