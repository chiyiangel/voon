# Voon

Voon is a headless music server in the spirit of Roon. It reads a library from a NAS mounted over SMB or NFS, plays bit-perfect audio to a USB DAC, and is meant to be controlled from a phone on the LAN.

Phase 1 targets macOS. This repository currently contains the first slice of milestone M1: a Cargo workspace, configuration and a launchd example, a CoreAudio exclusive-mode output behind a backend trait, a Symphonia playback pipeline, the SQLite schema, a `/health` endpoint, and `voon-audiotest`.

The requirements document is [`docs/PRD.md`](docs/PRD.md). The Mac DAC procedure is [`docs/TESTING-macos.md`](docs/TESTING-macos.md).

## Crates

| Crate | Role |
| --- | --- |
| `voon-core` | TOML config |
| `voon-audio` | `AudioOutput` trait, in-memory test sink, macOS CoreAudio HAL backend |
| `voon-engine` | Symphonia decode (FLAC, WAV, AIFF, ALAC) and preroll playback |
| `voon-library` | SQLite schema, FTS5 search, DSD `playable = 0` |
| `voon-api` | `GET /health` |
| `voon-daemon` | `voon` binary: migrate the database, clear a stale hog, serve health |
| `voon-audiotest` | List devices and play one file in hog mode |

A later ALSA backend can implement `AudioOutput` without changing the engine. CoreAudio code is compiled only on macOS (`cfg(target_os = "macos")`). `cpal` is not used.

## Build

```bash
cargo test --workspace --locked
cargo build --release -p voon-daemon -p voon-audiotest
```

Rust 1.85 or newer is required. On macOS, `coreaudio-sys` runs bindgen and needs `libclang` (the Xcode command-line tools provide it). CI sets `LIBCLANG_PATH` from `xcrun --find clang`.

## Configuration

Copy [`config/voon.example.toml`](config/voon.example.toml) and edit the paths. `database_path` belongs on local disk. `library_path` is the mounted NAS tree. `output_device` is a CoreAudio UID or a unique device name; omit it to use the default output.

```bash
voon --config /usr/local/etc/voon/config.toml
```

The process listens on `listen` (default `0.0.0.0:8876`) and answers `GET /health` with `{"status":"ok","service":"voon"}`. If the DAC is unplugged, startup logs a warning and still serves health. On Linux the audio backend reports that CoreAudio is unavailable and the daemon still starts.

## launchd

[`packaging/launchd/com.voon.daemon.plist`](packaging/launchd/com.voon.daemon.plist) runs `/usr/local/bin/voon`. Install steps on the Mac:

1. `cargo build --release -p voon-daemon`
2. Copy `target/release/voon` to `/usr/local/bin/voon` (or change `ProgramArguments` to that binary).
3. Copy the example config to `/usr/local/etc/voon/config.toml` and edit the paths.
4. `sudo mkdir -p /usr/local/var/log/voon`
5. For a user agent, copy the plist to `~/Library/LaunchAgents/` and replace the `/usr/local` paths if the binary is not installed system-wide. For a boot agent, copy it to `/Library/LaunchDaemons/`.
6. `launchctl bootstrap gui/$UID ~/Library/LaunchAgents/com.voon.daemon.plist`

`ProcessType` is `Interactive` so App Nap does not stall playback. `KeepAlive` restarts the process. A missing DAC must not crash-loop; the daemon continues after a device error.

## Exclusive output

`voon-audiotest play` and the engine use this order:

1. Release a stale hog whose PID is not alive. A live owner is left alone.
2. Take hog mode (`kAudioDevicePropertyHogMode`).
3. Disable mixing when the property is settable.
4. Set the nominal sample rate to the file rate and wait until the property listener fires and a read-back matches. If the device is already on that rate, playback starts without waiting for a change that will not arrive.
5. Choose a signed-integer linear PCM physical format (no bit truncation, prefer non-mixable and interleaved) and set it, then try the same integer virtual format.
6. Start the IO callback only after that. The callback copies from a ring; it does not read the file.
7. On a normal exit, restore the saved formats and rate, re-enable mixing, and set hog to `-1`. On Ctrl-C or SIGTERM the hog is released immediately and the slow format restore is skipped. A second Ctrl-C, or `kill -9`, skips destructors; the next play or daemon start clears that stale hog.

Volume is unity. The amplifier is the volume control.

## `voon-audiotest`

```bash
cargo build --release -p voon-audiotest
./target/release/voon-audiotest list
./target/release/voon-audiotest list --device "DAC"
./target/release/voon-audiotest decode track.flac
./target/release/voon-audiotest play --device "DAC" --preroll-secs 30 track.flac
```

`decode` works on Linux. `list` and `play` require macOS and a device that supports hog mode. Built-in speakers often do not. Exit status is 0 on success, 1 on error, 130 after SIGINT, and 143 after SIGTERM.

## Library schema

`tracks` stores `file_size` and `mtime_ns` for a later incremental scan, `codec_kind` (`pcm` or `dsd`), and `playable`. `.dsf` and `.dff` are indexed as DSD with `playable = 0`. Default search and list queries hide those rows, so phase 2 can set `playable = 1` without rescanning. `.m4a` and `.mp4` stay unplayable until a scanner confirms ALAC. Search uses FTS5 (`unicode61`) on title, artist, album, album artist, and genre.

## Not in this slice

Library scanning, gapless playlists, the playback REST and WebSocket API, Bonjour, and the phone web UI are later work. DSD is stored and is not played.
