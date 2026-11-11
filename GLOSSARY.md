# Glossary

The terms Voon uses for a music library. Code, UI copy and discussions use
these words with these meanings.

**Library root** — A folder the user adds as the top of a music library, such
as an SMB share mounted at `/Volumes/NAS1/Music`. A user can have several.

**Collection** — A folder that only groups albums, such as a box-set series
(`Philips Duo/`) or a performer folder (`Alfred_Brendel/`). A Collection is
not itself an Album. Collections can contain Collections.

**Manual collection** — A list of albums the user puts together by hand,
independent of where the albums sit on disk.

**Album** — One release: the unit the user browses and edits. An Album has
one or more Discs.

**Disc** — One physical or logical disc of an Album. A subfolder such as
`CD1`, `CD 02` or `Disc 2` is a Disc of the Album above it, not an Album of
its own.

**Track** — One piece of music on a Disc. A Track is either a whole audio
file or a Virtual track.

**Whole-disc image** (image file) — A single audio file holding an entire
Disc, such as `CDImage.ape`, whose track boundaries are given by a CUE sheet.

**CUE sheet** — A text file (`.cue`) that lists the tracks of a Whole-disc
image (or of a set of files) with titles, performers and start positions.

**Virtual track** — A Track defined as a span of a Whole-disc image, from one
CUE start position to the next. Voon shows Virtual tracks without cutting
the image into files.

**Cover** — The picture that represents an Album: an image file next to the
music (such as `cover.jpg`, `folder.jpg`, `front.jpg`) or a picture embedded
in an audio file's metadata tags.

**Metadata tag** — A field stored inside an audio file by its format (Vorbis
comment, APEv2, ID3 and so on), such as title, artist or composer.

**User tag** — A label the user attaches to Albums inside Voon, such as
"华语经典". Not written into files.

**Facet** — A property Voon derives automatically for filtering: format,
sample rate and bit depth, DSD or PCM, language, decade.

**Field source** (provenance) — Where an Album field's value came from: a
user edit, a Metadata tag, a CUE sheet, or the folder name. When sources
disagree the stronger one wins, in that order; a user edit always wins.

**Loose audio** — Audio files directly in a Library root rather than in an
album folder. Voon groups them into Albums by their album and album artist
Metadata tags; files without an album tag need attention.

**Content signature** — A fingerprint of an Album's audio files (their paths
within the album folder and their sizes) that does not change when the album
folder is renamed or moved.

**Rename candidate** — A pairing, proposed after a rescan, of an album path
that disappeared with a new album path that has the same Content signature.
When the user confirms it, their edits, User tags and Manual collection
membership move to the new path.

**Needs attention** (待处理) — An Album Voon could not resolve with
confidence, listed with the reason, for the user to fix.
