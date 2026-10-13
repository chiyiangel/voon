use std::fmt;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use crate::error::AudioError;
use crate::hog::HogDisposition;

/// Which device `open` should claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceSelector {
    Default,
    /// UID, exact name, or a unique case-insensitive name substring.
    Query(String),
}

/// One advertised nominal-rate entry. A point range has `min_hz == max_hz`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SampleRateRange {
    pub min_hz: f64,
    pub max_hz: f64,
}

impl SampleRateRange {
    pub fn contains(self, rate_hz: u32) -> bool {
        if self.min_hz == 0.0 && self.max_hz == 0.0 {
            return true;
        }
        let rate = f64::from(rate_hz);
        rate + 0.5 >= self.min_hz && rate - 0.5 <= self.max_hz
    }
}

/// A physical format reported by a device, for `list` output.
#[derive(Debug, Clone, PartialEq)]
pub struct ListedFormat {
    pub encoding: String,
    pub bits_per_channel: u32,
    pub channels: u32,
    pub bytes_per_frame: u32,
    pub rate_min_hz: f64,
    pub rate_max_hz: f64,
    pub flags: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceInfo {
    pub uid: String,
    pub name: String,
    pub is_default: bool,
    pub nominal_sample_rate_hz: f64,
    pub sample_rates: Vec<SampleRateRange>,
    pub hog: HogDisposition,
    pub output_stream_count: usize,
    pub current_physical_format: Option<ListedFormat>,
    pub physical_formats: Vec<ListedFormat>,
}

/// PCM the file wants the device to reproduce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesiredPcm {
    pub sample_rate: u32,
    pub channels: u32,
    pub bits_per_sample: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatFlags {
    pub float: bool,
    pub signed_integer: bool,
    pub packed: bool,
    pub aligned_high: bool,
    pub non_interleaved: bool,
    pub non_mixable: bool,
    pub big_endian: bool,
}

impl FormatFlags {
    pub fn describe(self) -> String {
        let mut parts = Vec::new();
        if self.float {
            parts.push("float");
        }
        if self.signed_integer {
            parts.push("signed");
        }
        if self.packed {
            parts.push("packed");
        }
        if self.aligned_high {
            parts.push("aligned_high");
        }
        if self.non_interleaved {
            parts.push("non_interleaved");
        }
        if self.non_mixable {
            parts.push("nonmixable");
        }
        if self.big_endian {
            parts.push("big_endian");
        } else {
            parts.push("little_endian");
        }
        if parts.is_empty() {
            parts.push("none");
        }
        parts.join(",")
    }
}

/// One hardware format the chooser can see. Platform code fills this from the HAL.
#[derive(Debug, Clone, PartialEq)]
pub struct CandidateFormat {
    pub is_linear_pcm: bool,
    pub sample_rate_min: f64,
    pub sample_rate_max: f64,
    pub bits_per_channel: u32,
    pub channels: u32,
    /// `mBytesPerFrame` as advertised. For non-interleaved formats this is one channel.
    pub bytes_per_frame: u32,
    pub flags: FormatFlags,
}

impl CandidateFormat {
    pub fn bytes_per_sample(&self) -> Option<u32> {
        if self.channels == 0 || self.bytes_per_frame == 0 {
            return None;
        }
        if self.flags.non_interleaved {
            Some(self.bytes_per_frame)
        } else if self.bytes_per_frame % self.channels == 0 {
            Some(self.bytes_per_frame / self.channels)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleEncoding {
    SignedInteger {
        aligned_high: bool,
        big_endian: bool,
    },
    Float32 {
        big_endian: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RateConfirmation {
    pub requested: u32,
    pub observed_hz: f64,
    pub listener_fired: bool,
    /// The device was already at the requested rate, so no change notification was expected.
    pub already_set: bool,
    pub wait: Duration,
}

/// Format the engine must pack into. `bytes_per_frame` is the interleaved size.
#[derive(Debug, Clone, PartialEq)]
pub struct NegotiatedFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_channel: u16,
    pub bytes_per_sample: u16,
    pub bytes_per_frame: u16,
    pub encoding: SampleEncoding,
    pub non_interleaved: bool,
    pub non_mixable: bool,
    pub physical_integer: bool,
    pub mixing_disabled: bool,
    pub rate: RateConfirmation,
    pub output_latency: Duration,
    pub physical_summary: String,
    pub virtual_summary: String,
}

#[derive(Debug, Clone)]
pub struct OpenRequest {
    pub device: DeviceSelector,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    /// Ring capacity in PCM frames. The engine sizes this to the preroll plus one second.
    pub buffer_frames: u32,
    pub cancel: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaleHogReport {
    pub released_pid: Option<i32>,
    pub busy_pid: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamStats {
    pub underruns: u64,
    pub bytes_consumed: u64,
    pub hog_released: bool,
    pub stale_hog_released: Option<i32>,
    pub hog_pid: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrainStatus {
    Finished,
    Cancelled,
}

pub fn resolve_device<'a>(
    devices: &'a [DeviceInfo],
    selector: &DeviceSelector,
) -> Result<&'a DeviceInfo, AudioError> {
    match selector {
        DeviceSelector::Default => {
            devices
                .iter()
                .find(|device| device.is_default)
                .ok_or_else(|| AudioError::DeviceNotFound {
                    selector: selector.to_string(),
                })
        }
        DeviceSelector::Query(query) => {
            let query = query.trim();
            if query.is_empty() {
                return Err(AudioError::DeviceNotFound {
                    selector: selector.to_string(),
                });
            }
            if let Some(device) = devices.iter().find(|device| device.uid == query) {
                return Ok(device);
            }
            let exact: Vec<_> = devices
                .iter()
                .filter(|device| device.name.eq_ignore_ascii_case(query))
                .collect();
            if exact.len() == 1 {
                return Ok(exact[0]);
            }
            if exact.len() > 1 {
                return Err(AudioError::AmbiguousDevice {
                    selector: query.to_string(),
                    matches: exact
                        .iter()
                        .map(|device| device.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                });
            }
            let folded = query.to_lowercase();
            let partial: Vec<_> = devices
                .iter()
                .filter(|device| device.name.to_lowercase().contains(&folded))
                .collect();
            match partial.as_slice() {
                [device] => Ok(device),
                [] => Err(AudioError::DeviceNotFound {
                    selector: query.to_string(),
                }),
                many => Err(AudioError::AmbiguousDevice {
                    selector: query.to_string(),
                    matches: many
                        .iter()
                        .map(|device| device.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                }),
            }
        }
    }
}

pub fn format_rate_list(rates: &[SampleRateRange]) -> String {
    if rates.is_empty() {
        return "unknown".to_string();
    }
    rates
        .iter()
        .map(|range| format_rate_range(range.min_hz, range.max_hz))
        .collect::<Vec<_>>()
        .join(",")
}

pub fn format_rate_range(min_hz: f64, max_hz: f64) -> String {
    if min_hz == 0.0 && max_hz == 0.0 {
        "any".to_string()
    } else if (min_hz - max_hz).abs() < 0.5 {
        format_hz(min_hz)
    } else {
        format!("{}-{}", format_hz(min_hz), format_hz(max_hz))
    }
}

pub fn format_hz(hz: f64) -> String {
    if (hz - hz.round()).abs() < 0.05 {
        format!("{}", hz.round() as i64)
    } else {
        format!("{hz}")
    }
}

impl fmt::Display for DeviceSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => f.write_str("default output"),
            Self::Query(query) => write!(f, "{query}"),
        }
    }
}

impl fmt::Display for HogDisposition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Free => f.write_str("free"),
            Self::OwnedByUs => f.write_str("held_by_this_process"),
            Self::Stale { pid } => write!(f, "stale pid={pid}"),
            Self::Busy { pid } => write!(f, "busy pid={pid}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hog::HogDisposition;

    fn device(uid: &str, name: &str, default: bool) -> DeviceInfo {
        DeviceInfo {
            uid: uid.into(),
            name: name.into(),
            is_default: default,
            nominal_sample_rate_hz: 48000.0,
            sample_rates: vec![],
            hog: HogDisposition::Free,
            output_stream_count: 1,
            current_physical_format: None,
            physical_formats: vec![],
        }
    }

    #[test]
    fn resolves_uid_before_a_name_substring() {
        let devices = vec![
            device("uid-dac", "USB DAC", true),
            device("uid-speakers", "Speakers", false),
        ];
        let found =
            resolve_device(&devices, &DeviceSelector::Query("uid-speakers".into())).unwrap();
        assert_eq!(found.name, "Speakers");
        let found = resolve_device(&devices, &DeviceSelector::Query("dac".into())).unwrap();
        assert_eq!(found.uid, "uid-dac");
    }

    #[test]
    fn rejects_an_ambiguous_substring() {
        let devices = vec![
            device("a", "USB DAC A", true),
            device("b", "USB DAC B", false),
        ];
        let error = resolve_device(&devices, &DeviceSelector::Query("usb".into())).unwrap_err();
        assert!(matches!(error, AudioError::AmbiguousDevice { .. }));
    }

    #[test]
    fn default_requires_the_flag() {
        let devices = vec![device("a", "DAC", false)];
        let error = resolve_device(&devices, &DeviceSelector::Default).unwrap_err();
        assert!(matches!(error, AudioError::DeviceNotFound { .. }));
    }
}
