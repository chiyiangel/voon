use std::fs::File;
use std::path::Path;
use std::time::Duration;

use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{
    CodecType, Decoder, DecoderOptions, CODEC_TYPE_ALAC, CODEC_TYPE_FLAC, CODEC_TYPE_PCM_F32BE,
    CODEC_TYPE_PCM_F32LE, CODEC_TYPE_PCM_F64BE, CODEC_TYPE_PCM_F64LE, CODEC_TYPE_PCM_S16BE,
    CODEC_TYPE_PCM_S16LE, CODEC_TYPE_PCM_S24BE, CODEC_TYPE_PCM_S24LE, CODEC_TYPE_PCM_S32BE,
    CODEC_TYPE_PCM_S32LE, CODEC_TYPE_PCM_S8,
};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::sample::Sample;

use crate::EngineError;

#[derive(Debug, Clone, PartialEq)]
pub struct SourceInfo {
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub duration: Option<Duration>,
    pub frames: Option<u64>,
}

pub(crate) struct DecoderStream {
    path: String,
    info: SourceInfo,
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    skip_head: u64,
    padding_frames: usize,
    held: Vec<i32>,
    finished: bool,
}

impl DecoderStream {
    pub(crate) fn open(path: &Path) -> Result<Self, EngineError> {
        let file = File::open(path)?;
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
            hint.with_extension(extension);
        }
        let source = MediaSourceStream::new(Box::new(file), Default::default());
        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                source,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|error| decode_error(path, error))?;
        let reader = probed.format;
        let track = reader
            .tracks()
            .iter()
            .find(|track| track.codec_params.sample_rate.is_some())
            .cloned()
            .ok_or_else(|| {
                EngineError::Unsupported(format!("{} has no audio track", path.display()))
            })?;
        let params = track.codec_params.clone();
        let sample_rate = params.sample_rate.ok_or_else(|| {
            EngineError::Unsupported(format!("{} has no sample rate", path.display()))
        })?;
        let alac = if params.codec == CODEC_TYPE_ALAC {
            params.extra_data.as_deref().and_then(parse_alac_cookie)
        } else {
            None
        };
        let channels = params
            .channels
            .map(|channels| channels.count())
            .or(alac.as_ref().map(|cookie| cookie.channels))
            .unwrap_or(0);
        if channels > 8 {
            return Err(EngineError::Unsupported(format!(
                "{} has {channels} channels; this slice plays 1 to 8",
                path.display()
            )));
        }
        let channels = channels as u16;
        let bits = params
            .bits_per_sample
            .or(params.bits_per_coded_sample)
            .filter(|bits| (8..=32).contains(bits))
            .or(alac.as_ref().map(|cookie| cookie.bits))
            .unwrap_or(0) as u16;
        if matches!(
            params.codec,
            CODEC_TYPE_PCM_F32LE
                | CODEC_TYPE_PCM_F32BE
                | CODEC_TYPE_PCM_F64LE
                | CODEC_TYPE_PCM_F64BE
        ) {
            return Err(EngineError::Unsupported(format!(
                "{} is floating-point PCM; exclusive integer output needs integer audio",
                path.display()
            )));
        }
        let decoder = symphonia::default::get_codecs()
            .make(&params, &DecoderOptions::default())
            .map_err(|error| decode_error(path, error))?;
        let delay = u64::from(params.delay.unwrap_or(0));
        let padding = params.padding.unwrap_or(0) as usize;
        let playable = params
            .n_frames
            .map(|frames| frames.saturating_sub(delay).saturating_sub(padding as u64));
        let duration = playable
            .map(|frames| Duration::from_secs_f64(frames as f64 / f64::from(sample_rate.max(1))));
        Ok(Self {
            path: path.display().to_string(),
            info: SourceInfo {
                codec: codec_label(params.codec),
                sample_rate,
                channels,
                bits_per_sample: bits,
                duration,
                frames: playable,
            },
            format: reader,
            decoder,
            track_id: track.id,
            skip_head: delay,
            padding_frames: padding,
            held: Vec::new(),
            finished: false,
        })
    }

    pub(crate) fn info(&self) -> &SourceInfo {
        &self.info
    }

    /// Next interleaved, sign-extended PCM frames. `None` is end of stream.
    pub(crate) fn next_frames(&mut self) -> Result<Option<Vec<i32>>, EngineError> {
        if self.finished {
            return Ok(None);
        }
        loop {
            let packet = match self.format.next_packet() {
                Ok(packet) => packet,
                Err(SymphoniaError::IoError(error))
                    if error.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    self.finished = true;
                    return Ok(self.drain_tail());
                }
                Err(SymphoniaError::ResetRequired) => {
                    return Err(EngineError::Decode {
                        path: self.path.clone(),
                        message: "decoder reset required mid-stream".into(),
                    });
                }
                Err(error) => return Err(self.decode_error(error)),
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            let decoded = match self.decoder.decode(&packet) {
                Ok(decoded) => decoded,
                Err(SymphoniaError::DecodeError(message)) => {
                    tracing::warn!(message, "skipping an undecodable packet");
                    continue;
                }
                Err(error) => return Err(self.decode_error(error)),
            };
            if self.info.channels == 0 {
                let count = decoded.spec().channels.count();
                if count == 0 || count > 8 {
                    return Err(EngineError::Unsupported(format!(
                        "{} has {count} channels; this slice plays 1 to 8",
                        self.path
                    )));
                }
                self.info.channels = count as u16;
            }
            let mut samples = Vec::new();
            let buffer_bits =
                append_interleaved(&decoded, &mut samples, self.info.bits_per_sample)?;
            if self.info.bits_per_sample == 0 {
                self.info.bits_per_sample = buffer_bits;
            }
            if samples.is_empty() {
                continue;
            }
            let channels = self.info.channels as usize;
            let mut frames = samples.len() / channels;
            if self.skip_head > 0 {
                let skip = (self.skip_head as usize).min(frames);
                samples.drain(..skip * channels);
                self.skip_head -= skip as u64;
                frames -= skip;
                if frames == 0 {
                    continue;
                }
            }
            let hold_tail = self
                .padding_frames
                .saturating_mul(self.info.channels as usize);
            if hold_tail == 0 {
                return Ok(Some(samples));
            }
            self.held.extend(samples);
            if self.held.len() <= hold_tail {
                continue;
            }
            let emit = self.held.len() - hold_tail;
            let emit = emit - (emit % channels);
            if emit == 0 {
                continue;
            }
            let out = self.held.drain(..emit).collect();
            return Ok(Some(out));
        }
    }

    fn drain_tail(&mut self) -> Option<Vec<i32>> {
        if self.held.is_empty() {
            None
        } else {
            // `held` is the encoder padding that should not be played.
            self.held.clear();
            None
        }
    }

    fn decode_error(&self, error: SymphoniaError) -> EngineError {
        EngineError::Decode {
            path: self.path.clone(),
            message: error.to_string(),
        }
    }
}

pub fn probe_file(path: &Path) -> Result<SourceInfo, EngineError> {
    Ok(DecoderStream::open(path)?.info().clone())
}

/// Decode a whole file to interleaved sign-extended samples. Used by tests and `decode`.
pub fn decode_interleaved(path: &Path) -> Result<(SourceInfo, Vec<i32>), EngineError> {
    let mut decoder = DecoderStream::open(path)?;
    let mut all = Vec::new();
    while let Some(frames) = decoder.next_frames()? {
        all.extend(frames);
    }
    if decoder.info.bits_per_sample == 0 {
        return Err(EngineError::Unsupported(format!(
            "{} produced no integer PCM",
            path.display()
        )));
    }
    Ok((decoder.info, all))
}

fn append_interleaved(
    decoded: &AudioBufferRef<'_>,
    out: &mut Vec<i32>,
    known_bits: u16,
) -> Result<u16, EngineError> {
    match decoded {
        AudioBufferRef::S8(buffer) => {
            push_planes(buffer, out, |sample| sample as i32);
            Ok(8)
        }
        AudioBufferRef::S16(buffer) => {
            push_planes(buffer, out, |sample| sample as i32);
            Ok(16)
        }
        AudioBufferRef::S24(buffer) => {
            push_planes(buffer, out, |sample| sample.clamped().inner());
            Ok(24)
        }
        AudioBufferRef::S32(buffer) => {
            // FLAC and ALAC store every depth in an i32 and left-justify it
            // (`sample << (32 - bits)`). Shift back to a sign-extended sample.
            let bits = if (1..32).contains(&known_bits) {
                known_bits
            } else {
                32
            };
            let shift = 32 - u32::from(bits);
            push_planes(buffer, out, |sample| sample >> shift);
            Ok(bits)
        }
        AudioBufferRef::F32(_) | AudioBufferRef::F64(_) => Err(EngineError::Unsupported(
            "floating-point PCM cannot be sent bit-perfect to an integer DAC".into(),
        )),
        AudioBufferRef::U8(_)
        | AudioBufferRef::U16(_)
        | AudioBufferRef::U24(_)
        | AudioBufferRef::U32(_) => Err(EngineError::Unsupported(
            "unsigned PCM is not played by this slice".into(),
        )),
    }
}

fn push_planes<S, F>(
    buffer: &symphonia::core::audio::AudioBuffer<S>,
    out: &mut Vec<i32>,
    mut convert: F,
) where
    S: Sample,
    F: FnMut(S) -> i32,
{
    let channels = buffer.spec().channels.count();
    let frames = buffer.frames();
    out.reserve(frames * channels);
    for frame in 0..frames {
        for channel in 0..channels {
            out.push(convert(buffer.chan(channel)[frame]));
        }
    }
}

fn codec_label(codec: CodecType) -> String {
    if codec == CODEC_TYPE_FLAC {
        "FLAC".to_string()
    } else if codec == CODEC_TYPE_ALAC {
        "ALAC".to_string()
    } else if codec == CODEC_TYPE_PCM_S16LE || codec == CODEC_TYPE_PCM_S16BE {
        "PCM_S16".to_string()
    } else if codec == CODEC_TYPE_PCM_S24LE || codec == CODEC_TYPE_PCM_S24BE {
        "PCM_S24".to_string()
    } else if codec == CODEC_TYPE_PCM_S32LE || codec == CODEC_TYPE_PCM_S32BE {
        "PCM_S32".to_string()
    } else if codec == CODEC_TYPE_PCM_S8 {
        "PCM_S8".to_string()
    } else {
        format!("{codec:?}")
    }
}

struct AlacCookie {
    bits: u32,
    channels: usize,
}

/// The ISOMP4 demuxer stores ALAC's channel count and bit depth only in the magic cookie.
fn parse_alac_cookie(extra: &[u8]) -> Option<AlacCookie> {
    if extra.len() != 24 && extra.len() != 48 {
        return None;
    }
    let bits = extra[5] as u32;
    let channels = extra[9] as usize;
    if !(8..=32).contains(&bits) || !(1..=8).contains(&channels) {
        return None;
    }
    Some(AlacCookie { bits, channels })
}

fn decode_error(path: &Path, error: SymphoniaError) -> EngineError {
    EngineError::Decode {
        path: path.display().to_string(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_pcm_wav(path: &Path, rate: u32, channels: u16, bits: u16, samples: &[i32]) {
        assert!(bits == 16 || bits == 24);
        let bytes_per_sample = (bits / 8) as usize;
        let block_align = channels as usize * bytes_per_sample;
        let data_len = samples.len() * bytes_per_sample;
        let mut data = Vec::with_capacity(data_len);
        for sample in samples {
            let bytes = sample.to_le_bytes();
            data.extend_from_slice(&bytes[..bytes_per_sample]);
        }
        let mut file = Vec::new();
        file.extend_from_slice(b"RIFF");
        file.extend_from_slice(&(36 + data_len as u32).to_le_bytes());
        file.extend_from_slice(b"WAVE");
        file.extend_from_slice(b"fmt ");
        file.extend_from_slice(&16u32.to_le_bytes());
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&channels.to_le_bytes());
        file.extend_from_slice(&rate.to_le_bytes());
        file.extend_from_slice(&(rate * block_align as u32).to_le_bytes());
        file.extend_from_slice(&(block_align as u16).to_le_bytes());
        file.extend_from_slice(&bits.to_le_bytes());
        file.extend_from_slice(b"data");
        file.extend_from_slice(&(data_len as u32).to_le_bytes());
        file.extend_from_slice(&data);
        std::fs::write(path, file).unwrap();
    }

    #[test]
    fn wav_s16_round_trip_is_sign_extended() {
        let dir = std::env::temp_dir().join(format!("voon-wav-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ramp.wav");
        let samples = [0, -1, 32767, -32768];
        write_pcm_wav(&path, 44100, 2, 16, &samples);
        let (info, decoded) = decode_interleaved(&path).unwrap();
        assert_eq!(info.codec, "PCM_S16");
        assert_eq!(info.sample_rate, 44100);
        assert_eq!(info.channels, 2);
        assert_eq!(info.bits_per_sample, 16);
        assert_eq!(decoded, samples);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn wav_s24_keeps_low_24_bits() {
        let dir = std::env::temp_dir().join(format!("voon-wav24-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("wide.wav");
        let samples = [0x123456, -0x123456];
        write_pcm_wav(&path, 96000, 2, 24, &samples);
        let (info, decoded) = decode_interleaved(&path).unwrap();
        assert_eq!(info.sample_rate, 96000);
        assert_eq!(info.bits_per_sample, 24);
        assert_eq!(decoded, samples, "decoded={decoded:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn container_fixtures_decode_to_the_same_pcm() {
        let expected = [0, -1, 32767, -32768];
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        for (name, codec) in [
            ("tone.flac", "FLAC"),
            ("tone.aiff", "PCM_S16"),
            ("tone.m4a", "ALAC"),
        ] {
            let (info, decoded) = decode_interleaved(&root.join(name)).unwrap();
            assert_eq!(info.codec, codec, "{name}");
            assert_eq!(info.sample_rate, 44100, "{name}");
            assert_eq!(info.channels, 2, "{name}");
            assert_eq!(info.bits_per_sample, 16, "{name}");
            assert_eq!(decoded, expected, "{name} decoded={decoded:?}");
        }
    }
}
