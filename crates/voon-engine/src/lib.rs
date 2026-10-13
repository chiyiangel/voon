//! Decode FLAC, WAV, AIFF, and ALAC with Symphonia and feed an [`AudioOutput`].
//!
//! The output callback never reads the file. This crate decodes on the caller
//! thread into a ring sized for at least the requested preroll (30 seconds by
//! default) and only then starts the device.

mod decode;
mod play;

pub use decode::{decode_interleaved, probe_file, SourceInfo};
pub use play::{play_file, PlayConfig, PlayEvent, PlayReport};

use std::time::Duration;

use voon_audio::AudioError;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("{0}")]
    Unsupported(String),
    #[error("failed to decode {path}: {message}")]
    Decode { path: String, message: String },
    #[error(transparent)]
    Audio(#[from] AudioError),
    #[error("failed to read audio: {0}")]
    Io(#[from] std::io::Error),
}

pub const DEFAULT_PREROLL: Duration = Duration::from_secs(30);
