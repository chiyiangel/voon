# ADR 0001: Phase 1 never writes to the music library

Status: accepted

## Context

The library is about 450 GB on a NAS, much of it from CD rips and box sets
that are hard or impossible to obtain again. Folder layouts are inconsistent,
so any automatic rename or move is guessed from imperfect information. One
wrong batch operation could damage thousands of files before anyone notices.

## Decision

In phase 1 Voon only reads the library. It lists folders, reads CUE sheets
and metadata tags, and keeps everything it learns, including the user's
edits, user tags and manual collections, in its own SQLite index on the Mac.
Files and folders on the NAS are never created, renamed, moved, deleted or
retagged.

Write operations (rename, move, tag writing, CUE splitting, junk cleanup)
come later. When they do, every one will show a preview of the changes,
require confirmation, record an undo log, and be off by default.

## Alternatives

- **Write tags and rename from day one.** Most useful immediately, but every
  scanner bug becomes data loss.
- **Write only to sidecar files on the NAS.** Still touches the share, leaves
  files the user did not ask for, and needs the NAS mounted to edit.
