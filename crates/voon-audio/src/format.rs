use crate::error::AudioError;
use crate::model::{CandidateFormat, DesiredPcm, NegotiatedFormat, SampleEncoding};

/// Pick an integer linear-PCM physical format.
///
/// Exact channel count is required. Bit depth may be wider than the file (the
/// extra bits stay zero) and must not be narrower. Ties prefer a non-mixable
/// flag, interleaved layout, native endian, then the smallest extra bit depth.
pub fn choose_integer_format(available: &[CandidateFormat], desired: &DesiredPcm) -> Option<usize> {
    let mut best: Option<(Score, usize)> = None;
    for (index, format) in available.iter().enumerate() {
        let Some(score) = score_format(format, desired) else {
            continue;
        };
        let replace = match best {
            None => true,
            Some((current, current_index)) => {
                score > current || (score == current && index < current_index)
            }
        };
        if replace {
            best = Some((score, index));
        }
    }
    best.map(|(_, index)| index)
}

type Score = (i32, i32, i32, i32, i32);

fn score_format(format: &CandidateFormat, desired: &DesiredPcm) -> Option<Score> {
    if !format.is_linear_pcm || format.flags.float || !format.flags.signed_integer {
        return None;
    }
    if format.channels != desired.channels {
        return None;
    }
    if !rate_contains(
        format.sample_rate_min,
        format.sample_rate_max,
        desired.sample_rate,
    ) {
        return None;
    }
    if format.bits_per_channel < desired.bits_per_sample || format.bits_per_channel > 32 {
        return None;
    }
    let bytes_per_sample = format.bytes_per_sample()?;
    if bytes_per_sample == 0 || bytes_per_sample > 4 {
        return None;
    }
    if bytes_per_sample * 8 < format.bits_per_channel {
        return None;
    }
    let native_endian = if cfg!(target_endian = "little") {
        !format.flags.big_endian
    } else {
        format.flags.big_endian
    };
    Some((
        i32::from(format.flags.non_mixable),
        i32::from(!format.flags.non_interleaved),
        i32::from(native_endian),
        -((format.bits_per_channel - desired.bits_per_sample) as i32),
        i32::from(format.flags.packed),
    ))
}

fn rate_contains(min_hz: f64, max_hz: f64, rate: u32) -> bool {
    if min_hz == 0.0 && max_hz == 0.0 {
        return true;
    }
    let rate = f64::from(rate);
    rate + 0.5 >= min_hz && rate - 0.5 <= max_hz
}

/// Pack interleaved, sign-extended samples into the device's integer or float layout.
///
/// Integer samples are placed in the significant bits of the container. Wider
/// containers are zero-padded (low-aligned) or left-justified (high-aligned).
/// Float32 is used only when the virtual format is float and the source is at
/// most 24-bit, which float32 can hold exactly.
pub fn pack_interleaved(
    samples: &[i32],
    src_bits: u16,
    format: &NegotiatedFormat,
    out: &mut Vec<u8>,
) -> Result<(), AudioError> {
    let channels = format.channels as usize;
    if channels == 0 || samples.len() % channels != 0 {
        return Err(AudioError::message(
            "sample buffer is not a whole number of frames",
        ));
    }
    let src_bits = u32::from(src_bits);
    if src_bits == 0 || src_bits > 32 {
        return Err(AudioError::message(format!(
            "unsupported source bit depth {src_bits}"
        )));
    }
    let bytes_per_sample = format.bytes_per_sample as usize;
    if bytes_per_sample == 0 || bytes_per_sample > 4 {
        return Err(AudioError::message(
            "negotiated format has an unsupported container width",
        ));
    }
    match format.encoding {
        SampleEncoding::SignedInteger {
            aligned_high,
            big_endian,
        } => {
            if src_bits > u32::from(format.bits_per_channel) {
                return Err(AudioError::message(format!(
                    "refusing to truncate {}-bit audio into a {}-bit integer format",
                    src_bits, format.bits_per_channel
                )));
            }
            let container_bits = (bytes_per_sample * 8) as u32;
            if container_bits < u32::from(format.bits_per_channel) {
                return Err(AudioError::message(
                    "integer container is smaller than the significant bit depth",
                ));
            }
            out.reserve(samples.len() * bytes_per_sample);
            for sample in samples {
                let packed = pack_integer(
                    *sample,
                    src_bits,
                    u32::from(format.bits_per_channel),
                    container_bits,
                    aligned_high,
                );
                write_container(packed, bytes_per_sample, big_endian, out);
            }
        }
        SampleEncoding::Float32 { big_endian } => {
            if src_bits > 24 {
                return Err(AudioError::message(
                    "32-bit integer audio cannot be represented exactly as float32",
                ));
            }
            if bytes_per_sample != 4 {
                return Err(AudioError::message("float virtual format is not 32-bit"));
            }
            let scale = (1u32 << (src_bits - 1)) as f32;
            out.reserve(samples.len() * 4);
            for sample in samples {
                let value = (*sample as f32) / scale;
                let bytes = if big_endian {
                    value.to_be_bytes()
                } else {
                    value.to_le_bytes()
                };
                out.extend_from_slice(&bytes);
            }
        }
    }
    Ok(())
}

fn pack_integer(
    sample: i32,
    src_bits: u32,
    dst_bits: u32,
    container_bits: u32,
    aligned_high: bool,
) -> u32 {
    let shift = if aligned_high {
        container_bits - src_bits
    } else {
        dst_bits - src_bits
    };
    let shifted = (sample as i64) << shift;
    if container_bits >= 32 {
        shifted as u32
    } else {
        let mask = (1u64 << container_bits) - 1;
        (shifted as u64 & mask) as u32
    }
}

fn write_container(bits: u32, container_bytes: usize, big_endian: bool, out: &mut Vec<u8>) {
    if big_endian {
        let encoded = bits.to_be_bytes();
        let start = 4 - container_bytes;
        out.extend_from_slice(&encoded[start..]);
    } else {
        let encoded = bits.to_le_bytes();
        out.extend_from_slice(&encoded[..container_bytes]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FormatFlags, RateConfirmation, SampleEncoding};
    use std::time::Duration;

    fn flags(partial: FormatFlags) -> FormatFlags {
        partial
    }

    fn integer_flags() -> FormatFlags {
        FormatFlags {
            float: false,
            signed_integer: true,
            packed: true,
            aligned_high: false,
            non_interleaved: false,
            non_mixable: false,
            big_endian: false,
        }
    }

    fn candidate(
        bits: u32,
        channels: u32,
        bytes_per_frame: u32,
        min: f64,
        max: f64,
        format_flags: FormatFlags,
    ) -> CandidateFormat {
        CandidateFormat {
            is_linear_pcm: true,
            sample_rate_min: min,
            sample_rate_max: max,
            bits_per_channel: bits,
            channels,
            bytes_per_frame,
            flags: format_flags,
        }
    }

    fn desired() -> DesiredPcm {
        DesiredPcm {
            sample_rate: 96000,
            channels: 2,
            bits_per_sample: 16,
        }
    }

    #[test]
    fn prefers_exact_integer_over_float_and_wider_integers() {
        let mut wide = integer_flags();
        wide.non_mixable = true;
        let mut exact = integer_flags();
        exact.non_mixable = true;
        let mut float_fmt = integer_flags();
        float_fmt.float = true;
        float_fmt.signed_integer = false;
        let available = vec![
            candidate(32, 2, 8, 44100.0, 192000.0, wide),
            candidate(32, 2, 8, 44100.0, 192000.0, float_fmt),
            candidate(16, 2, 4, 44100.0, 192000.0, exact),
            candidate(16, 2, 4, 44100.0, 48000.0, integer_flags()),
        ];
        assert_eq!(choose_integer_format(&available, &desired()), Some(2));
    }

    #[test]
    fn rejects_truncation_and_channel_mismatch() {
        let available = vec![
            candidate(16, 2, 4, 96000.0, 96000.0, integer_flags()),
            candidate(24, 1, 3, 96000.0, 96000.0, integer_flags()),
        ];
        let wanted = DesiredPcm {
            sample_rate: 96000,
            channels: 2,
            bits_per_sample: 24,
        };
        assert_eq!(choose_integer_format(&available, &wanted), None);
    }

    #[test]
    fn zero_rate_range_matches_any_rate() {
        let available = vec![candidate(24, 2, 6, 0.0, 0.0, integer_flags())];
        assert_eq!(choose_integer_format(&available, &desired()), Some(0));
    }

    fn negotiated(
        encoding: SampleEncoding,
        bits: u16,
        bytes_per_sample: u16,
        channels: u16,
    ) -> NegotiatedFormat {
        NegotiatedFormat {
            sample_rate: 44100,
            channels,
            bits_per_channel: bits,
            bytes_per_sample,
            bytes_per_frame: bytes_per_sample * channels,
            encoding,
            non_interleaved: false,
            non_mixable: true,
            physical_integer: true,
            mixing_disabled: true,
            rate: RateConfirmation {
                requested: 44100,
                observed_hz: 44100.0,
                listener_fired: true,
                already_set: false,
                wait: Duration::from_millis(1),
            },
            output_latency: Duration::from_millis(10),
            physical_summary: String::new(),
            virtual_summary: String::new(),
        }
    }

    #[test]
    fn packs_s16_little_endian() {
        let format = negotiated(
            SampleEncoding::SignedInteger {
                aligned_high: false,
                big_endian: false,
            },
            16,
            2,
            2,
        );
        let mut out = Vec::new();
        pack_interleaved(&[1, -1], 16, &format, &mut out).unwrap();
        assert_eq!(out, [0x01, 0x00, 0xff, 0xff]);
    }

    #[test]
    fn packs_s16_into_24_in_32_low_and_high() {
        let low = negotiated(
            SampleEncoding::SignedInteger {
                aligned_high: false,
                big_endian: false,
            },
            24,
            4,
            1,
        );
        let mut out = Vec::new();
        pack_interleaved(&[1], 16, &low, &mut out).unwrap();
        assert_eq!(out, [0x00, 0x01, 0x00, 0x00]);

        let high = negotiated(
            SampleEncoding::SignedInteger {
                aligned_high: true,
                big_endian: false,
            },
            24,
            4,
            1,
        );
        out.clear();
        pack_interleaved(&[1], 16, &high, &mut out).unwrap();
        assert_eq!(out, [0x00, 0x00, 0x01, 0x00]);
    }

    #[test]
    fn packs_negative_s16_into_float32_exactly() {
        let format = negotiated(SampleEncoding::Float32 { big_endian: false }, 32, 4, 1);
        let mut out = Vec::new();
        pack_interleaved(&[-32768], 16, &format, &mut out).unwrap();
        let value = f32::from_le_bytes(out.try_into().unwrap());
        assert_eq!(value, -1.0);
    }

    #[test]
    fn refuses_to_truncate() {
        let format = negotiated(
            SampleEncoding::SignedInteger {
                aligned_high: false,
                big_endian: false,
            },
            16,
            2,
            1,
        );
        let error = pack_interleaved(&[1], 24, &format, &mut Vec::new()).unwrap_err();
        assert!(error.to_string().contains("truncate"));
    }

    #[test]
    fn flags_round_trip_through_describe() {
        let text = flags(integer_flags()).describe();
        assert!(text.contains("signed"));
        assert!(text.contains("little_endian"));
    }
}
