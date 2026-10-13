use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use voon_audio::{
    pack_interleaved, AudioError, AudioOutput, DeviceSelector, DrainStatus, NegotiatedFormat,
    OpenRequest,
};

use crate::decode::DecoderStream;
use crate::{EngineError, DEFAULT_PREROLL};

#[derive(Debug, Clone)]
pub struct PlayConfig {
    pub device: DeviceSelector,
    pub preroll: Duration,
}

impl Default for PlayConfig {
    fn default() -> Self {
        Self {
            device: DeviceSelector::Default,
            preroll: DEFAULT_PREROLL,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PlayEvent {
    Opened {
        source_codec: String,
        sample_rate: u32,
        bits_per_sample: u16,
        channels: u16,
        duration: Option<Duration>,
        negotiated: NegotiatedFormat,
        stale_hog_released: Option<i32>,
    },
    Started {
        preroll: Duration,
    },
    Position {
        played: Duration,
        underruns: u64,
    },
    Finished {
        interrupted: bool,
    },
}

#[derive(Debug, Clone)]
pub struct PlayReport {
    pub interrupted: bool,
    pub underruns: u64,
    pub hog_released: bool,
    pub stale_hog_released: Option<i32>,
    pub negotiated: NegotiatedFormat,
}

pub fn play_file(
    path: &Path,
    output: &mut dyn AudioOutput,
    config: &PlayConfig,
    cancel: &Arc<AtomicBool>,
    mut on_event: impl FnMut(PlayEvent),
) -> Result<PlayReport, EngineError> {
    if cancel.load(Ordering::Acquire) {
        return Err(EngineError::Audio(AudioError::Cancelled));
    }
    let mut decoder = DecoderStream::open(path)?;
    let mut pending = None;
    if decoder.info().bits_per_sample == 0 || decoder.info().channels == 0 {
        pending = decoder.next_frames()?;
        if decoder.info().bits_per_sample == 0 || decoder.info().channels == 0 {
            return Err(EngineError::Unsupported(format!(
                "{} produced no PCM frames",
                path.display()
            )));
        }
    }
    let info = decoder.info().clone();
    let preroll_frames = (config.preroll.as_secs_f64() * f64::from(info.sample_rate))
        .ceil()
        .max(0.0) as u64;
    // The ring holds the requested preroll plus one second, clamped to 1..=60 seconds.
    let ring_frames = preroll_frames
        .saturating_add(u64::from(info.sample_rate))
        .clamp(
            u64::from(info.sample_rate),
            u64::from(info.sample_rate) * 60,
        )
        .max(1);
    let mut stream = output.open(OpenRequest {
        device: config.device.clone(),
        sample_rate: info.sample_rate,
        channels: info.channels,
        bits_per_sample: info.bits_per_sample,
        buffer_frames: u32::try_from(ring_frames).unwrap_or(u32::MAX),
        cancel: Arc::clone(cancel),
    })?;
    let negotiated = stream.negotiated().clone();
    let stale = stream.stats().stale_hog_released;
    on_event(PlayEvent::Opened {
        source_codec: info.codec.clone(),
        sample_rate: info.sample_rate,
        bits_per_sample: info.bits_per_sample,
        channels: info.channels,
        duration: info.duration,
        negotiated: negotiated.clone(),
        stale_hog_released: stale,
    });

    let pump = pump(Pump {
        stream: &mut *stream,
        decoder: &mut decoder,
        pending,
        info: &info,
        preroll_frames,
        preroll: config.preroll,
        cancel,
        on_event: &mut on_event,
    });
    let _ = stream.stop();
    let stats = stream.stats();
    drop(stream);
    let interrupted = matches!(
        pump,
        Ok(true) | Err(EngineError::Audio(AudioError::Cancelled))
    );
    if let Err(error) = pump {
        if !interrupted {
            return Err(error);
        }
    }
    on_event(PlayEvent::Finished { interrupted });
    Ok(PlayReport {
        interrupted,
        underruns: stats.underruns,
        hog_released: stats.hog_released,
        stale_hog_released: stats.stale_hog_released,
        negotiated,
    })
}

struct Pump<'a, F> {
    stream: &'a mut dyn voon_audio::OutputStream,
    decoder: &'a mut DecoderStream,
    pending: Option<Vec<i32>>,
    info: &'a crate::decode::SourceInfo,
    preroll_frames: u64,
    preroll: Duration,
    cancel: &'a AtomicBool,
    on_event: &'a mut F,
}

fn pump<F: FnMut(PlayEvent)>(pump: Pump<'_, F>) -> Result<bool, EngineError> {
    let Pump {
        stream,
        decoder,
        mut pending,
        info,
        preroll_frames,
        preroll,
        cancel,
        on_event,
    } = pump;
    let frame_bytes = stream.negotiated().bytes_per_frame as usize;
    let mut queued_frames = 0u64;
    let mut started = false;
    let mut last_report = 0.0f64;
    let mut saw_audio = false;
    let start_after = preroll_frames.max(1);

    loop {
        if cancel.load(Ordering::Acquire) {
            return Ok(true);
        }
        let samples = if let Some(samples) = pending.take() {
            samples
        } else {
            match decoder.next_frames()? {
                Some(samples) => samples,
                None => break,
            }
        };
        if samples.is_empty() {
            continue;
        }
        saw_audio = true;
        let mut packed = Vec::new();
        pack_interleaved(
            &samples,
            info.bits_per_sample,
            stream.negotiated(),
            &mut packed,
        )?;
        let mut offset = 0;
        while offset < packed.len() {
            if cancel.load(Ordering::Acquire) {
                return Ok(true);
            }
            if !started && queued_frames > 0 && stream.free_bytes() < frame_bytes {
                stream.start()?;
                started = true;
                on_event(PlayEvent::Started { preroll });
            }
            let wrote = match stream.write(&packed[offset..]) {
                Ok(0) => {
                    if !started && queued_frames > 0 {
                        stream.start()?;
                        started = true;
                        on_event(PlayEvent::Started { preroll });
                    }
                    report_position(stream, info.sample_rate, &mut last_report, on_event);
                    thread::park_timeout(Duration::from_millis(5));
                    continue;
                }
                Ok(bytes) => bytes,
                Err(AudioError::Cancelled) => return Ok(true),
                Err(error) => return Err(error.into()),
            };
            offset += wrote;
            queued_frames += (wrote / frame_bytes.max(1)) as u64;
            if !started && queued_frames >= start_after {
                stream.start()?;
                started = true;
                on_event(PlayEvent::Started { preroll });
            }
        }
    }

    if !saw_audio {
        return Err(EngineError::Unsupported(
            "file produced no audio frames".into(),
        ));
    }
    // Mark the producer finished before starting a short file so the callback
    // treats the tail as end-of-stream silence rather than an underrun.
    stream.mark_producer_done();
    if !started {
        stream.start()?;
        on_event(PlayEvent::Started { preroll });
    }
    if cancel.load(Ordering::Acquire) {
        return Ok(true);
    }
    match stream.wait_until_drained(cancel)? {
        DrainStatus::Cancelled => Ok(true),
        DrainStatus::Finished => {
            report_position(stream, info.sample_rate, &mut last_report, on_event);
            Ok(false)
        }
    }
}

fn report_position(
    stream: &dyn voon_audio::OutputStream,
    sample_rate: u32,
    last_report: &mut f64,
    on_event: &mut impl FnMut(PlayEvent),
) {
    let frames = stream.played_bytes() / u64::from(stream.negotiated().bytes_per_frame.max(1));
    let seconds = frames as f64 / f64::from(sample_rate.max(1));
    if seconds < *last_report + 5.0 && *last_report != 0.0 {
        return;
    }
    if seconds == 0.0 {
        return;
    }
    *last_report = seconds;
    on_event(PlayEvent::Position {
        played: Duration::from_secs_f64(seconds),
        underruns: stream.underruns(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::decode_interleaved;
    use voon_audio::MemoryOutput;

    fn write_pcm_wav(path: &Path, samples: &[i32]) {
        let channels: u16 = 2;
        let bits: u16 = 16;
        let rate: u32 = 44100;
        let bytes_per_sample = 2usize;
        let block_align = channels as usize * bytes_per_sample;
        let data_len = samples.len() * bytes_per_sample;
        let mut data = Vec::with_capacity(data_len);
        for sample in samples {
            data.extend_from_slice(&sample.to_le_bytes()[..bytes_per_sample]);
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
    fn memory_playback_is_bit_exact_and_starts_after_audio_exists() {
        let dir = std::env::temp_dir().join(format!("voon-play-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tone.wav");
        let samples = [0, -1, 32767, -32768];
        write_pcm_wav(&path, &samples);
        let (_info, decoded) = decode_interleaved(&path).unwrap();
        assert_eq!(decoded, samples);

        let mut output = MemoryOutput::new();
        let cancel = Arc::new(AtomicBool::new(false));
        let mut started = false;
        let report = play_file(
            &path,
            &mut output,
            &PlayConfig {
                device: DeviceSelector::Default,
                preroll: Duration::ZERO,
            },
            &cancel,
            |event| {
                if matches!(event, PlayEvent::Started { .. }) {
                    started = true;
                }
            },
        )
        .unwrap();
        assert!(started);
        assert!(output.started());
        assert!(!report.interrupted);
        assert!(report.hog_released);
        assert_eq!(
            output.captured(),
            vec![0x00, 0x00, 0xff, 0xff, 0xff, 0x7f, 0x00, 0x80]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancel_before_open_does_not_start_playback() {
        let dir = std::env::temp_dir().join(format!("voon-play-cancel-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tone.wav");
        write_pcm_wav(&path, &[0, 0]);
        let mut output = MemoryOutput::new();
        let cancel = Arc::new(AtomicBool::new(true));
        let error = play_file(
            &path,
            &mut output,
            &PlayConfig {
                preroll: Duration::ZERO,
                ..PlayConfig::default()
            },
            &cancel,
            |_| {},
        )
        .unwrap_err();
        assert!(matches!(error, EngineError::Audio(AudioError::Cancelled)));
        assert!(!output.started());
        assert!(output.captured().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
