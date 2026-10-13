use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::Thread;
use std::time::Duration;

use crate::error::AudioError;
use crate::hog::HogDisposition;
use crate::model::{
    DeviceInfo, DeviceSelector, DrainStatus, NegotiatedFormat, OpenRequest, RateConfirmation,
    SampleEncoding, StaleHogReport, StreamStats,
};

pub trait AudioOutput: Send {
    fn backend_name(&self) -> &'static str;
    fn list_devices(&self) -> Result<Vec<DeviceInfo>, AudioError>;
    fn release_stale_hog(&mut self, device: &DeviceSelector) -> Result<StaleHogReport, AudioError>;
    fn open(&mut self, request: OpenRequest) -> Result<Box<dyn OutputStream>, AudioError>;
}

pub trait OutputStream: Send {
    fn negotiated(&self) -> &NegotiatedFormat;
    /// Queue interleaved PCM already packed for [`NegotiatedFormat`].
    ///
    /// Returns the number of bytes accepted. `0` means the ring is full; the
    /// caller should park briefly. The audio callback unparks that thread.
    fn write(&mut self, pcm: &[u8]) -> Result<usize, AudioError>;
    fn free_bytes(&self) -> usize;
    fn start(&mut self) -> Result<(), AudioError>;
    fn mark_producer_done(&mut self);
    fn played_bytes(&self) -> u64;
    fn underruns(&self) -> u64;
    fn wait_until_drained(&mut self, cancel: &AtomicBool) -> Result<DrainStatus, AudioError>;
    fn stop(&mut self) -> Result<(), AudioError>;
    fn stats(&self) -> StreamStats;
}

/// Software sink used by tests and by the playback pipeline on machines without CoreAudio.
pub struct MemoryOutput {
    captured: Arc<Mutex<Vec<u8>>>,
    started: Arc<AtomicBool>,
    underruns: Arc<AtomicU64>,
}

impl MemoryOutput {
    pub fn new() -> Self {
        Self {
            captured: Arc::new(Mutex::new(Vec::new())),
            started: Arc::new(AtomicBool::new(false)),
            underruns: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn captured(&self) -> Vec<u8> {
        self.captured.lock().expect("memory capture lock").clone()
    }

    pub fn started(&self) -> bool {
        self.started.load(Ordering::Acquire)
    }
}

impl Default for MemoryOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioOutput for MemoryOutput {
    fn backend_name(&self) -> &'static str {
        "memory"
    }

    fn list_devices(&self) -> Result<Vec<DeviceInfo>, AudioError> {
        Ok(vec![memory_device()])
    }

    fn release_stale_hog(
        &mut self,
        _device: &DeviceSelector,
    ) -> Result<StaleHogReport, AudioError> {
        Ok(StaleHogReport {
            released_pid: None,
            busy_pid: None,
        })
    }

    fn open(&mut self, request: OpenRequest) -> Result<Box<dyn OutputStream>, AudioError> {
        if request.cancel.load(Ordering::Acquire) {
            return Err(AudioError::Cancelled);
        }
        let _ = crate::model::resolve_device(&[memory_device()], &request.device)?;
        if request.channels == 0 || request.bits_per_sample == 0 || request.bits_per_sample > 32 {
            return Err(AudioError::message(
                "memory output rejected the PCM description",
            ));
        }
        if request.bits_per_sample % 8 != 0 {
            return Err(AudioError::message(
                "memory output only accepts whole-byte PCM",
            ));
        }
        let bytes_per_sample = request.bits_per_sample / 8;
        let negotiated = NegotiatedFormat {
            sample_rate: request.sample_rate,
            channels: request.channels,
            bits_per_channel: request.bits_per_sample,
            bytes_per_sample,
            bytes_per_frame: bytes_per_sample * request.channels,
            encoding: SampleEncoding::SignedInteger {
                aligned_high: false,
                big_endian: false,
            },
            non_interleaved: false,
            non_mixable: true,
            physical_integer: true,
            mixing_disabled: true,
            rate: RateConfirmation {
                requested: request.sample_rate,
                observed_hz: f64::from(request.sample_rate),
                listener_fired: false,
                already_set: true,
                wait: Duration::ZERO,
            },
            output_latency: Duration::from_millis(1),
            physical_summary: format!(
                "integer bits={} channels={} bytes_per_frame={}",
                request.bits_per_sample,
                request.channels,
                bytes_per_sample * request.channels
            ),
            virtual_summary: "integer".to_string(),
        };
        Ok(Box::new(MemoryStream {
            negotiated,
            captured: Arc::clone(&self.captured),
            started: Arc::clone(&self.started),
            underruns: Arc::clone(&self.underruns),
            producer_done: false,
            stopped: false,
            played: 0,
        }))
    }
}

fn memory_device() -> DeviceInfo {
    DeviceInfo {
        uid: "memory".to_string(),
        name: "Memory Output".to_string(),
        is_default: true,
        nominal_sample_rate_hz: 48000.0,
        sample_rates: vec![],
        hog: HogDisposition::Free,
        output_stream_count: 1,
        current_physical_format: None,
        physical_formats: vec![],
    }
}

struct MemoryStream {
    negotiated: NegotiatedFormat,
    captured: Arc<Mutex<Vec<u8>>>,
    started: Arc<AtomicBool>,
    underruns: Arc<AtomicU64>,
    producer_done: bool,
    stopped: bool,
    played: u64,
}

impl OutputStream for MemoryStream {
    fn negotiated(&self) -> &NegotiatedFormat {
        &self.negotiated
    }

    fn write(&mut self, pcm: &[u8]) -> Result<usize, AudioError> {
        let frame = self.negotiated.bytes_per_frame as usize;
        if frame == 0 || pcm.len() % frame != 0 {
            return Err(AudioError::message(
                "memory output write was not a whole number of frames",
            ));
        }
        self.captured
            .lock()
            .expect("memory capture lock")
            .extend_from_slice(pcm);
        self.played += pcm.len() as u64;
        Ok(pcm.len())
    }

    fn free_bytes(&self) -> usize {
        usize::MAX / 4
    }

    fn start(&mut self) -> Result<(), AudioError> {
        self.started.store(true, Ordering::Release);
        Ok(())
    }

    fn mark_producer_done(&mut self) {
        self.producer_done = true;
    }

    fn played_bytes(&self) -> u64 {
        self.played
    }

    fn underruns(&self) -> u64 {
        self.underruns.load(Ordering::Acquire)
    }

    fn wait_until_drained(&mut self, cancel: &AtomicBool) -> Result<DrainStatus, AudioError> {
        if cancel.load(Ordering::Acquire) {
            Ok(DrainStatus::Cancelled)
        } else {
            Ok(DrainStatus::Finished)
        }
    }

    fn stop(&mut self) -> Result<(), AudioError> {
        self.stopped = true;
        Ok(())
    }

    fn stats(&self) -> StreamStats {
        StreamStats {
            underruns: self.underruns(),
            bytes_consumed: self.played,
            hog_released: true,
            stale_hog_released: None,
            hog_pid: None,
        }
    }
}

/// Remembered so the real-time callback can wake the decoder without allocating.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) struct Waiter {
    thread: Mutex<Option<Thread>>,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
impl Waiter {
    pub(crate) fn new() -> Self {
        Self {
            thread: Mutex::new(None),
        }
    }

    pub(crate) fn remember_current(&self) {
        if let Ok(mut slot) = self.thread.lock() {
            *slot = Some(std::thread::current());
        }
    }

    pub(crate) fn unpark(&self) {
        if let Ok(slot) = self.thread.try_lock() {
            if let Some(thread) = slot.as_ref() {
                thread.unpark();
            }
        }
    }
}
