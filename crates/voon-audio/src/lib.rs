//! Audio output for Voon.
//!
//! The [`AudioOutput`] trait is what a future ALSA backend implements. On macOS,
//! [`open_system`] returns a CoreAudio HAL backend that takes hog mode, waits for
//! the nominal sample rate to change, and selects an integer physical format.
//! The real-time callback only copies from a ring buffer.

mod backend;
mod error;
mod format;
mod hog;
mod model;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod render;

#[cfg(target_os = "macos")]
mod coreaudio;

pub use backend::{AudioOutput, MemoryOutput, OutputStream};
pub use error::{format_os_status, AudioError};
pub use format::{choose_integer_format, pack_interleaved};
pub use hog::{classify_hog, HogDisposition};
pub use model::{
    format_hz, format_rate_list, format_rate_range, resolve_device, CandidateFormat, DesiredPcm,
    DeviceInfo, DeviceSelector, DrainStatus, FormatFlags, ListedFormat, NegotiatedFormat,
    OpenRequest, RateConfirmation, SampleEncoding, SampleRateRange, StaleHogReport, StreamStats,
};

/// Open the platform output. CoreAudio is macOS-only; other targets return an error
/// so the rest of the workspace still builds and tests.
pub fn open_system() -> Result<Box<dyn AudioOutput>, AudioError> {
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(coreaudio::CoreAudioOutput::new()))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(AudioError::PlatformUnsupported {
            backend: "coreaudio",
        })
    }
}
