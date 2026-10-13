//! Shared configuration for the Voon daemon.

mod config;

pub use config::{load_config, Config, ConfigError};
