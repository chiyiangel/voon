use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rtrb::{Consumer, Producer, RingBuffer};

use super::hal::{self, sys};
use crate::backend::{AudioOutput, OutputStream, Waiter};
use crate::error::{format_os_status, AudioError};
use crate::model::{
    DeviceInfo, DeviceSelector, DrainStatus, NegotiatedFormat, OpenRequest, StaleHogReport,
    StreamStats,
};
use crate::render::{self, FillResult, Plane};

pub struct CoreAudioOutput;

impl CoreAudioOutput {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CoreAudioOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioOutput for CoreAudioOutput {
    fn backend_name(&self) -> &'static str {
        "coreaudio"
    }

    fn list_devices(&self) -> Result<Vec<DeviceInfo>, AudioError> {
        Ok(hal::list_devices()?
            .into_iter()
            .map(|device| device.info)
            .collect())
    }

    fn release_stale_hog(&mut self, device: &DeviceSelector) -> Result<StaleHogReport, AudioError> {
        let found = hal::resolve_id(device)?;
        hal::release_stale(found.id)
    }

    fn open(&mut self, request: OpenRequest) -> Result<Box<dyn OutputStream>, AudioError> {
        if request.cancel.load(Ordering::Acquire) {
            return Err(AudioError::Cancelled);
        }
        if request.channels == 0 || request.sample_rate == 0 || request.bits_per_sample == 0 {
            return Err(AudioError::message(
                "refusing to open a device for empty PCM",
            ));
        }
        let found = hal::resolve_id(&request.device)?;
        let device = found.id;
        let original_rate = found.info.nominal_sample_rate_hz;
        let (stale_released, hog_pid) = hal::acquire_exclusive(device)?;
        let mut session = ExclusiveSession {
            device,
            hog_held: true,
            mixing_changed: false,
            saved: Vec::new(),
            proc_id: None,
            context: None,
            started: false,
            stopped: false,
            cancel: Arc::clone(&request.cancel),
            original_rate_hz: original_rate,
            rate_changed: false,
            hog_released: false,
        };
        let opened = (|| {
            session.mixing_changed = hal::disable_mixing(device);
            let mut rate = hal::confirm_nominal_rate(device, request.sample_rate, &request.cancel)?;
            session.rate_changed = !rate.already_set;
            let (physical, virtual_format, saved) = hal::configure_integer_output(
                device,
                request.sample_rate,
                request.channels,
                request.bits_per_sample,
                &request.cancel,
            )?;
            session.saved = saved;
            let observed = hal::nominal_rate(device).unwrap_or(rate.observed_hz);
            if (observed - f64::from(request.sample_rate)).abs() >= 0.5 {
                rate = hal::confirm_nominal_rate(device, request.sample_rate, &request.cancel)?;
                session.rate_changed = true;
            }
            let (
                encoding,
                non_interleaved,
                bytes_per_sample,
                interleaved_bytes,
                bits,
                non_mixable,
                physical_summary,
                virtual_summary,
            ) = hal::interpret_virtual(&physical, &virtual_format, request.channels)?;
            let hardware_frames = hal::tune_buffer(device, request.sample_rate);
            let output_latency = hal::output_latency(device, request.sample_rate, hardware_frames);
            let frame_bytes = usize::from(interleaved_bytes);
            if frame_bytes == 0 {
                return Err(AudioError::message("negotiated frame size is zero"));
            }
            let ring_frames = u32::from(request.buffer_frames).max(hardware_frames).max(1) as usize;
            let (producer, consumer) =
                RingBuffer::<u8>::new(ring_frames.saturating_mul(frame_bytes).max(frame_bytes));
            let shared = Arc::new(Shared {
                underruns: AtomicU64::new(0),
                bytes_consumed: AtomicU64::new(0),
                producer_done: AtomicBool::new(false),
                drained: AtomicBool::new(false),
                waiter: Waiter::new(),
            });
            session.context = Some(Box::new(IoContext {
                consumer,
                interleaved_frame_bytes: frame_bytes,
                bytes_per_sample: usize::from(bytes_per_sample),
                non_interleaved,
                shared: Arc::clone(&shared),
            }));
            let negotiated = NegotiatedFormat {
                sample_rate: request.sample_rate,
                channels: request.channels,
                bits_per_channel: bits,
                bytes_per_sample,
                bytes_per_frame: interleaved_bytes,
                encoding,
                non_interleaved,
                non_mixable,
                physical_integer: true,
                mixing_disabled: session.mixing_changed,
                rate,
                output_latency,
                physical_summary,
                virtual_summary,
            };
            Ok((
                negotiated,
                producer,
                shared,
                frame_bytes,
                ring_frames * frame_bytes,
            ))
        })();
        let (negotiated, producer, shared, frame_bytes, capacity) = match opened {
            Ok(value) => value,
            Err(error) => {
                session.shutdown();
                return Err(error);
            }
        };
        Ok(Box::new(CoreStream {
            negotiated,
            producer,
            shared,
            session,
            capacity,
            frame_bytes,
            cancel: request.cancel,
            stale_released,
            hog_pid,
        }))
    }
}

struct Shared {
    underruns: AtomicU64,
    bytes_consumed: AtomicU64,
    producer_done: AtomicBool,
    drained: AtomicBool,
    waiter: Waiter,
}

struct IoContext {
    consumer: Consumer<u8>,
    interleaved_frame_bytes: usize,
    bytes_per_sample: usize,
    non_interleaved: bool,
    shared: Arc<Shared>,
}

struct ExclusiveSession {
    device: sys::AudioDeviceID,
    hog_held: bool,
    mixing_changed: bool,
    saved: Vec<hal::SavedStreamFormat>,
    proc_id: Option<SendProcId>,
    context: Option<Box<IoContext>>,
    started: bool,
    stopped: bool,
    cancel: Arc<AtomicBool>,
    original_rate_hz: f64,
    rate_changed: bool,
    hog_released: bool,
}

/// Opaque HAL callback token. Created and destroyed on the stream thread.
struct SendProcId(sys::AudioDeviceIOProcID);

// SAFETY: the id is only passed back to AudioDeviceStop / AudioDeviceDestroyIOProcID
// on the thread that owns the session. It is not dereferenced as a Rust pointer.
unsafe impl Send for SendProcId {}

impl ExclusiveSession {
    fn start_io(&mut self) -> Result<(), AudioError> {
        if self.started {
            return Ok(());
        }
        let context = self
            .context
            .as_mut()
            .ok_or_else(|| AudioError::message("audio callback context is missing"))?;
        let client = context.as_mut() as *mut IoContext as *mut c_void;
        let mut proc_id: sys::AudioDeviceIOProcID = unsafe { std::mem::zeroed() };
        let status = unsafe {
            sys::AudioDeviceCreateIOProcID(self.device, Some(io_proc), client, &mut proc_id)
        };
        if status != 0 {
            return Err(AudioError::CoreAudio {
                operation: "AudioDeviceCreateIOProcID",
                status,
                message: format_os_status(status),
            });
        }
        let status = unsafe { sys::AudioDeviceStart(self.device, proc_id) };
        if status != 0 {
            unsafe { sys::AudioDeviceDestroyIOProcID(self.device, proc_id) };
            return Err(AudioError::CoreAudio {
                operation: "AudioDeviceStart",
                status,
                message: format_os_status(status),
            });
        }
        self.proc_id = Some(SendProcId(proc_id));
        self.started = true;
        Ok(())
    }

    fn shutdown(&mut self) {
        if self.stopped {
            return;
        }
        self.stopped = true;
        if self.started {
            if let Some(proc_id) = self.proc_id.as_ref() {
                let status = unsafe { sys::AudioDeviceStop(self.device, proc_id.0) };
                if status != 0 {
                    tracing::warn!(status = format_os_status(status), "AudioDeviceStop failed");
                }
            }
            self.started = false;
        }
        if let Some(proc_id) = self.proc_id.take() {
            let status = unsafe { sys::AudioDeviceDestroyIOProcID(self.device, proc_id.0) };
            if status != 0 {
                tracing::warn!(
                    status = format_os_status(status),
                    "AudioDeviceDestroyIOProcID failed"
                );
            }
        }
        // Stop returns only after the callback has finished, so the consumer can be dropped.
        self.context.take();
        let cancelled = self.cancel.load(Ordering::Acquire);
        if !cancelled {
            hal::restore_formats(&self.saved, &self.cancel);
            if self.rate_changed {
                hal::restore_nominal_rate(self.device, self.original_rate_hz, &self.cancel);
            }
        }
        self.saved.clear();
        if self.mixing_changed {
            hal::enable_mixing(self.device);
            self.mixing_changed = false;
        }
        if self.hog_held {
            self.hog_released = hal::release_our_hog(self.device);
            self.hog_held = false;
        }
    }
}

impl Drop for ExclusiveSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct CoreStream {
    negotiated: NegotiatedFormat,
    producer: Producer<u8>,
    shared: Arc<Shared>,
    session: ExclusiveSession,
    capacity: usize,
    frame_bytes: usize,
    cancel: Arc<AtomicBool>,
    stale_released: Option<i32>,
    hog_pid: i32,
}

impl OutputStream for CoreStream {
    fn negotiated(&self) -> &NegotiatedFormat {
        &self.negotiated
    }

    fn write(&mut self, pcm: &[u8]) -> Result<usize, AudioError> {
        if self.frame_bytes == 0 || pcm.len() % self.frame_bytes != 0 {
            return Err(AudioError::message(
                "CoreAudio write was not a whole number of frames",
            ));
        }
        if self.cancel.load(Ordering::Acquire) {
            return Err(AudioError::Cancelled);
        }
        self.shared.waiter.remember_current();
        let (pushed, _rest) = self.producer.push_partial_slice(pcm);
        Ok(pushed.len())
    }

    fn free_bytes(&self) -> usize {
        self.producer.slots()
    }

    fn start(&mut self) -> Result<(), AudioError> {
        self.shared.waiter.remember_current();
        self.session.start_io()
    }

    fn mark_producer_done(&mut self) {
        self.shared.producer_done.store(true, Ordering::Release);
    }

    fn played_bytes(&self) -> u64 {
        self.shared.bytes_consumed.load(Ordering::Acquire)
    }

    fn underruns(&self) -> u64 {
        self.shared.underruns.load(Ordering::Acquire)
    }

    fn wait_until_drained(&mut self, cancel: &AtomicBool) -> Result<DrainStatus, AudioError> {
        let started = std::time::Instant::now();
        loop {
            if cancel.load(Ordering::Acquire) {
                return Ok(DrainStatus::Cancelled);
            }
            let done = self.shared.producer_done.load(Ordering::Acquire);
            let drained = self.shared.drained.load(Ordering::Acquire);
            let empty = self.producer.slots() == self.capacity;
            if drained || (done && empty) {
                std::thread::sleep(self.negotiated.output_latency);
                return Ok(DrainStatus::Finished);
            }
            if done && started.elapsed() > Duration::from_secs(5) {
                tracing::warn!("timed out waiting for the device to drain; stopping");
                return Ok(DrainStatus::Finished);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn stop(&mut self) -> Result<(), AudioError> {
        self.session.shutdown();
        Ok(())
    }

    fn stats(&self) -> StreamStats {
        StreamStats {
            underruns: self.underruns(),
            bytes_consumed: self.played_bytes(),
            hog_released: self.session.hog_released || !self.session.hog_held,
            stale_hog_released: self.stale_released,
            hog_pid: Some(self.hog_pid),
        }
    }
}

impl Drop for CoreStream {
    fn drop(&mut self) {
        self.session.shutdown();
    }
}

unsafe extern "C" fn io_proc(
    _device: sys::AudioDeviceID,
    _now: *const sys::AudioTimeStamp,
    _input: *const sys::AudioBufferList,
    _input_time: *const sys::AudioTimeStamp,
    output: *mut sys::AudioBufferList,
    _output_time: *const sys::AudioTimeStamp,
    client: *mut c_void,
) -> sys::OSStatus {
    if output.is_null() || client.is_null() {
        return 0;
    }
    let context = &mut *(client as *mut IoContext);
    let rendered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        render_output(context, output)
    }));
    if rendered.is_err() {
        silence_list(output);
    }
    0
}

fn render_output(context: &mut IoContext, output: *mut sys::AudioBufferList) -> FillResult {
    let done = context.shared.producer_done.load(Ordering::Acquire);
    let result = unsafe { fill_list(context, output, done) };
    if result.bytes_consumed > 0 {
        context
            .shared
            .bytes_consumed
            .fetch_add(result.bytes_consumed as u64, Ordering::Release);
        context.shared.waiter.unpark();
    }
    if result.underrun {
        context.shared.underruns.fetch_add(1, Ordering::Relaxed);
    }
    if result.complete {
        context.shared.drained.store(true, Ordering::Release);
    }
    result
}

unsafe fn fill_list(
    context: &mut IoContext,
    output: *mut sys::AudioBufferList,
    producer_done: bool,
) -> FillResult {
    let count = (*output).mNumberBuffers as usize;
    if count == 0 {
        return FillResult {
            underrun: false,
            complete: producer_done,
            bytes_consumed: 0,
        };
    }
    if !context.non_interleaved || count == 1 {
        let dest = buffer_slice(output, 0);
        return render::fill_interleaved(
            &mut context.consumer,
            dest,
            context.interleaved_frame_bytes,
            producer_done,
        );
    }
    let mut planes = [EMPTY_PLANE; 8];
    let used = count.min(planes.len());
    for index in 0..used {
        let audio = &*(*output).mBuffers.as_ptr().add(index);
        planes[index] = Plane {
            ptr: audio.mData as *mut u8,
            len: audio.mDataByteSize as usize,
        };
    }
    render::fill_planes(
        &mut context.consumer,
        &mut planes[..used],
        context.bytes_per_sample,
        producer_done,
    )
}

const EMPTY_PLANE: Plane = Plane {
    ptr: std::ptr::null_mut(),
    len: 0,
};

unsafe fn buffer_slice<'a>(output: *mut sys::AudioBufferList, index: usize) -> &'a mut [u8] {
    let audio = &mut *(*output).mBuffers.as_mut_ptr().add(index);
    if audio.mData.is_null() || audio.mDataByteSize == 0 {
        return &mut [];
    }
    std::slice::from_raw_parts_mut(audio.mData as *mut u8, audio.mDataByteSize as usize)
}

unsafe fn silence_list(output: *mut sys::AudioBufferList) {
    let count = (*output).mNumberBuffers as usize;
    let base = (*output).mBuffers.as_mut_ptr();
    for index in 0..count {
        let audio = &mut *base.add(index);
        if !audio.mData.is_null() && audio.mDataByteSize > 0 {
            std::ptr::write_bytes(audio.mData as *mut u8, 0, audio.mDataByteSize as usize);
        }
    }
}
