# ADR 0002: Whole-disc images become virtual tracks

Status: accepted

## Context

Hundreds of albums in the library (about 527 folders) are one Whole-disc
image such as `CDImage.ape` plus a CUE sheet. CUE sheets come in UTF-8,
UTF-16, GBK and Shift-JIS, often name a `.wav` that was later compressed to
`.ape` or `.flac`, and sometimes appear twice for the same image.

## Decision

Voon parses the CUE sheet and shows each entry as a Virtual track: a span of
the image from one start position to the next, the last one running to the
end. The image file is never cut. When a sheet lists one file per track and
those files exist, the files are ordinary Tracks and the sheet only supplies
titles. A sheet whose image cannot be found is reported in the Needs
attention view.

## Alternatives

- **Split images into per-track files.** Gives every player a normal album,
  but writes to the library (see ADR 0001) and doubles storage until the
  originals are removed.
- **Treat the image as a single long track.** Simple, but the track list,
  titles and per-track performers in the sheet are lost.
