//! Voon daemon: load config, migrate the library, clear a stale hog, serve `/health`.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use voon_audio::{open_system, AudioError, DeviceSelector};
use voon_core::Config;
use voon_library::Library;

pub fn prepare(config: &Config) -> anyhow::Result<()> {
    if !config.library_path.exists() {
        tracing::warn!(
            path = %config.library_path.display(),
            "library path is not available; the daemon still serves /health"
        );
    }
    if let Some(parent) = config.database_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let library = Library::open(&config.database_path)?;
    library.migrate()?;
    release_configured_hog(config);
    Ok(())
}

fn release_configured_hog(config: &Config) {
    let Some(device) = config.output_device.as_deref() else {
        return;
    };
    let mut output = match open_system() {
        Ok(output) => output,
        Err(AudioError::PlatformUnsupported { backend }) => {
            tracing::info!(
                backend,
                "skipping stale-hog cleanup on this operating system"
            );
            return;
        }
        Err(error) => {
            tracing::warn!(%error, "could not open the audio backend; continuing");
            return;
        }
    };
    match output.release_stale_hog(&DeviceSelector::Query(device.to_string())) {
        Ok(report) => {
            if let Some(pid) = report.released_pid {
                tracing::info!(pid, device, "released a stale exclusive lock");
            } else if let Some(pid) = report.busy_pid {
                tracing::warn!(
                    pid,
                    device,
                    "output device is exclusively locked by a live process"
                );
            }
        }
        Err(error) => {
            tracing::warn!(%error, device, "could not inspect the output device; continuing");
        }
    }
}

pub async fn serve(listen: SocketAddr, shutdown: Arc<AtomicBool>) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(listen).await?;
    tracing::info!(%listen, "listening");
    axum::serve(listener, voon_api::router())
        .with_graceful_shutdown(async move {
            while !shutdown.load(Ordering::Acquire) {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use voon_core::Config;

    #[test]
    fn prepare_migrates_when_the_library_mount_is_missing() {
        let dir = std::env::temp_dir().join(format!("voon-daemon-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let config = Config {
            library_path: dir.join("not-mounted"),
            database_path: dir.join("db").join("library.db"),
            output_device: Some("USB DAC".into()),
            listen: "127.0.0.1:9".parse::<SocketAddr>().unwrap(),
            scan_interval_secs: 3600,
        };
        prepare(&config).unwrap();
        prepare(&config).unwrap();
        assert!(config.database_path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
