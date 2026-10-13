use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::Deserialize;

const DEFAULT_LISTEN: &str = "0.0.0.0:8876";
const DEFAULT_SCAN_INTERVAL_SECS: u64 = 3600;

/// Process configuration loaded from a TOML file.
///
/// `database_path` should be on local disk. The music files themselves live
/// under `library_path`, which is typically an SMB or NFS mount.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub library_path: PathBuf,
    pub database_path: PathBuf,
    /// CoreAudio UID or device name. `None` selects the system default output.
    pub output_device: Option<String>,
    pub listen: SocketAddr,
    /// Seconds between incremental library scans. `0` disables the timer.
    /// Scanning itself is not part of this slice.
    pub scan_interval_secs: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse config {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("invalid listen address '{value}' in {path}")]
    Listen { path: PathBuf, value: String },
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    library_path: PathBuf,
    database_path: PathBuf,
    #[serde(default)]
    output_device: Option<String>,
    #[serde(default = "default_listen")]
    listen: String,
    #[serde(default = "default_scan_interval")]
    scan_interval_secs: u64,
}

fn default_listen() -> String {
    DEFAULT_LISTEN.to_string()
}

fn default_scan_interval() -> u64 {
    DEFAULT_SCAN_INTERVAL_SECS
}

pub fn load_config(path: impl AsRef<Path>) -> Result<Config, ConfigError> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let file: ConfigFile = toml::from_str(&text).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })?;
    let listen: SocketAddr = file.listen.parse().map_err(|_| ConfigError::Listen {
        path: path.to_path_buf(),
        value: file.listen.clone(),
    })?;
    let output_device = file
        .output_device
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    Ok(Config {
        library_path: file.library_path,
        database_path: file.database_path,
        output_device,
        listen,
        scan_interval_secs: file.scan_interval_secs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_required_fields_and_defaults() {
        let dir = std::env::temp_dir().join(format!("voon-config-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("voon.toml");
        fs::write(
            &path,
            "\
library_path = \"/Volumes/Music\"
database_path = \"/tmp/voon/library.db\"
output_device = \"  \"
",
        )
        .unwrap();

        let config = load_config(&path).unwrap();
        assert_eq!(config.library_path, PathBuf::from("/Volumes/Music"));
        assert_eq!(config.database_path, PathBuf::from("/tmp/voon/library.db"));
        assert_eq!(config.output_device, None);
        assert_eq!(config.listen, DEFAULT_LISTEN.parse().unwrap());
        assert_eq!(config.scan_interval_secs, DEFAULT_SCAN_INTERVAL_SECS);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_a_bad_listen_address() {
        let dir = std::env::temp_dir().join(format!("voon-config-bad-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("voon.toml");
        fs::write(
            &path,
            "\
library_path = \"/music\"
database_path = \"/tmp/voon.db\"
listen = \"not-an-address\"
",
        )
        .unwrap();
        let error = load_config(&path).unwrap_err();
        assert!(matches!(error, ConfigError::Listen { .. }));
        let _ = fs::remove_dir_all(&dir);
    }
}
