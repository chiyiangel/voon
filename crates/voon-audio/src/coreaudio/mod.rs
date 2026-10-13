//! CoreAudio HAL exclusive output.
//!
//! Playback setup, in order:
//! 1. Release a stale hog (owner PID is not running). Never steal a live owner.
//! 2. Take hog mode and check that the property reads back as our PID.
//! 3. Disable mixing when `kAudioDevicePropertySupportsMixing` is settable.
//! 4. Set the nominal sample rate and wait for the property listener before any audio.
//! 5. Set an integer physical format when the device advertises one, and wait until it reads back.
//! 6. Point the virtual format at that same integer layout when the device accepts it.
//! 7. Only then start the IO proc. The proc copies from a lock-free ring and does no I/O.
//!
//! Drop, normal return, SIGINT, and SIGTERM all run the same shutdown: stop the device,
//! restore mixing, and set hog mode back to -1. A dead owner left behind by SIGKILL is
//! cleared the next time the device is opened.

mod hal;
mod stream;

pub use stream::CoreAudioOutput;
