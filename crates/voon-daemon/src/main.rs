use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use clap::Parser;
use voon_core::load_config;

#[derive(Debug, Parser)]
#[command(name = "voon", about = "Headless music server")]
struct Cli {
    /// Path to the TOML configuration file.
    #[arg(long)]
    config: PathBuf,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let config = load_config(&cli.config)?;
    voon_daemon::prepare(&config)?;

    let shutdown = Arc::new(AtomicBool::new(false));
    // `SigId` unregisters the hook when dropped, so these live for the process.
    let _sigint = signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&shutdown))?;
    let _sigterm =
        signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&shutdown))?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(voon_daemon::serve(config.listen, shutdown))
}
