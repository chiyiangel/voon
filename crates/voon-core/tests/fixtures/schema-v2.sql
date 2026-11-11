-- Legacy schema captured from f59e55a; intentionally fixed for upgrade coverage.

CREATE TABLE library_root (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    last_scan INTEGER
);
CREATE TABLE collection (
    id INTEGER PRIMARY KEY,
    root_id INTEGER NOT NULL REFERENCES library_root(id) ON DELETE CASCADE,
    path TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    parent_path TEXT
);
CREATE TABLE album (
    id INTEGER PRIMARY KEY,
    root_id INTEGER NOT NULL REFERENCES library_root(id) ON DELETE CASCADE,
    path TEXT NOT NULL UNIQUE,
    folder_name TEXT NOT NULL,
    collection_path TEXT,
    cover_path TEXT,
    cover_embedded INTEGER NOT NULL DEFAULT 0
);
-- Every candidate value per field and source; the strongest source wins on read.
CREATE TABLE album_field (
    album_id INTEGER NOT NULL REFERENCES album(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    source TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (album_id, name, source)
);
CREATE TABLE disc (
    id INTEGER PRIMARY KEY,
    album_id INTEGER NOT NULL REFERENCES album(id) ON DELETE CASCADE,
    number INTEGER NOT NULL,
    path TEXT NOT NULL
);
CREATE TABLE track (
    id INTEGER PRIMARY KEY,
    disc_id INTEGER NOT NULL REFERENCES disc(id) ON DELETE CASCADE,
    number INTEGER,
    title TEXT,
    title_source TEXT,
    artist TEXT,
    artist_source TEXT,
    format TEXT NOT NULL,
    path TEXT NOT NULL,
    cue_path TEXT,
    start_frames INTEGER,
    end_frames INTEGER,
    sample_rate INTEGER,
    bit_depth INTEGER,
    channels INTEGER,
    duration_ms INTEGER
);
CREATE INDEX track_disc ON track(disc_id);
CREATE TABLE scan_issue (
    id INTEGER PRIMARY KEY,
    root_id INTEGER NOT NULL REFERENCES library_root(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    kind TEXT NOT NULL,
    detail TEXT
);
-- Tag cache for incremental rescans: re-read only when size or mtime changed.
CREATE TABLE file_cache (
    path TEXT PRIMARY KEY,
    size INTEGER NOT NULL,
    mtime INTEGER NOT NULL,
    readable INTEGER NOT NULL,
    title TEXT, artist TEXT, album TEXT, album_artist TEXT, composer TEXT,
    conductor TEXT, genre TEXT, year INTEGER, track INTEGER, disc INTEGER,
    catalog_number TEXT, has_picture INTEGER NOT NULL DEFAULT 0,
    sample_rate INTEGER, bit_depth INTEGER, channels INTEGER, duration_ms INTEGER
);
-- User-owned data, keyed by album path.
CREATE TABLE user_edit (
    album_path TEXT NOT NULL,
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (album_path, name)
);
CREATE TABLE user_tag (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);
CREATE TABLE album_user_tag (
    album_path TEXT NOT NULL,
    tag_id INTEGER NOT NULL REFERENCES user_tag(id) ON DELETE CASCADE,
    PRIMARY KEY (album_path, tag_id)
);
CREATE TABLE manual_collection (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);
CREATE TABLE manual_collection_album (
    collection_id INTEGER NOT NULL REFERENCES manual_collection(id) ON DELETE CASCADE,
    album_path TEXT NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY (collection_id, album_path)
);


ALTER TABLE album ADD COLUMN signature TEXT NOT NULL DEFAULT '';
CREATE INDEX album_signature ON album(signature);
-- An album path that vanished in a rescan while a new path with the same
-- content signature appeared. Kept across rescans until confirmed or
-- dismissed, or until the new path itself disappears.
CREATE TABLE rename_candidate (
    old_path TEXT NOT NULL,
    new_path TEXT NOT NULL,
    signature TEXT NOT NULL,
    PRIMARY KEY (old_path, new_path)
);

PRAGMA user_version = 2;
