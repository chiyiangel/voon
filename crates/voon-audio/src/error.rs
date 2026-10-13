#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("audio backend '{backend}' is not available on this operating system")]
    PlatformUnsupported { backend: &'static str },
    #[error("audio device not found: {selector}")]
    DeviceNotFound { selector: String },
    #[error("more than one audio device matches '{selector}': {matches}")]
    AmbiguousDevice { selector: String, matches: String },
    #[error("device is exclusively owned by live process {pid}")]
    DeviceBusy { pid: i32 },
    #[error("this process already holds exclusive access to the device")]
    AlreadyExclusive,
    #[error("device does not support hog mode ({detail})")]
    HogUnsupported { detail: String },
    #[error("sample rate {rate} Hz is not in the device's advertised rates ({supported})")]
    SampleRateUnsupported { rate: u32, supported: String },
    #[error("timed out waiting for the device to confirm sample rate {rate} Hz ({detail})")]
    SampleRateTimeout { rate: u32, detail: String },
    #[error("no integer physical format for {rate} Hz, {channels} ch, {bits} bit ({detail})")]
    IntegerFormatUnavailable {
        rate: u32,
        channels: u16,
        bits: u16,
        detail: String,
    },
    #[error("failed to set the stream physical format: {detail}")]
    PhysicalFormat { detail: String },
    #[error("CoreAudio {operation} failed: {message}")]
    CoreAudio {
        operation: &'static str,
        status: i32,
        message: String,
    },
    #[error("playback cancelled")]
    Cancelled,
    #[error("{0}")]
    Message(String),
}

impl AudioError {
    pub fn message(detail: impl Into<String>) -> Self {
        Self::Message(detail.into())
    }
}

/// Render an `OSStatus` as a four-character code when the bytes are printable.
pub fn format_os_status(status: i32) -> String {
    let bytes = (status as u32).to_be_bytes();
    if bytes
        .iter()
        .all(|byte| byte.is_ascii_graphic() || *byte == b' ')
    {
        let text = String::from_utf8_lossy(&bytes);
        format!("'{text}' ({status})")
    } else {
        format!("{status}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_status_fourcc_is_readable() {
        let fmt = u32::from_be_bytes(*b"fmt?");
        let text = format_os_status(fmt as i32);
        assert!(text.contains("'fmt?'"), "{text}");
    }
}
