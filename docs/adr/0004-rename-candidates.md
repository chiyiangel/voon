# ADR 0004: Detect renamed folders by content, confirm by hand

Status: accepted

## Context

User edits, user tags and manual collections are keyed by album path, so
they survive rescans (album rows are rebuilt on every scan). Renaming or
moving an album folder on the NAS would silently orphan them. Voon never
writes to the library (ADR 0001), so it cannot leave an ID file in the folder
to follow it.

## Decision

Each album stores a content signature: a hash of its audio files' paths
relative to the album folder plus their sizes. It is cheap (directory
listing and file sizes the scan already has, no audio reads) and does not
change when only the album folder's name or location changes.

When a rescan of a library root finds an album path gone and exactly one new
path with the same signature, Voon records a rename candidate and lists it in
待处理. Nothing moves until the user confirms; confirming moves user edits,
user tags and manual collection membership to the new path in one
transaction. Ambiguous matches are not proposed. A folder renamed again
before the user decides keeps one candidate from the first path to the
latest one.

## Alternatives

- **Migrate automatically.** Faster, but a wrong pairing (two identical rips)
  would move the user's work without asking.
- **Key user data by signature instead of path.** Survives renames without a
  prompt, but any re-encode, retag that changes size, or added track breaks
  the link, and identical copies would share edits.
- **Hash audio content.** More precise, but reads gigabytes over SMB on every
  scan.
