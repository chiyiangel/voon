//! Platform differences and process inputs stay at the application's edge.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathAction {
    Open,
    Reveal,
}

/// Injected by state tests; native process launching stays at this edge.
pub trait SystemOpener: Send + Sync {
    fn open(&self, path: &Path, action: PathAction) -> std::io::Result<()>;
}

pub struct NativeSystemOpener;
impl SystemOpener for NativeSystemOpener {
    fn open(&self, path: &Path, action: PathAction) -> std::io::Result<()> {
        use std::process::{Command, Stdio};
        let path = path.canonicalize()?;
        #[cfg(target_os = "macos")]
        let mut command = {
            let mut command = Command::new("open");
            if action == PathAction::Reveal {
                command.arg("-R");
            }
            command.arg("--").arg(&path);
            command
        };
        #[cfg(not(target_os = "macos"))]
        let mut command = {
            let target = if action == PathAction::Reveal && path.is_file() {
                path.parent().unwrap_or(&path)
            } else {
                &path
            };
            let mut command = Command::new("xdg-open");
            command.arg(target);
            command
        };
        let status = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other("系统未能打开此路径"))
        }
    }
}

#[cfg(target_os = "macos")]
fn default_root(_home: Option<PathBuf>) -> PathBuf {
    PathBuf::from("/Volumes/NAS1/Music")
}

#[cfg(not(target_os = "macos"))]
fn default_root(home: Option<PathBuf>) -> PathBuf {
    home.unwrap_or_else(|| PathBuf::from("/")).join("Music")
}

/// CLI argument, then `VOON_ROOT`, then the platform default.
pub fn resolve_root(arg: Option<String>, env: Option<String>, home: Option<PathBuf>) -> PathBuf {
    arg.or(env)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| default_root(home))
}

pub fn library_root() -> PathBuf {
    resolve_root(
        std::env::args().nth(1),
        std::env::var("VOON_ROOT").ok(),
        std::env::var_os("HOME").map(PathBuf::from),
    )
}

pub fn index_path() -> std::io::Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| std::io::Error::other("无法确定本机用户目录"))?;
    #[cfg(target_os = "macos")]
    let directory = home.join("Library/Application Support/Voon");
    #[cfg(not(target_os = "macos"))]
    let directory = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join(".local/share"))
        .join("voon");
    std::fs::create_dir_all(&directory)?;
    Ok(directory.join("library.sqlite3"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argument_beats_env_beats_default() {
        let home = Some(PathBuf::from("/home/u"));
        assert_eq!(
            resolve_root(Some("/a".into()), Some("/b".into()), home.clone()),
            PathBuf::from("/a")
        );
        assert_eq!(
            resolve_root(None, Some("/b".into()), home.clone()),
            PathBuf::from("/b")
        );
        let default = resolve_root(None, Some(String::new()), home);
        if cfg!(target_os = "macos") {
            assert_eq!(default, PathBuf::from("/Volumes/NAS1/Music"));
        } else {
            assert_eq!(default, PathBuf::from("/home/u/Music"));
        }
    }
}
