# Voon

Voon organizes a music library on a NAS. It is a macOS app written in Rust
with [GPUI](https://www.gpui.rs). The interface is in Chinese.

Status: early. `voon-core` (library model, scanner, index) works and is
tested; `voon-app` is a minimal window while the layout is prototyped.

## Phase 1 scope

- **Read-only toward the library.** Voon indexes, browses and edits metadata
  in its own database; it never changes files on the NAS
  ([ADR 0001](docs/adr/0001-read-only-phase-1.md)).
- **Albums, Discs and Collections.** `CD1`, `CD 02`, `Disc 2` subfolders are
  Discs of one Album. Folders that only hold albums (a box-set series, a
  performer folder) are Collections.
- **Whole-disc images.** A CUE sheet plus one image file becomes Virtual
  tracks; files are never split
  ([ADR 0002](docs/adr/0002-virtual-cue-tracks.md)). CUE sheets in UTF-8,
  UTF-16, GBK and Shift-JIS are decoded.
- **Metadata priority:** user edits > embedded tags > CUE sheet > folder
  name. Every field records its source. The folder name supplies year,
  format, sample rate/bit depth, DSD rate, catalog number and edition (e.g.
  `(1988) [24-96] [438317-2] [台湾首版]`).
- **Classical music:** album-level composer, performers, conductor and
  orchestra; browse by composer.
- **Classification:** automatic facets (format, sample rate/bit depth, DSD vs
  PCM, language, decade), user tags and manual collections. Genre comes from
  tags only.
- **Covers:** `cover`/`folder`/`front` images next to the music or in a
  `Booklet`/`Scans` folder, else embedded art. All other non-music files are
  ignored.
- **Local index:** SQLite on the Mac, so browsing works with the NAS
  unmounted. Rescans re-read only files whose size or modification time
  changed. Several library roots.
- **Background scanning:** the first scan runs in the background, shows
  albums as they are found, and can be cancelled.
- **Needs attention (待处理):** albums that could not be recognised or have
  conflicting information, each with the reason.
- **Editing:** album-level fields, merging and splitting albums, choosing the
  cover.
- **No playback.** At most "open in the default player".

Formats: FLAC, APE, WavPack, WAV, AIFF, DSF, DFF, MP3, M4A/ALAC, AAC, Ogg
Vorbis, Opus.

## Phase 2

- Online lookup (MusicBrainz, Discogs).
- Work-level grouping for classical music (one work, many recordings).
- Smart collections.
- Write operations, each previewed, confirmed and undoable, off by default:
  rename and move, tag writing, CUE splitting, junk cleanup.

## Known gaps

- DSF: stream properties are read from the header; the ID3v2 tag inside is
  not read yet. DFF: no tags or properties yet. Both are recognised as DSD
  from the extension and folder name.
- Orchestra has no common tag key, so it is only set by user edits for now.
- CUE encoding detection can still misjudge very short legacy sheets.

## Layout

| Path | What |
|---|---|
| `crates/voon-core` | Model, scanner, CUE and folder-name parsing, tags, SQLite index. No GUI; builds and tests on Linux. |
| `crates/voon-app` | The GPUI macOS app. |
| `GLOSSARY.md` | Domain terms. |
| `docs/adr` | Decisions. |

## Development

```bash
cargo test -p voon-core
cargo run -p voon-app -- /Volumes/NAS1/Music   # macOS; or set VOON_ROOT
```

Run `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`
before pushing; CI checks both.
