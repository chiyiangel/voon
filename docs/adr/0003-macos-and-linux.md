# ADR 0003: macOS and Linux, one codebase

Status: accepted

## Context

Voon started as a macOS app. The owner also wants it on Linux, and the
development box is Linux, so Linux is also the platform that can be built
and run automatically. GPUI supports both.

## Decision

Voon targets macOS and Linux (X11 and Wayland) from the same crates.
`voon-core` stays free of platform code. Platform differences in `voon-app`
are kept behind `cfg(target_os = ...)` at the edges; today that is only
the default library root (`/Volumes/NAS1/Music` on macOS, `~/Music` on
Linux). macOS is checked by hand on the owner's Mac; Linux is built,
linted, tested and launched on the development box. There is no hosted CI
for now.

## Alternatives

- **macOS only.** Simpler, but nothing GUI-related could be verified off
  the owner's Mac.
- **Linux only through a web UI.** Two front ends to maintain, and GPUI is
  already the chosen toolkit.
