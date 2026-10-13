//! Property helpers for the CoreAudio HAL. This module is macOS-only.

use std::ffi::{c_void, CStr};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{format_os_status, AudioError};
use crate::format::choose_integer_format;
use crate::hog::{classify_hog, HogDisposition};
use crate::model::{
    format_rate_list, CandidateFormat, DesiredPcm, DeviceInfo, DeviceSelector, FormatFlags,
    ListedFormat, RateConfirmation, SampleEncoding, SampleRateRange,
};

pub use coreaudio_sys as sys;

const ELEMENT_MAIN: u32 = 0;
const RATE_TIMEOUT: Duration = Duration::from_secs(5);
const FORMAT_TIMEOUT: Duration = Duration::from_secs(3);
const RESTORE_TIMEOUT: Duration = Duration::from_millis(1500);
const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringGetLength(string: *const c_void) -> isize;
    fn CFStringGetMaximumSizeForEncoding(length: isize, encoding: u32) -> isize;
    fn CFStringGetCString(
        string: *const c_void,
        buffer: *mut libc::c_char,
        buffer_size: isize,
        encoding: u32,
    ) -> u8;
    fn CFRelease(value: *const c_void);
}

pub struct EnumeratedDevice {
    pub id: sys::AudioDeviceID,
    pub info: DeviceInfo,
}

pub struct SavedStreamFormat {
    pub stream: sys::AudioStreamID,
    pub physical: sys::AudioStreamBasicDescription,
    pub virtual_format: sys::AudioStreamBasicDescription,
}

struct Observe {
    hits: AtomicU32,
}

pub fn list_devices() -> Result<Vec<EnumeratedDevice>, AudioError> {
    let ids = read_device_ids()?;
    let default_id = try_get_prop::<sys::AudioDeviceID>(
        sys::kAudioObjectSystemObject,
        sys::kAudioHardwarePropertyDefaultOutputDevice,
        sys::kAudioObjectPropertyScopeGlobal,
    )
    .ok();
    let mut devices = Vec::new();
    for id in ids {
        if let Some(device) = describe_device(id, default_id) {
            devices.push(device);
        }
    }
    devices.sort_by(|left, right| left.info.name.cmp(&right.info.name));
    Ok(devices)
}

pub fn resolve_id(selector: &DeviceSelector) -> Result<EnumeratedDevice, AudioError> {
    let devices = list_devices()?;
    let uid = {
        let infos: Vec<DeviceInfo> = devices.iter().map(|device| device.info.clone()).collect();
        crate::model::resolve_device(&infos, selector)?.uid.clone()
    };
    devices
        .into_iter()
        .find(|device| device.info.uid == uid)
        .ok_or_else(|| AudioError::DeviceNotFound {
            selector: selector.to_string(),
        })
}

pub fn release_stale(
    device: sys::AudioDeviceID,
) -> Result<crate::model::StaleHogReport, AudioError> {
    let state = hog_state(device)?;
    match state {
        HogDisposition::Free => Ok(crate::model::StaleHogReport {
            released_pid: None,
            busy_pid: None,
        }),
        HogDisposition::Busy { pid } => Ok(crate::model::StaleHogReport {
            released_pid: None,
            busy_pid: Some(pid),
        }),
        HogDisposition::Stale { pid } => {
            clear_hog(device, "release stale hog")?;
            Ok(crate::model::StaleHogReport {
                released_pid: Some(pid),
                busy_pid: None,
            })
        }
        HogDisposition::OwnedByUs => {
            let pid = our_pid();
            clear_hog(device, "release hog held by this process")?;
            Ok(crate::model::StaleHogReport {
                released_pid: Some(pid),
                busy_pid: None,
            })
        }
    }
}

pub fn acquire_exclusive(device: sys::AudioDeviceID) -> Result<(Option<i32>, i32), AudioError> {
    if !has_property(
        device,
        sys::kAudioDevicePropertyHogMode,
        sys::kAudioObjectPropertyScopeGlobal,
    ) {
        return Err(AudioError::HogUnsupported {
            detail: "kAudioDevicePropertyHogMode is absent".into(),
        });
    }
    let mut stale_released = None;
    match hog_state(device)? {
        HogDisposition::Busy { pid } => return Err(AudioError::DeviceBusy { pid }),
        HogDisposition::OwnedByUs => return Err(AudioError::AlreadyExclusive),
        HogDisposition::Stale { pid } => {
            clear_hog(device, "clear stale hog before acquire")?;
            stale_released = Some(pid);
        }
        HogDisposition::Free => {}
    }
    let pid = our_pid();
    set_prop(
        device,
        sys::kAudioDevicePropertyHogMode,
        sys::kAudioObjectPropertyScopeGlobal,
        &pid,
    )
    .map_err(|error| AudioError::HogUnsupported {
        detail: error.to_string(),
    })?;
    let actual = read_hog(device)?;
    if actual != pid {
        return Err(AudioError::HogUnsupported {
            detail: format!("hog mode read back as {actual} after setting {pid}"),
        });
    }
    Ok((stale_released, pid))
}

pub fn release_our_hog(device: sys::AudioDeviceID) -> bool {
    match read_hog(device) {
        Ok(pid) if pid == our_pid() => {
            clear_hog(device, "release hog").is_ok() && hog_is_free(device)
        }
        Ok(pid) if pid < 0 => true,
        _ => false,
    }
}

fn hog_is_free(device: sys::AudioDeviceID) -> bool {
    read_hog(device).map(|pid| pid < 0).unwrap_or(false)
}

pub fn disable_mixing(device: sys::AudioDeviceID) -> bool {
    let address = address(
        sys::kAudioDevicePropertySupportsMixing,
        sys::kAudioObjectPropertyScopeGlobal,
    );
    if !has_property(
        device,
        sys::kAudioDevicePropertySupportsMixing,
        sys::kAudioObjectPropertyScopeGlobal,
    ) {
        return false;
    }
    if !is_settable(device, &address).unwrap_or(false) {
        return false;
    }
    let disabled: u32 = 0;
    set_prop(
        device,
        sys::kAudioDevicePropertySupportsMixing,
        sys::kAudioObjectPropertyScopeGlobal,
        &disabled,
    )
    .is_ok()
}

pub fn enable_mixing(device: sys::AudioDeviceID) {
    let enabled: u32 = 1;
    let _ = set_prop(
        device,
        sys::kAudioDevicePropertySupportsMixing,
        sys::kAudioObjectPropertyScopeGlobal,
        &enabled,
    );
}

pub fn confirm_nominal_rate(
    device: sys::AudioDeviceID,
    rate: u32,
    cancel: &AtomicBool,
) -> Result<RateConfirmation, AudioError> {
    let supported = nominal_ranges(device).unwrap_or_default();
    if !supported.is_empty() && !supported.iter().any(|range| range.contains(rate)) {
        return Err(AudioError::SampleRateUnsupported {
            rate,
            supported: format_rate_list(&supported),
        });
    }
    let current = read_nominal_rate(device)?;
    if rate_matches(current, rate) {
        return Ok(RateConfirmation {
            requested: rate,
            observed_hz: current,
            listener_fired: false,
            already_set: true,
            wait: Duration::ZERO,
        });
    }
    let observe = Observe {
        hits: AtomicU32::new(0),
    };
    let property = address(
        sys::kAudioDevicePropertyNominalSampleRate,
        sys::kAudioObjectPropertyScopeGlobal,
    );
    let _listener = ListenerGuard::install(device, property, &observe)?;
    let hits_before = observe.hits.load(Ordering::Acquire);
    let rate_f64 = f64::from(rate);
    set_prop(
        device,
        sys::kAudioDevicePropertyNominalSampleRate,
        sys::kAudioObjectPropertyScopeGlobal,
        &rate_f64,
    )?;
    let started = Instant::now();
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err(AudioError::Cancelled);
        }
        let observed = read_nominal_rate(device)?;
        let hits = observe.hits.load(Ordering::Acquire);
        if rate_matches(observed, rate) && hits > hits_before {
            return Ok(RateConfirmation {
                requested: rate,
                observed_hz: observed,
                listener_fired: true,
                already_set: false,
                wait: started.elapsed(),
            });
        }
        if started.elapsed() > RATE_TIMEOUT {
            return Err(AudioError::SampleRateTimeout {
                rate,
                detail: format!(
                    "nominal rate is {observed}, listener_hits={}",
                    hits.saturating_sub(hits_before)
                ),
            });
        }
        thread::sleep(Duration::from_millis(5));
    }
}

pub fn configure_integer_output(
    device: sys::AudioDeviceID,
    rate: u32,
    channels: u16,
    bits: u16,
    cancel: &AtomicBool,
) -> Result<
    (
        sys::AudioStreamBasicDescription,
        sys::AudioStreamBasicDescription,
        Vec<SavedStreamFormat>,
    ),
    AudioError,
> {
    let streams = output_streams(device)?;
    if streams.is_empty() {
        return Err(AudioError::message("device has no output stream"));
    }
    let available = physical_formats(streams[0])?;
    let candidates: Vec<CandidateFormat> = available
        .iter()
        .map(|(candidate, _)| candidate.clone())
        .collect();
    let desired = DesiredPcm {
        sample_rate: rate,
        channels: u32::from(channels),
        bits_per_sample: u32::from(bits),
    };
    let Some(index) = choose_integer_format(&candidates, &desired) else {
        return Err(AudioError::IntegerFormatUnavailable {
            rate,
            channels,
            bits,
            detail: describe_candidates(&candidates),
        });
    };
    let mut chosen = available[index].1;
    chosen.mSampleRate = f64::from(rate);
    let mut saved = Vec::new();
    for stream in &streams {
        let physical = get_prop::<sys::AudioStreamBasicDescription>(
            *stream,
            sys::kAudioStreamPropertyPhysicalFormat,
            sys::kAudioObjectPropertyScopeGlobal,
        )?;
        let virtual_format = get_prop::<sys::AudioStreamBasicDescription>(
            *stream,
            sys::kAudioStreamPropertyVirtualFormat,
            sys::kAudioObjectPropertyScopeGlobal,
        )?;
        saved.push(SavedStreamFormat {
            stream: *stream,
            physical,
            virtual_format,
        });
        if let Err(error) = set_format_and_wait(
            *stream,
            sys::kAudioStreamPropertyPhysicalFormat,
            chosen,
            cancel,
            FORMAT_TIMEOUT,
            "physical",
        ) {
            restore_formats(&saved, cancel);
            return Err(error);
        }
    }
    let physical = get_prop::<sys::AudioStreamBasicDescription>(
        streams[0],
        sys::kAudioStreamPropertyPhysicalFormat,
        sys::kAudioObjectPropertyScopeGlobal,
    )?;
    if !is_signed_integer(&physical) {
        restore_formats(&saved, cancel);
        return Err(AudioError::PhysicalFormat {
            detail: format!(
                "readback is not signed integer: {}",
                summarize_asbd(&physical)
            ),
        });
    }
    // Ask the IO proc to see the same integer layout. A short timeout keeps a
    // device that only accepts a float virtual format from stalling playback.
    if set_format_and_wait(
        streams[0],
        sys::kAudioStreamPropertyVirtualFormat,
        chosen,
        cancel,
        Duration::from_millis(400),
        "virtual",
    )
    .is_err()
    {
        tracing::debug!("virtual format stayed on the device default");
    }
    let virtual_format = get_prop::<sys::AudioStreamBasicDescription>(
        streams[0],
        sys::kAudioStreamPropertyVirtualFormat,
        sys::kAudioObjectPropertyScopeGlobal,
    )?;
    Ok((physical, virtual_format, saved))
}

pub fn restore_formats(saved: &[SavedStreamFormat], cancel: &AtomicBool) {
    for stream in saved {
        let _ = set_format_and_wait(
            stream.stream,
            sys::kAudioStreamPropertyVirtualFormat,
            stream.virtual_format,
            cancel,
            RESTORE_TIMEOUT,
            "restore virtual",
        );
        let _ = set_format_and_wait(
            stream.stream,
            sys::kAudioStreamPropertyPhysicalFormat,
            stream.physical,
            cancel,
            RESTORE_TIMEOUT,
            "restore physical",
        );
    }
}

pub fn restore_nominal_rate(device: sys::AudioDeviceID, rate_hz: f64, cancel: &AtomicBool) {
    let current = read_nominal_rate(device).unwrap_or(rate_hz);
    if (current - rate_hz).abs() < 0.5 {
        return;
    }
    let observe = Observe {
        hits: AtomicU32::new(0),
    };
    let property = address(
        sys::kAudioDevicePropertyNominalSampleRate,
        sys::kAudioObjectPropertyScopeGlobal,
    );
    let Ok(_listener) = ListenerGuard::install(device, property, &observe) else {
        return;
    };
    if set_prop(
        device,
        sys::kAudioDevicePropertyNominalSampleRate,
        sys::kAudioObjectPropertyScopeGlobal,
        &rate_hz,
    )
    .is_err()
    {
        return;
    }
    let started = Instant::now();
    while started.elapsed() < RESTORE_TIMEOUT {
        if cancel.load(Ordering::Acquire) {
            return;
        }
        if read_nominal_rate(device)
            .map(|value| (value - rate_hz).abs() < 0.5)
            .unwrap_or(false)
        {
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
}

pub fn tune_buffer(device: sys::AudioDeviceID, sample_rate: u32) -> u32 {
    let range = try_get_prop::<sys::AudioValueRange>(
        device,
        sys::kAudioDevicePropertyBufferFrameSizeRange,
        sys::kAudioObjectPropertyScopeOutput,
    )
    .ok();
    let current = try_get_prop::<u32>(
        device,
        sys::kAudioDevicePropertyBufferFrameSize,
        sys::kAudioObjectPropertyScopeOutput,
    )
    .unwrap_or(0);
    let Some(range) = range else {
        return current.max(1);
    };
    let target = ((f64::from(sample_rate) * 0.015).round() as u32).clamp(64, 4096);
    let min = range.mMinimum.round().max(1.0) as u32;
    let max = range.mMaximum.round().max(f64::from(min)) as u32;
    let chosen = target.clamp(min, max);
    if chosen != current {
        let _ = set_prop(
            device,
            sys::kAudioDevicePropertyBufferFrameSize,
            sys::kAudioObjectPropertyScopeOutput,
            &chosen,
        );
    }
    try_get_prop::<u32>(
        device,
        sys::kAudioDevicePropertyBufferFrameSize,
        sys::kAudioObjectPropertyScopeOutput,
    )
    .unwrap_or(chosen)
    .max(1)
}

pub fn output_latency(
    device: sys::AudioDeviceID,
    sample_rate: u32,
    buffer_frames: u32,
) -> Duration {
    let mut frames = u64::from(buffer_frames);
    for selector in [
        sys::kAudioDevicePropertyLatency,
        sys::kAudioDevicePropertySafetyOffset,
    ] {
        if let Ok(extra) =
            try_get_prop::<u32>(device, selector, sys::kAudioObjectPropertyScopeOutput)
        {
            frames += u64::from(extra);
        }
    }
    let rate = u64::from(sample_rate.max(1));
    Duration::from_nanos(frames.saturating_mul(1_000_000_000) / rate) + Duration::from_millis(20)
}

pub fn interpret_virtual(
    physical: &sys::AudioStreamBasicDescription,
    virtual_format: &sys::AudioStreamBasicDescription,
    channels: u16,
) -> Result<(SampleEncoding, bool, u16, u16, u16, bool, String, String), AudioError> {
    if virtual_format.mChannelsPerFrame != u32::from(channels) {
        return Err(AudioError::message(format!(
            "virtual format has {} channels, file has {channels}",
            virtual_format.mChannelsPerFrame
        )));
    }
    let non_interleaved = flag(
        virtual_format.mFormatFlags,
        sys::kAudioFormatFlagIsNonInterleaved,
    );
    let bytes_per_sample = if non_interleaved {
        virtual_format.mBytesPerFrame
    } else if channels > 0 && virtual_format.mBytesPerFrame % u32::from(channels) == 0 {
        virtual_format.mBytesPerFrame / u32::from(channels)
    } else {
        return Err(AudioError::message(format!(
            "cannot derive a sample stride from {}",
            summarize_asbd(virtual_format)
        )));
    };
    if bytes_per_sample == 0 || bytes_per_sample > 4 {
        return Err(AudioError::message(format!(
            "unsupported sample stride in {}",
            summarize_asbd(virtual_format)
        )));
    }
    let encoding = if flag(virtual_format.mFormatFlags, sys::kAudioFormatFlagIsFloat) {
        if virtual_format.mBitsPerChannel != 32 || bytes_per_sample != 4 {
            return Err(AudioError::message(format!(
                "only float32 virtual formats can be packed, got {}",
                summarize_asbd(virtual_format)
            )));
        }
        SampleEncoding::Float32 {
            big_endian: flag(
                virtual_format.mFormatFlags,
                sys::kAudioFormatFlagIsBigEndian,
            ),
        }
    } else if flag(
        virtual_format.mFormatFlags,
        sys::kAudioFormatFlagIsSignedInteger,
    ) {
        SampleEncoding::SignedInteger {
            aligned_high: flag(
                virtual_format.mFormatFlags,
                sys::kAudioFormatFlagIsAlignedHigh,
            ),
            big_endian: flag(
                virtual_format.mFormatFlags,
                sys::kAudioFormatFlagIsBigEndian,
            ),
        }
    } else {
        return Err(AudioError::message(format!(
            "virtual format is neither signed integer nor float: {}",
            summarize_asbd(virtual_format)
        )));
    };
    let interleaved = u16::try_from(bytes_per_sample * u32::from(channels)).unwrap_or(u16::MAX);
    let bits = u16::try_from(virtual_format.mBitsPerChannel).unwrap_or(u16::MAX);
    Ok((
        encoding,
        non_interleaved,
        u16::try_from(bytes_per_sample).unwrap_or(u16::MAX),
        interleaved,
        bits,
        is_signed_integer(physical)
            && flag(physical.mFormatFlags, sys::kAudioFormatFlagIsNonMixable),
        summarize_asbd(physical),
        summarize_asbd(virtual_format),
    ))
}

fn describe_device(
    id: sys::AudioDeviceID,
    default_id: Option<sys::AudioDeviceID>,
) -> Option<EnumeratedDevice> {
    let alive = try_get_prop::<u32>(
        id,
        sys::kAudioDevicePropertyDeviceIsAlive,
        sys::kAudioObjectPropertyScopeGlobal,
    )
    .unwrap_or(1);
    if alive == 0 {
        return None;
    }
    let streams = output_streams(id).unwrap_or_default();
    if streams.is_empty() {
        return None;
    }
    let uid = read_cfstring(id, sys::kAudioDevicePropertyDeviceUID).ok()?;
    let name =
        read_cfstring(id, sys::kAudioObjectPropertyName).unwrap_or_else(|_| format!("device-{id}"));
    let nominal = read_nominal_rate(id).unwrap_or(0.0);
    let sample_rates = nominal_ranges(id).unwrap_or_default();
    let hog = hog_state(id).unwrap_or(HogDisposition::Free);
    let current = get_prop::<sys::AudioStreamBasicDescription>(
        streams[0],
        sys::kAudioStreamPropertyPhysicalFormat,
        sys::kAudioObjectPropertyScopeGlobal,
    )
    .ok();
    let formats = physical_formats(streams[0]).unwrap_or_default();
    let physical_formats = formats
        .iter()
        .map(|(candidate, asbd)| listed_format(candidate, asbd))
        .collect();
    let current_physical_format = current.as_ref().map(listed_current);
    Some(EnumeratedDevice {
        id,
        info: DeviceInfo {
            uid,
            name,
            is_default: default_id == Some(id),
            nominal_sample_rate_hz: nominal,
            sample_rates,
            hog,
            output_stream_count: streams.len(),
            current_physical_format,
            physical_formats,
        },
    })
}

fn listed_format(
    candidate: &CandidateFormat,
    asbd: &sys::AudioStreamBasicDescription,
) -> ListedFormat {
    ListedFormat {
        encoding: encoding_name(asbd).to_string(),
        bits_per_channel: candidate.bits_per_channel,
        channels: candidate.channels,
        bytes_per_frame: candidate.bytes_per_frame,
        rate_min_hz: candidate.sample_rate_min,
        rate_max_hz: candidate.sample_rate_max,
        flags: candidate.flags.describe(),
    }
}

fn listed_current(asbd: &sys::AudioStreamBasicDescription) -> ListedFormat {
    let candidate = candidate_from_asbd(
        asbd,
        sys::AudioValueRange {
            mMinimum: asbd.mSampleRate,
            mMaximum: asbd.mSampleRate,
        },
    );
    listed_format(&candidate, asbd)
}

fn encoding_name(asbd: &sys::AudioStreamBasicDescription) -> &'static str {
    if flag(asbd.mFormatFlags, sys::kAudioFormatFlagIsFloat) {
        "float"
    } else if flag(asbd.mFormatFlags, sys::kAudioFormatFlagIsSignedInteger) {
        "integer"
    } else if asbd.mFormatID == sys::kAudioFormatLinearPCM {
        "pcm"
    } else {
        "other"
    }
}

pub fn summarize_asbd(asbd: &sys::AudioStreamBasicDescription) -> String {
    format!(
        "{} bits={} channels={} bytes_per_frame={} rate_hz={} flags={}",
        encoding_name(asbd),
        asbd.mBitsPerChannel,
        asbd.mChannelsPerFrame,
        asbd.mBytesPerFrame,
        crate::model::format_hz(asbd.mSampleRate),
        flags_from(asbd.mFormatFlags).describe()
    )
}

fn describe_candidates(candidates: &[CandidateFormat]) -> String {
    if candidates.is_empty() {
        return "device advertised no physical formats".into();
    }
    candidates
        .iter()
        .map(|candidate| {
            format!(
                "{} bits={} ch={} bytes={} rate={} flags={}",
                if candidate.flags.float {
                    "float"
                } else if candidate.flags.signed_integer {
                    "integer"
                } else {
                    "other"
                },
                candidate.bits_per_channel,
                candidate.channels,
                candidate.bytes_per_frame,
                crate::model::format_rate_range(
                    candidate.sample_rate_min,
                    candidate.sample_rate_max
                ),
                candidate.flags.describe()
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn physical_formats(
    stream: sys::AudioStreamID,
) -> Result<Vec<(CandidateFormat, sys::AudioStreamBasicDescription)>, AudioError> {
    let ranged = get_array::<sys::AudioStreamRangedDescription>(
        stream,
        sys::kAudioStreamPropertyAvailablePhysicalFormats,
        sys::kAudioObjectPropertyScopeGlobal,
    )?;
    Ok(ranged
        .into_iter()
        .map(|entry| {
            (
                candidate_from_asbd(&entry.mFormat, entry.mSampleRateRange),
                entry.mFormat,
            )
        })
        .collect())
}

fn candidate_from_asbd(
    asbd: &sys::AudioStreamBasicDescription,
    range: sys::AudioValueRange,
) -> CandidateFormat {
    let (min, max) = if range.mMinimum == 0.0 && range.mMaximum == 0.0 {
        (asbd.mSampleRate, asbd.mSampleRate)
    } else {
        (range.mMinimum, range.mMaximum)
    };
    CandidateFormat {
        is_linear_pcm: asbd.mFormatID == sys::kAudioFormatLinearPCM,
        sample_rate_min: min,
        sample_rate_max: max,
        bits_per_channel: asbd.mBitsPerChannel,
        channels: asbd.mChannelsPerFrame,
        bytes_per_frame: asbd.mBytesPerFrame,
        flags: flags_from(asbd.mFormatFlags),
    }
}

fn flags_from(flags: u32) -> FormatFlags {
    FormatFlags {
        float: flag(flags, sys::kAudioFormatFlagIsFloat),
        signed_integer: flag(flags, sys::kAudioFormatFlagIsSignedInteger),
        packed: flag(flags, sys::kAudioFormatFlagIsPacked),
        aligned_high: flag(flags, sys::kAudioFormatFlagIsAlignedHigh),
        non_interleaved: flag(flags, sys::kAudioFormatFlagIsNonInterleaved),
        non_mixable: flag(flags, sys::kAudioFormatFlagIsNonMixable),
        big_endian: flag(flags, sys::kAudioFormatFlagIsBigEndian),
    }
}

fn flag(flags: u32, bit: u32) -> bool {
    flags & bit != 0
}

fn is_signed_integer(asbd: &sys::AudioStreamBasicDescription) -> bool {
    asbd.mFormatID == sys::kAudioFormatLinearPCM
        && flag(asbd.mFormatFlags, sys::kAudioFormatFlagIsSignedInteger)
        && !flag(asbd.mFormatFlags, sys::kAudioFormatFlagIsFloat)
}

fn set_format_and_wait(
    stream: sys::AudioStreamID,
    selector: u32,
    format: sys::AudioStreamBasicDescription,
    cancel: &AtomicBool,
    timeout: Duration,
    label: &str,
) -> Result<(), AudioError> {
    if let Ok(actual) = get_prop::<sys::AudioStreamBasicDescription>(
        stream,
        selector,
        sys::kAudioObjectPropertyScopeGlobal,
    ) {
        if format_matches(&actual, &format) {
            return Ok(());
        }
    }
    let observe = Observe {
        hits: AtomicU32::new(0),
    };
    let property = address(selector, sys::kAudioObjectPropertyScopeGlobal);
    let _listener = ListenerGuard::install(stream, property, &observe)?;
    let hits_before = observe.hits.load(Ordering::Acquire);
    set_prop(
        stream,
        selector,
        sys::kAudioObjectPropertyScopeGlobal,
        &format,
    )
    .map_err(|error| AudioError::PhysicalFormat {
        detail: format!("{label} format rejected: {error}"),
    })?;
    let started = Instant::now();
    loop {
        if cancel.load(Ordering::Acquire) {
            return Err(AudioError::Cancelled);
        }
        let actual = get_prop::<sys::AudioStreamBasicDescription>(
            stream,
            selector,
            sys::kAudioObjectPropertyScopeGlobal,
        )?;
        let hits = observe.hits.load(Ordering::Acquire);
        let matched = format_matches(&actual, &format);
        let confirmed = hits > hits_before || asbd_eq(&actual, &format);
        if matched && confirmed {
            return Ok(());
        }
        if started.elapsed() > timeout {
            return Err(AudioError::PhysicalFormat {
                detail: format!(
                    "{label} format timed out, device has {}",
                    summarize_asbd(&actual)
                ),
            });
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn format_matches(
    actual: &sys::AudioStreamBasicDescription,
    expected: &sys::AudioStreamBasicDescription,
) -> bool {
    actual.mFormatID == expected.mFormatID
        && actual.mBitsPerChannel == expected.mBitsPerChannel
        && actual.mChannelsPerFrame == expected.mChannelsPerFrame
        && actual.mBytesPerFrame == expected.mBytesPerFrame
        && (actual.mSampleRate - expected.mSampleRate).abs() < 0.5
        && flag(actual.mFormatFlags, sys::kAudioFormatFlagIsFloat)
            == flag(expected.mFormatFlags, sys::kAudioFormatFlagIsFloat)
        && flag(actual.mFormatFlags, sys::kAudioFormatFlagIsSignedInteger)
            == flag(expected.mFormatFlags, sys::kAudioFormatFlagIsSignedInteger)
}

fn asbd_eq(
    actual: &sys::AudioStreamBasicDescription,
    expected: &sys::AudioStreamBasicDescription,
) -> bool {
    format_matches(actual, expected)
        && actual.mFormatFlags == expected.mFormatFlags
        && actual.mBytesPerPacket == expected.mBytesPerPacket
}

fn nominal_ranges(device: sys::AudioDeviceID) -> Result<Vec<SampleRateRange>, AudioError> {
    let ranges = get_array::<sys::AudioValueRange>(
        device,
        sys::kAudioDevicePropertyAvailableNominalSampleRates,
        sys::kAudioObjectPropertyScopeGlobal,
    )?;
    Ok(ranges
        .into_iter()
        .map(|range| SampleRateRange {
            min_hz: range.mMinimum,
            max_hz: range.mMaximum,
        })
        .collect())
}

fn output_streams(device: sys::AudioDeviceID) -> Result<Vec<sys::AudioStreamID>, AudioError> {
    get_array::<sys::AudioStreamID>(
        device,
        sys::kAudioDevicePropertyStreams,
        sys::kAudioObjectPropertyScopeOutput,
    )
}

fn read_device_ids() -> Result<Vec<sys::AudioDeviceID>, AudioError> {
    get_array::<sys::AudioDeviceID>(
        sys::kAudioObjectSystemObject,
        sys::kAudioHardwarePropertyDevices,
        sys::kAudioObjectPropertyScopeGlobal,
    )
}

pub fn nominal_rate(device: sys::AudioDeviceID) -> Result<f64, AudioError> {
    read_nominal_rate(device)
}

fn read_nominal_rate(device: sys::AudioDeviceID) -> Result<f64, AudioError> {
    get_prop(
        device,
        sys::kAudioDevicePropertyNominalSampleRate,
        sys::kAudioObjectPropertyScopeGlobal,
    )
}

fn rate_matches(actual: f64, requested: u32) -> bool {
    (actual - f64::from(requested)).abs() < 0.5
}

fn hog_state(device: sys::AudioDeviceID) -> Result<HogDisposition, AudioError> {
    let pid = read_hog(device)?;
    Ok(classify_hog(pid, our_pid(), process_alive(pid)))
}

fn read_hog(device: sys::AudioDeviceID) -> Result<i32, AudioError> {
    let pid = get_prop::<libc::pid_t>(
        device,
        sys::kAudioDevicePropertyHogMode,
        sys::kAudioObjectPropertyScopeGlobal,
    )?;
    Ok(pid as i32)
}

fn clear_hog(device: sys::AudioDeviceID, what: &'static str) -> Result<(), AudioError> {
    let free: libc::pid_t = -1;
    set_prop(
        device,
        sys::kAudioDevicePropertyHogMode,
        sys::kAudioObjectPropertyScopeGlobal,
        &free,
    )
    .map_err(|error| AudioError::message(format!("{what}: {error}")))?;
    let after = read_hog(device)?;
    if after >= 0 {
        return Err(AudioError::message(format!(
            "{what}: hog pid is still {after}"
        )));
    }
    Ok(())
}

fn our_pid() -> i32 {
    // SAFETY: getpid is always safe to call.
    unsafe { libc::getpid() as i32 }
}

fn process_alive(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }
    // SAFETY: signal 0 only checks that the process exists.
    let rc = unsafe { libc::kill(pid, 0) };
    if rc == 0 {
        true
    } else {
        std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    }
}

fn read_cfstring(object: sys::AudioObjectID, selector: u32) -> Result<String, AudioError> {
    let mut string: *const c_void = std::ptr::null();
    let mut size = std::mem::size_of::<*const c_void>() as u32;
    let property = address(selector, sys::kAudioObjectPropertyScopeGlobal);
    let status = unsafe {
        sys::AudioObjectGetPropertyData(
            object,
            &property,
            0,
            std::ptr::null(),
            &mut size,
            &mut string as *mut *const c_void as *mut c_void,
        )
    };
    check(status, "AudioObjectGetPropertyData CFString")?;
    if string.is_null() {
        return Err(AudioError::message("CoreAudio returned a null string"));
    }
    let text = cfstring_to_string(string);
    unsafe { CFRelease(string) };
    Ok(text)
}

fn cfstring_to_string(string: *const c_void) -> String {
    let length = unsafe { CFStringGetLength(string) };
    let max = unsafe { CFStringGetMaximumSizeForEncoding(length, CF_STRING_ENCODING_UTF8) };
    let mut buffer = vec![0u8; max as usize + 1];
    let ok = unsafe {
        CFStringGetCString(
            string,
            buffer.as_mut_ptr() as *mut libc::c_char,
            buffer.len() as isize,
            CF_STRING_ENCODING_UTF8,
        )
    };
    if ok == 0 {
        return "<unreadable>".to_string();
    }
    let bytes = unsafe { CStr::from_ptr(buffer.as_ptr() as *const libc::c_char) };
    bytes.to_string_lossy().into_owned()
}

struct ListenerGuard {
    object: sys::AudioObjectID,
    address: sys::AudioObjectPropertyAddress,
    client: *mut c_void,
}

impl ListenerGuard {
    fn install(
        object: sys::AudioObjectID,
        address: sys::AudioObjectPropertyAddress,
        observe: &Observe,
    ) -> Result<Self, AudioError> {
        let client = observe as *const Observe as *mut c_void;
        let status = unsafe {
            sys::AudioObjectAddPropertyListener(object, &address, Some(on_property_change), client)
        };
        check(status, "AudioObjectAddPropertyListener")?;
        Ok(Self {
            object,
            address,
            client,
        })
    }
}

impl Drop for ListenerGuard {
    fn drop(&mut self) {
        unsafe {
            sys::AudioObjectRemovePropertyListener(
                self.object,
                &self.address,
                Some(on_property_change),
                self.client,
            );
        }
    }
}

unsafe extern "C" fn on_property_change(
    _object: sys::AudioObjectID,
    _count: u32,
    _addresses: *const sys::AudioObjectPropertyAddress,
    client: *mut c_void,
) -> sys::OSStatus {
    if !client.is_null() {
        let observe = &*(client as *const Observe);
        observe.hits.fetch_add(1, Ordering::Release);
    }
    0
}

fn address(selector: u32, scope: u32) -> sys::AudioObjectPropertyAddress {
    sys::AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: scope,
        mElement: ELEMENT_MAIN,
    }
}

fn has_property(object: sys::AudioObjectID, selector: u32, scope: u32) -> bool {
    let property = address(selector, scope);
    boolean_is_true(unsafe { sys::AudioObjectHasProperty(object, &property) })
}

fn is_settable(
    object: sys::AudioObjectID,
    property: &sys::AudioObjectPropertyAddress,
) -> Result<bool, AudioError> {
    // `Boolean` is an unsigned char in the SDK. Bindgen has emitted both `u8` and `bool`.
    let mut settable: sys::Boolean = unsafe { std::mem::zeroed() };
    let status = unsafe { sys::AudioObjectIsPropertySettable(object, property, &mut settable) };
    check(status, "AudioObjectIsPropertySettable")?;
    Ok(boolean_is_true(settable))
}

fn boolean_is_true(value: sys::Boolean) -> bool {
    let raw = value as u8;
    raw != 0
}

fn get_prop<T: Copy>(
    object: sys::AudioObjectID,
    selector: u32,
    scope: u32,
) -> Result<T, AudioError> {
    let property = address(selector, scope);
    let mut value = std::mem::MaybeUninit::<T>::uninit();
    let mut size = std::mem::size_of::<T>() as u32;
    let status = unsafe {
        sys::AudioObjectGetPropertyData(
            object,
            &property,
            0,
            std::ptr::null(),
            &mut size,
            value.as_mut_ptr() as *mut c_void,
        )
    };
    check(status, "AudioObjectGetPropertyData")?;
    if size as usize != std::mem::size_of::<T>() {
        return Err(AudioError::message(format!(
            "property {selector:#x} returned {size} bytes, expected {}",
            std::mem::size_of::<T>()
        )));
    }
    // SAFETY: GetPropertyData succeeded and wrote `size` bytes of T.
    Ok(unsafe { value.assume_init() })
}

fn try_get_prop<T: Copy>(
    object: sys::AudioObjectID,
    selector: u32,
    scope: u32,
) -> Result<T, AudioError> {
    get_prop(object, selector, scope)
}

fn set_prop<T>(
    object: sys::AudioObjectID,
    selector: u32,
    scope: u32,
    value: &T,
) -> Result<(), AudioError> {
    let property = address(selector, scope);
    let status = unsafe {
        sys::AudioObjectSetPropertyData(
            object,
            &property,
            0,
            std::ptr::null(),
            std::mem::size_of::<T>() as u32,
            value as *const T as *const c_void,
        )
    };
    check(status, "AudioObjectSetPropertyData")
}

fn get_array<T: Copy>(
    object: sys::AudioObjectID,
    selector: u32,
    scope: u32,
) -> Result<Vec<T>, AudioError> {
    let property = address(selector, scope);
    let mut size = 0u32;
    let status = unsafe {
        sys::AudioObjectGetPropertyDataSize(object, &property, 0, std::ptr::null(), &mut size)
    };
    check(status, "AudioObjectGetPropertyDataSize")?;
    if size == 0 {
        return Ok(Vec::new());
    }
    let mut items = Vec::<T>::with_capacity(size as usize / std::mem::size_of::<T>());
    let status = unsafe {
        sys::AudioObjectGetPropertyData(
            object,
            &property,
            0,
            std::ptr::null(),
            &mut size,
            items.as_mut_ptr() as *mut c_void,
        )
    };
    check(status, "AudioObjectGetPropertyData")?;
    let count = size as usize / std::mem::size_of::<T>();
    // SAFETY: the HAL wrote `count` initialized elements of T.
    unsafe { items.set_len(count) };
    Ok(items)
}

fn check(status: i32, operation: &'static str) -> Result<(), AudioError> {
    if status == 0 {
        Ok(())
    } else {
        Err(AudioError::CoreAudio {
            operation,
            status,
            message: format_os_status(status),
        })
    }
}
