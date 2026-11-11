# Voon

Voon organizes a music library on a NAS. It is a desktop app for macOS and
Linux, written in Rust with [GPUI](https://www.gpui.rs). The interface is in
Chinese.

Status: phase 1 is implemented; platform acceptance is pending. The native app
includes library scanning, browsing, metadata editing and local organization.
See the [ticket index](docs/specs/0001-phase-1/tickets/README.md) for verification
evidence and the remaining platform acceptance checks.

## Phase 1 scope

- **Read-only toward the library.** Voon indexes, browses and edits metadata
  in its own database; it never changes files on the NAS
  ([ADR 0001](docs/adr/0001-read-only-phase-1.md)).
- **Albums, Discs and Collections.** `CD1`, `CD 02`, `Disc 2` subfolders are
  Discs of one Album. Folders that only hold albums (a box-set series, a
  performer folder) are Collections. An album folder that contains another
  album folder keeps both albums and lists the outer one in 待处理.
- **Loose audio in a library root** is grouped into albums by its album and
  album artist tags; files without an album tag are listed in 待处理 instead
  of forming an album.
- **Whole-disc images.** A CUE sheet plus one image file becomes Virtual
  tracks; files are never split
  ([ADR 0002](docs/adr/0002-virtual-cue-tracks.md)). CUE sheets in UTF-8,
  UTF-16, GBK and Shift-JIS are decoded.
- **Metadata priority:** user edits > embedded tags > CUE sheet > folder
  name. Every field records its source. The folder name supplies year,
  format, sample rate/bit depth, DSD rate, catalog number and edition (e.g.
  `(1988) [24-96] [438317-2] [台湾首版]`).
- **Classical music:** album-level composer, performers, conductor and
  orchestra; browse by composer. Orchestra is set by user edits only in
  phase 1 (no common tag carries it).
- **Classification:** automatic facets (format, sample rate/bit depth, DSD vs
  PCM, language, decade), user tags and manual collections. Genre comes from
  tags only.
- **Covers:** `cover`/`folder`/`front` images next to the music or in a
  `Booklet`/`Scans` folder, else embedded art. All other non-music files are
  ignored.
- **Local index:** SQLite on the local computer, so browsing works with the
  NAS unmounted. Ordinary rescans reuse successful tag reads for unchanged
  files and retry I/O or parsing failures. Unchanged unsupported formats are
  not retried automatically. Several library roots are supported.
- **Renamed or moved folders:** user edits, user tags and manual collection
  membership are keyed by album path. When a rescan finds an album path gone
  and a new one with the same content (relative file paths and sizes), it
  lists the pair in 待处理 as a possible rename; confirming moves the user's
  data to the new path ([ADR 0004](docs/adr/0004-rename-candidates.md)).
- **Background scanning:** the first scan runs in the background, shows
  albums as they are found, and can be cancelled.
- **Needs attention (待处理):** albums that could not be recognised or have
  conflicting information, each with the reason.
- **Editing:** album-level fields, merging and splitting albums, choosing the
  cover.
- **No playback.** At most "open in the default player".
- **macOS packaging:** at the end of phase 1, a `.app` bundle with an icon.
  No code signing or notarization (personal use).

Formats: FLAC, APE, WavPack, WAV, AIFF, DSF, DFF, MP3, M4A/ALAC, AAC, Ogg
Vorbis, Opus. Tags are read from all of them except DFF, which relies on the
folder name and CUE sheet in phase 1.

## Phase 2

- Online lookup (MusicBrainz, Discogs), including filling in the orchestra.
- DFF tags and stream properties.
- Work-level grouping for classical music (one work, many recordings).
- Smart collections.
- Write operations, each previewed, confirmed and undoable, off by default:
  rename and move, tag writing, CUE splitting, junk cleanup.

## Known gaps

- DFF: no tags or stream properties yet (phase 2); recognised as DSD from
  the extension and folder name.
- Rename detection works within one library root and needs the audio files
  unchanged; an album moved to another root, or re-encoded, is not paired.
  Identical copies (same files, same sizes) are never paired automatically.
- CUE encoding detection can still misjudge very short legacy sheets.
- `libc` is held below 0.2.190 because GPUI's `xattr 0.2` dependency does
  not build on Linux with newer versions (see `crates/voon-app/Cargo.toml`).

## Layout

| Path | What |
|---|---|
| `crates/voon-core` | Model, scanner, CUE and folder-name parsing, tags, SQLite index. No GUI; builds and tests on Linux. |
| `crates/voon-app` | The GPUI desktop app (macOS, Linux). |
| `GLOSSARY.md` | Domain terms. |
| `docs/adr` | Decisions. |

## Platforms

| Platform | Status |
|---|---|
| macOS (Apple Silicon) | Current implementation builds locally; agent window checks passed. Mobert's manual acceptance is pending. |
| Linux (X11, Wayland) | Baseline was verified on Debian 13 (X11). The new phase 1 UI still needs Linux build and window acceptance; Wayland remains untested. |

See [ADR 0003](docs/adr/0003-macos-and-linux.md).

### Default library root

The first-launch screen suggests the first command-line argument, else
`VOON_ROOT`, else the following path. Choose **开始扫描** to add it, or choose
another folder. Previously added libraries reload from the local index:

- macOS: `/Volumes/NAS1/Music`
- Linux: `~/Music`

### Linux build dependencies

A C/C++ toolchain, `pkg-config`, and the development files GPUI links
against (X11/xcb, xkbcommon, Wayland, fontconfig, freetype, Vulkan). A Vulkan
driver is needed at run time; `mesa-vulkan-drivers` includes the lavapipe
software driver for machines without a GPU. On Debian/Ubuntu:

```bash
sudo apt-get install -y build-essential clang cmake pkg-config \
  libfreetype-dev libfontconfig-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libxcb1-dev libx11-dev libx11-xcb-dev libwayland-dev \
  libvulkan-dev libvulkan1 mesa-vulkan-drivers libzstd-dev
```

A CJK font (for example `fonts-noto-cjk`) is needed for the Chinese UI.

## Development

```bash
cargo test --workspace
cargo run -p voon-app -- /path/to/music    # or set VOON_ROOT
```

Use an isolated index and temporary music folder for development:

```bash
voon_dev_dir="$(mktemp -d)"
mkdir -p "$voon_dev_dir/music"
VOON_INDEX_PATH="$voon_dev_dir/library-0002.sqlite3" \
  cargo run -p voon-app -- "$voon_dev_dir/music"
```

`VOON_INDEX_PATH` must be absolute. It bypasses the default index and saved
index selection. The temporary path above isolates this run from your normal
index. `VOON_ROOT` only changes the suggested music folder.

Run `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`
before pushing. There is no hosted CI yet.

### macOS application bundle

On a Mac with full Xcode (including the Metal toolchain), run:

```bash
bash scripts/package-macos.sh          # release build
bash scripts/package-macos.sh --debug  # faster local preview
```

The script creates `dist/Voon.app` with a generated icon. Copy it to
Applications, open it, and optionally keep it in the Dock. The script does
not add code signing or notarization. If macOS blocks your own local build,
follow Apple's [instructions for opening an unnotarized app](https://support.apple.com/en-us/102445):
after attempting to open it, use **System Settings → Privacy & Security →
Open Anyway**, and confirm the application name. Do this only for a build
you trust; the packaging script does not change macOS security settings.

The default index is stored at
`~/Library/Application Support/Voon/library-0002.sqlite3` on macOS and
`$XDG_DATA_HOME/voon/library-0002.sqlite3` (or
`~/.local/share/voon/library-0002.sqlite3`) on Linux. A different index chosen
through the app is remembered in `current-index-path` beside the default
index. The fixed `VOON_INDEX_PATH` override bypasses that selection file.

This reliability milestone starts with a separate index and a fresh scan.
The previous `library.sqlite3` is kept in place; its edits and organization
are not imported into the fresh index. Use the new-index action in Settings
or on the startup error screen to create another index without replacing the
existing file. Music files remain read-only.

Removing a Library root hides its scan results and retains edits, tags and
manual-collection membership for when the same path is added again.


## License

[MIT](LICENSE).
