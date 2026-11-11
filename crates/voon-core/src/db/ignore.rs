//! User-owned, root-relative subtrees excluded from subsequent scans.

use std::path::{Path, PathBuf};

use rusqlite::params;

use super::{Index, Result, p};

fn relative_path(root: &Path, path: &Path) -> Result<PathBuf> {
    crate::scan::relative_ignored_path(root, path).ok_or_else(|| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "请选择此音乐库根目录内的子文件夹；不能忽略根目录或使用 ..",
        )))
    })
}

pub(super) const MIGRATION: &str = r#"
CREATE TABLE ignored_path (
    root_path TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    PRIMARY KEY (root_path, relative_path)
);
"#;

impl Index {
    /// Relative subfolder paths, scoped to this Library root. Rules remain
    /// available when a root is removed and later added again.
    pub fn ignored_paths(&self, root: &Path) -> Result<Vec<PathBuf>> {
        let mut statement = self.conn.prepare(
            "SELECT relative_path FROM ignored_path WHERE root_path = ?1 ORDER BY relative_path",
        )?;
        statement
            .query_map([p(root)], |row| Ok(PathBuf::from(row.get::<_, String>(0)?)))?
            .collect()
    }

    /// Exclude a subfolder on the next scan; accepts a relative path or an
    /// absolute child of `root`. The music library is never written to.
    pub fn add_ignored_path(&self, root: &Path, path: &Path) -> Result<()> {
        let relative = relative_path(root, path)?;
        self.conn.execute(
            "INSERT OR IGNORE INTO ignored_path (root_path, relative_path) VALUES (?1, ?2)",
            params![p(root), p(&relative)],
        )?;
        Ok(())
    }

    pub fn remove_ignored_path(&self, root: &Path, path: &Path) -> Result<()> {
        let relative = relative_path(root, path)?;
        self.conn.execute(
            "DELETE FROM ignored_path WHERE root_path = ?1 AND relative_path = ?2",
            params![p(root), p(&relative)],
        )?;
        Ok(())
    }
}
