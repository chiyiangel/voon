# macOS DAC test

Run this on the Mac with the USB DAC connected. The cloud build cannot open CoreAudio, so these steps are the exclusive-mode check.

Use the USB DAC. Built-in speakers often reject hog mode or have no integer physical format. If a step fails, copy the full `error:` line from the terminal.

Build from a checkout of this branch:

```bash
cargo build --release -p voon-audiotest
```

The binary is `target/release/voon-audiotest`. Commands below assume the current directory is the repo root.

Watch the DAC's sample-rate indicator **during** playback. After the file ends, or after Ctrl-C, the tool restores the previous rate on a normal finish and releases hog immediately on a signal, so the indicator may change again after the process exits. `sample_rate_confirmed_hz` on stdout is the rate CoreAudio confirmed before any audio was sent.

A single click when the DAC switches rate can be the DAC's own relay. Sustained crackle, a click at the start of the music, or a missing opening transient is a failure.

## 1. List devices

```bash
./target/release/voon-audiotest list
```

Expected:

- Exit status 0.
- One block whose `name:` is the USB DAC.
- `hog: free` before any playback.
- `nominal_rate_hz:` is the rate the DAC is sitting on now.
- `sample_rates:` lists the rates the DAC advertises (44.1 kHz, 48 kHz, 96 kHz, and so on, as the DAC supports).
- `physical_formats:` is greater than 0, and at least one `physical_format:` line contains `signed` (integer PCM). A `nonmixable` flag is a good sign and is not required for the list step to pass.
- `default: yes` only on the system default output. It does not have to be the DAC.

If several devices match a later name, pass `--device` with the `uid:` line instead of the name.

## 2. Decode without touching the DAC

```bash
./target/release/voon-audiotest decode /path/to/track.flac
```

Expected:

- Exit status 0.
- `codec: FLAC` (or `PCM_S16`, `PCM_S24`, `ALAC` for WAV, AIFF, and ALAC).
- `sample_rate:`, `channels:`, and `bits_per_sample:` match the file.
- The DAC indicator does not change. This command does not open a device.

Repeat with one WAV, one AIFF, and one ALAC file if you have them. `.dsf` / `.dff` are not played in this slice.

## 3. Play 44.1 kHz

Replace the device query with the DAC `uid:` or a unique substring of `name:`.

```bash
./target/release/voon-audiotest play --device "DAC" --preroll-secs 30 /path/to/44k.flac
```

Expected while the command is running:

- Stdout contains `sample_rate_confirmed_hz: 44100`.
- `sample_rate_listener:` is `yes` when the DAC had to switch, or `already_set` when it was already at 44.1 kHz. `sample_rate_wait_ms:` is the wait for that confirmation. Audio starts only after this line.
- `physical_integer: yes`.
- `mixing_disabled: yes` when the device allows it. `no` is acceptable when the property is not settable; integer `physical:` is the mixer bypass.
- `started: yes` after the preroll. About 30 seconds of silence before sound is the preroll, not a truncated start. The musical opening should still be intact once sound begins.
- The DAC indicator shows **44.1 kHz** during playback.
- No sustained pops or clicks after the rate has settled.

When the file finishes:

- `done: yes`, `interrupted: no`, `hog_released: yes`, `underruns: 0`.
- Exit status 0.
- `./target/release/voon-audiotest list --device "DAC"` shows `hog: free`.

## 4. Play 96 kHz

```bash
./target/release/voon-audiotest play --device "DAC" /path/to/96k.flac
```

Expected:

- `sample_rate_confirmed_hz: 96000`.
- The DAC indicator shows **96 kHz** during playback.
- Same hog, integer-format, and underrun expectations as step 3.
- The beginning of the track is not cut off. A short gap while the rate changes, under one second plus the preroll, is acceptable. A pop at the first note is not.

## 5. Play 192 kHz / 24-bit, when the DAC supports it

```bash
./target/release/voon-audiotest play --device "DAC" /path/to/192k24.flac
```

Expected:

- `sample_rate_confirmed_hz: 192000` and `source_bits: 24`.
- The DAC indicator shows **192 kHz** during playback.
- If the DAC has no 192 kHz rate, the command fails before audio with `sample rate 192000 Hz is not in the device's advertised rates`. Copy that error. That is a capability report, not a playback bug.

## 6. Ctrl-C releases hog

Start step 3 again and press Ctrl-C once after sound has started.

Expected:

- The process exits within a few seconds. Exit status 130.
- Stdout shows `interrupted: yes` and `hog_released: yes` if it gets that far. A fast Ctrl-C may skip the final lines; the following list command is the check that matters.
- `./target/release/voon-audiotest list --device "DAC"` shows `hog: free`.
- The Mac can play other audio through that DAC again.

## 7. SIGTERM releases hog

```bash
./target/release/voon-audiotest play --device "DAC" /path/to/44k.flac &
sleep 35
kill -TERM $!
wait
./target/release/voon-audiotest list --device "DAC"
```

`sleep 35` lets the 30 second preroll elapse so the DAC has actually started. Expected: the player exits 143, and `list` shows `hog: free`.

## 8. SIGKILL leaves a stale hog, and the next play clears it

```bash
./target/release/voon-audiotest play --device "DAC" /path/to/44k.flac &
sleep 35
kill -9 $!
./target/release/voon-audiotest list --device "DAC"
```

Expected on `list`: `hog: stale pid=` followed by the killed process id.

Then play a short file:

```bash
./target/release/voon-audiotest play --device "DAC" --preroll-secs 0 /path/to/44k.flac
```

Expected:

- Stdout contains `hog_stale_released_pid:` with that same pid.
- Playback proceeds and ends with `hog_released: yes`.
- A following `list` shows `hog: free`.

A second Ctrl-C during playback calls `_exit` and skips the hog release, same as `kill -9`. The next `play` should print `hog_stale_released_pid:` and recover. Do not leave the DAC held: if `list` still shows `stale` or `busy` after a successful play, report that.

## What to send back

Paste this filled in, plus the stdout of one `list` and one `play`:

```text
DAC model:
macOS version:
voon-audiotest commit:

list nominal_rate_hz:
list hog before play:
list sample_rates:
integer physical formats seen: yes/no

44.1 kHz file:
  sample_rate_confirmed_hz:
  sample_rate_listener:
  DAC indicator during playback:
  pops or clicks:
  truncated start:
  underruns:
  hog after exit:

96 kHz file:
  sample_rate_confirmed_hz:
  DAC indicator during playback:
  pops or clicks:
  truncated start:

192 kHz file (or the error text):
  sample_rate_confirmed_hz:
  DAC indicator during playback:

After one Ctrl-C, hog from list:
After kill -TERM, hog from list:
After kill -9, hog from list:
Next play printed hog_stale_released_pid: yes/no
Hog after that play:
```

Also copy any line that mentions `IntegerFormatUnavailable`, `timed out waiting for the device to confirm sample rate`, or `device is exclusively owned`.
