//! Startup failures remain visible without making an incompatible file writable.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use gpui::Context;
use voon_core::db::Index;

use super::{AppState, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexOpenProblem {
    pub path: Option<PathBuf>,
    pub message: String,
}

#[derive(Default)]
pub(super) struct LibraryIndexState {
    problem: Option<IndexOpenProblem>,
    selection_file: Option<PathBuf>,
    notice: Option<String>,
}

impl AppState {
    /// Open the requested local index; a failure becomes visible application
    /// state rather than terminating the process or rebuilding that file.
    pub fn open_index(
        path: PathBuf,
        selection_file: Option<PathBuf>,
        suggested_root: PathBuf,
    ) -> Result<Self> {
        let selected = selected_path(&path, selection_file.as_deref());
        let opened = selected
            .as_ref()
            .map_err(|error| error.to_string())
            .and_then(|path| {
                if path.starts_with(&suggested_root)
                    || normalized_path(path).is_ok_and(|path| path.starts_with(&suggested_root))
                {
                    return Err("本地索引不能保存在音乐库文件夹内，请选择本机其他位置".into());
                }
                Index::open(path)
                    .map_err(|error| error.to_string())
                    .and_then(|index| {
                        Self::new(index, suggested_root.clone()).map_err(|error| error.to_string())
                    })
            });
        let mut state = match opened {
            Ok(state) => state,
            Err(error) => {
                Self::unavailable_index(Some(selected.unwrap_or(path)), suggested_root, error)?
            }
        };
        state.library_index.selection_file = selection_file;
        Ok(state)
    }

    pub fn unavailable_index(
        path: Option<PathBuf>,
        suggested_root: PathBuf,
        error: impl std::fmt::Display,
    ) -> Result<Self> {
        let mut state = Self::new(Index::open_in_memory()?, suggested_root)?;
        state.library_index.problem = Some(IndexOpenProblem {
            path,
            message: format!(
                "无法打开本地资料库：{error}\n原资料库文件已保留。可新建索引重新扫描。"
            ),
        });
        Ok(state)
    }

    pub fn index_open_problem(&self) -> Option<&IndexOpenProblem> {
        self.library_index.problem.as_ref()
    }

    pub fn index_path(&self) -> Option<&Path> {
        self.index.database_path()
    }

    pub fn index_notice(&self) -> Option<&str> {
        self.library_index.notice.as_deref()
    }

    /// Switch only after a separate new Index is created successfully. The
    /// old file stays in place and no current scan can publish into the new one.
    pub fn create_new_index(&mut self, path: PathBuf, cx: &mut Context<Self>) -> Result<()> {
        if self.scan_session.is_some() {
            return Err("请先等待当前扫描结束，再新建索引".into());
        }
        let within_suggested_root = path.starts_with(&self.suggested_root);
        let path = normalized_path(&path)?;
        if within_suggested_root
            || path.starts_with(&self.suggested_root)
            || self.roots.iter().any(|root| path.starts_with(&root.path))
        {
            return Err("本地索引不能保存在音乐库文件夹内，请选择本机其他位置".into());
        }
        if self
            .library_index
            .selection_file
            .as_ref()
            .is_some_and(|file| normalized_path(file).is_ok_and(|selection| path == selection))
        {
            return Err("此文件名由应用设置使用，请选择其他索引文件名".into());
        }
        let path_text = path.to_str().ok_or("索引路径必须使用有效的 Unicode 字符")?;
        let index = Index::create(&path)?;
        let mut replacement = Self::new(index, self.suggested_root.clone())?;
        if let Some(selection_file) = &self.library_index.selection_file {
            let directory = selection_file.parent().ok_or("无法保存索引位置")?;
            let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
            temporary.write_all(path_text.as_bytes())?;
            temporary.as_file().sync_all()?;
            temporary.persist(selection_file)?;
        }
        replacement.library_index.selection_file = self.library_index.selection_file.clone();
        replacement.library_index.notice =
            Some("已创建新的本地索引，原资料库文件已保留。请添加音乐库文件夹重新扫描。".into());
        *self = replacement;
        cx.notify();
        Ok(())
    }
}

fn normalized_path(path: &Path) -> Result<PathBuf> {
    if path.try_exists()? {
        return Ok(path.canonicalize()?);
    }
    let filename = path.file_name().ok_or("请选择新的索引文件名")?;
    let parent = path
        .parent()
        .ok_or("请选择本机索引保存位置")?
        .canonicalize()?;
    Ok(parent.join(filename))
}

fn selected_path(default: &Path, selection_file: Option<&Path>) -> Result<PathBuf> {
    let Some(selection_file) = selection_file else {
        return Ok(default.to_path_buf());
    };
    let selection = match fs::read_to_string(selection_file) {
        Ok(selection) => PathBuf::from(selection),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(default.to_path_buf());
        }
        Err(error) => return Err(error.into()),
    };
    if !selection.is_absolute() {
        return Err("已保存的本地索引位置无效，请选择新位置创建索引".into());
    }
    Ok(selection)
}
