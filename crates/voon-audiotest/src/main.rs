//! Probe CoreAudio exclusive playback on a Mac with a USB DAC attached.
//!
//! `list` and `play` need macOS. `decode` runs anywhere Symphonia can read the file.
//! Machine-readable fields go to stdout. Logs go to stderr.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use clap::{Parser, Subcommand};
use voon_audio::{
    format_hz, format_rate_list, open_system, AudioError, DeviceSelector, ListedFormat,
};
use voon_engine::{play_file, PlayConfig, PlayEvent, PlayReport};

#[derive(Debug, Parser)]
#[command(
    name = "voon-audiotest",
    about = "List CoreAudio devices and play one file in hog mode"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print output devices, hog state, nominal rate, and physical formats.
    List {
        /// Device UID, exact name, or a unique name substring. Omit for every device.
        #[arg(long)]
        device: Option<String>,
    },
    /// Decode one file and play it with hog mode and an integer physical format.
    Play {
        /// Device UID, exact name, or a unique name substring. Omit for the default output.
        #[arg(long)]
        device: Option<String>,
        /// Seconds of audio to queue before the device starts. 0 starts after the first packet.
        #[arg(long, default_value_t = 30)]
        preroll_secs: u64,
        file: PathBuf,
    },
    /// Decode one file and print its PCM description. Does not open a device.
    Decode { file: PathBuf },
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(1)
        }
    }
}

fn run() -> anyhow::Result<ExitCode> {
    match Cli::parse().command {
        Command::List { device } => {
            list_devices(device.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Decode { file } => {
            decode_file(&file)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Play {
            device,
            preroll_secs,
            file,
        } => play(device.as_deref(), preroll_secs, &file),
    }
}

fn list_devices(query: Option<&str>) -> anyhow::Result<()> {
    let output = open_system()?;
    let mut devices = output.list_devices()?;
    if let Some(query) = query {
        let selector = DeviceSelector::Query(query.to_string());
        let matched = voon_audio::resolve_device(&devices, &selector)?.uid.clone();
        devices.retain(|device| device.uid == matched);
    }
    if devices.is_empty() {
        println!("devices: 0");
        return Ok(());
    }
    println!("devices: {}", devices.len());
    for device in &devices {
        println!("uid: {}", device.uid);
        println!("name: {}", device.name);
        println!("default: {}", yes_no(device.is_default));
        println!(
            "nominal_rate_hz: {}",
            format_hz(device.nominal_sample_rate_hz)
        );
        println!("hog: {}", device.hog);
        println!("sample_rates: {}", format_rate_list(&device.sample_rates));
        println!(
            "current_physical_format: {}",
            device
                .current_physical_format
                .as_ref()
                .map(format_listed)
                .unwrap_or_else(|| "none".to_string())
        );
        println!("physical_formats: {}", device.physical_formats.len());
        for format in &device.physical_formats {
            println!("physical_format: {}", format_listed(format));
        }
        println!("---");
    }
    Ok(())
}

fn format_listed(format: &ListedFormat) -> String {
    format!(
        "{} bits={} channels={} bytes_per_frame={} rates={} flags={}",
        format.encoding,
        format.bits_per_channel,
        format.channels,
        format.bytes_per_frame,
        voon_audio::format_rate_range(format.rate_min_hz, format.rate_max_hz),
        format.flags
    )
}

fn decode_file(path: &std::path::Path) -> anyhow::Result<()> {
    let (info, samples) = voon_engine::decode_interleaved(path)?;
    println!("file: {}", path.display());
    println!("codec: {}", info.codec);
    println!("sample_rate: {}", info.sample_rate);
    println!("channels: {}", info.channels);
    println!("bits_per_sample: {}", info.bits_per_sample);
    println!(
        "frames: {}",
        info.frames
            .map(|frames| frames.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    );
    println!(
        "duration_ms: {}",
        info.duration
            .map(|duration| duration.as_millis().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    );
    println!("samples: {}", samples.len());
    Ok(())
}

fn play(
    device: Option<&str>,
    preroll_secs: u64,
    path: &std::path::Path,
) -> anyhow::Result<ExitCode> {
    let selector = match device {
        Some(query) => DeviceSelector::Query(query.to_string()),
        None => DeviceSelector::Default,
    };
    let sigint = Arc::new(AtomicBool::new(false));
    let sigterm = Arc::new(AtomicBool::new(false));
    let cancel = Arc::new(AtomicBool::new(false));
    // The shutdown hook is registered first, so the first signal only sets the
    // flags. A second signal sees the flag and exits without running destructors;
    // the next `play` or daemon start releases that stale hog.
    // `SigId` unregisters the hook when dropped. Keep them until playback returns.
    // Shutdown is registered before the flag so the first signal only sets the
    // flag; a second signal sees it and `_exit`s without running destructors.
    let _sigint_exit = signal_hook::flag::register_conditional_shutdown(
        signal_hook::consts::SIGINT,
        130,
        Arc::clone(&sigint),
    )?;
    let _sigint_flag =
        signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&sigint))?;
    let _sigint_cancel =
        signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&cancel))?;
    let _sigterm_exit = signal_hook::flag::register_conditional_shutdown(
        signal_hook::consts::SIGTERM,
        143,
        Arc::clone(&sigterm),
    )?;
    let _sigterm_flag =
        signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&sigterm))?;
    let _sigterm_cancel =
        signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&cancel))?;

    let mut output = open_system()?;
    let config = PlayConfig {
        device: selector.clone(),
        preroll: Duration::from_secs(preroll_secs),
    };
    println!("file: {}", path.display());
    let report = match play_file(path, &mut *output, &config, &cancel, print_event) {
        Ok(report) => report,
        Err(voon_engine::EngineError::Audio(AudioError::Cancelled)) => {
            return Ok(signal_exit(&sigterm));
        }
        Err(error) => return Err(error.into()),
    };
    print_report(&report);
    if report.interrupted {
        Ok(signal_exit(&sigterm))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

fn print_event(event: PlayEvent) {
    match event {
        PlayEvent::Opened {
            source_codec,
            sample_rate,
            bits_per_sample,
            channels,
            duration,
            negotiated,
            stale_hog_released,
        } => {
            println!("source_codec: {source_codec}");
            println!("source_rate_hz: {sample_rate}");
            println!("source_bits: {bits_per_sample}");
            println!("source_channels: {channels}");
            println!(
                "source_duration_ms: {}",
                duration
                    .map(|duration| duration.as_millis().to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            );
            if let Some(pid) = stale_hog_released {
                println!("hog_stale_released_pid: {pid}");
            }
            println!("device_rate_requested_hz: {}", negotiated.rate.requested);
            println!(
                "sample_rate_confirmed_hz: {}",
                format_hz(negotiated.rate.observed_hz)
            );
            println!(
                "sample_rate_listener: {}",
                if negotiated.rate.already_set {
                    "already_set"
                } else if negotiated.rate.listener_fired {
                    "yes"
                } else {
                    "no"
                }
            );
            println!("sample_rate_wait_ms: {}", negotiated.rate.wait.as_millis());
            println!("mixing_disabled: {}", yes_no(negotiated.mixing_disabled));
            println!("physical_integer: {}", yes_no(negotiated.physical_integer));
            println!("non_mixable: {}", yes_no(negotiated.non_mixable));
            println!("physical: {}", negotiated.physical_summary);
            println!("virtual: {}", negotiated.virtual_summary);
        }
        PlayEvent::Started { preroll } => {
            println!("preroll_secs: {}", preroll.as_secs());
            println!("started: yes");
        }
        PlayEvent::Position { played, underruns } => {
            println!(
                "position_secs: {:.1} underruns: {underruns}",
                played.as_secs_f64()
            );
        }
        PlayEvent::Finished { interrupted } => {
            println!("finished_interrupted: {}", yes_no(interrupted));
        }
    }
}

fn print_report(report: &PlayReport) {
    println!("done: yes");
    println!("interrupted: {}", yes_no(report.interrupted));
    println!("underruns: {}", report.underruns);
    println!("hog_released: {}", yes_no(report.hog_released));
}

fn signal_exit(sigterm: &AtomicBool) -> ExitCode {
    if sigterm.load(Ordering::Acquire) {
        ExitCode::from(143)
    } else {
        ExitCode::from(130)
    }
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}
