//! Scanner and index tests on a fake library that mirrors the real NAS layout.
//! Audio files are empty placeholders: structure, CUE and folder names are
//! what is under test, not tag decoding.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use voon_core::db::{CachedTags, Index};
use voon_core::scan::{self, DiskTags, IssueKind, NoTags, ScanEvent};
use voon_core::*;

fn touch(root: &Path, rel: &str) -> PathBuf {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"x").unwrap();
    path
}

fn write(root: &Path, rel: &str, bytes: &[u8]) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

const DENG: &str = "邓丽君 - 淡淡幽情 (1983) [台湾首版]";

fn library() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path();

    // Junk that must never show up.
    touch(r, ".DS_Store");
    touch(r, "@eaDir/SYNOFILE_THUMB.jpg");
    touch(r, "#recycle/old/01.flac");

    // Whole-disc image + GBK CUE that names a .wav that was later compressed to .ape.
    let cue = "PERFORMER \"邓丽君\"\r\nTITLE \"淡淡幽情\"\r\nREM DATE 1983\r\nREM GENRE Pop\r\nFILE \"CDImage.wav\" WAVE\r\n  TRACK 01 AUDIO\r\n    TITLE \"独上西楼\"\r\n    INDEX 01 00:00:00\r\n  TRACK 02 AUDIO\r\n    TITLE \"但愿人长久\"\r\n    INDEX 01 03:10:00\r\n  TRACK 03 AUDIO\r\n    TITLE \"几多愁\"\r\n    INDEX 01 07:00:30\r\n";
    let (gbk, _, _) = encoding_rs::GBK.encode(cue);
    write(r, &format!("{DENG}/CDImage.cue"), &gbk);
    touch(r, &format!("{DENG}/CDImage.ape"));
    touch(r, &format!("{DENG}/._CDImage.ape"));
    touch(r, &format!("{DENG}/Cover.jpg"));
    touch(r, &format!("{DENG}/back.jpg"));
    touch(r, &format!("{DENG}/EAC.log"));
    touch(r, &format!("{DENG}/广告.url"));

    // A box set series folder: a Collection of multi-disc albums.
    let duo = "Philips Duo/[438317-2] Brendel - Schubert Impromptus";
    touch(r, &format!("{duo}/CD1/01.flac"));
    touch(r, &format!("{duo}/CD1/02.flac"));
    touch(r, &format!("{duo}/CD2/01.flac"));
    touch(r, &format!("{duo}/Booklet/front.jpg"));
    touch(r, &format!("{duo}/Booklet/page01.jpg"));
    touch(r, "Philips Duo/Bach Partitas [24-96]/0015830558.flac");
    touch(r, "Philips Duo/Bach Partitas [24-96]/0015830559.flac");
    touch(r, "Philips Duo/Bach Partitas [24-96]/folder.jpg");

    // A performer folder holding albums, with `Disc N` / `CD 03` disc folders.
    touch(r, "Alfred_Brendel/Mozart Concertos (1990)/Disc 1/01.wv");
    touch(r, "Alfred_Brendel/Mozart Concertos (1990)/Disc 2/01.wv");
    touch(r, "Alfred_Brendel/Mozart Concertos (1990)/CD 03/01.wv");

    // DSD.
    touch(r, "Kind of Blue [DSD64]/01.dsf");
    touch(r, "Kind of Blue [DSD64]/02.dsf");

    // Two CUE + image pairs in one folder: two discs.
    let pair = |n: u32| {
        format!(
            "TITLE \"Pair\"\nFILE \"CD{n}.flac\" WAVE\n  TRACK 01 AUDIO\n    INDEX 01 00:00:00\n  TRACK 02 AUDIO\n    INDEX 01 05:00:00\n"
        )
    };
    write(r, "Pair/CD1.cue", pair(1).as_bytes());
    write(r, "Pair/CD2.cue", pair(2).as_bytes());
    touch(r, "Pair/CD1.flac");
    touch(r, "Pair/CD2.flac");

    // One FILE per track: real tracks, CUE titles.
    write(
        r,
        "Split/album.cue",
        b"TITLE \"Split Album\"\nFILE \"01 A.flac\" WAVE\n  TRACK 01 AUDIO\n    TITLE \"Song A\"\n    INDEX 01 00:00:00\nFILE \"02 B.flac\" WAVE\n  TRACK 02 AUDIO\n    TITLE \"Song B\"\n    INDEX 01 00:00:00\n",
    );
    touch(r, "Split/01 A.flac");
    touch(r, "Split/02 B.flac");

    // A CUE whose image is missing.
    write(
        r,
        "Broken/x.cue",
        b"FILE \"missing.wav\" WAVE\n  TRACK 01 AUDIO\n    INDEX 01 00:00:00\n",
    );
    touch(r, "Broken/other.flac");

    // Only a music video: not an album.
    touch(r, "Videos/clip.m4v");
    dir
}

fn album<'a>(scan: &'a ScanResult, name: &str) -> &'a Album {
    scan.albums
        .iter()
        .find(|a| a.folder_name == name)
        .unwrap_or_else(|| panic!("no album {name}"))
}

#[test]
fn classifies_albums_discs_and_collections() {
    let lib = library();
    let scan = scan_root(lib.path(), &mut NoTags);

    let mut names: Vec<_> = scan.albums.iter().map(|a| a.folder_name.as_str()).collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "Bach Partitas [24-96]",
            "Broken",
            "Kind of Blue [DSD64]",
            "Mozart Concertos (1990)",
            "Pair",
            "Split",
            "[438317-2] Brendel - Schubert Impromptus",
            DENG,
        ]
    );

    let mut collections: Vec<_> = scan.collections.iter().map(|c| c.name.as_str()).collect();
    collections.sort();
    assert_eq!(collections, vec!["Alfred_Brendel", "Philips Duo"]);
    let duo = scan
        .collections
        .iter()
        .find(|c| c.name == "Philips Duo")
        .unwrap();
    assert_eq!(duo.albums.len(), 2);

    let schubert = album(&scan, "[438317-2] Brendel - Schubert Impromptus");
    assert_eq!(schubert.discs.len(), 2);
    assert_eq!(schubert.discs[0].number, 1);
    assert_eq!(schubert.discs[0].tracks.len(), 2);
    assert_eq!(schubert.discs[1].tracks.len(), 1);
    assert_eq!(schubert.collection.as_deref(), Some(duo.path.as_path()));
    // Cover found in the Booklet folder by name.
    assert!(matches!(
        &schubert.cover,
        Some(Cover { source: CoverSource::File(p) }) if p.ends_with("Booklet/front.jpg")
    ));

    let mozart = album(&scan, "Mozart Concertos (1990)");
    assert_eq!(
        mozart.discs.iter().map(|d| d.number).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(
        mozart.metadata.year,
        Some(Field::new(1990, FieldSource::FolderName))
    );
}

#[test]
fn whole_disc_image_becomes_virtual_tracks() {
    let lib = library();
    let scan = scan_root(lib.path(), &mut NoTags);
    let deng = album(&scan, DENG);
    assert_eq!(deng.discs.len(), 1);
    let tracks = &deng.discs[0].tracks;
    assert_eq!(tracks.len(), 3);
    assert_eq!(
        tracks[1].title,
        Some(Field::new("但愿人长久".to_string(), FieldSource::Cue))
    );
    match &tracks[1].source {
        TrackSource::Virtual {
            image, start, end, ..
        } => {
            assert!(image.ends_with("CDImage.ape"));
            assert_eq!(*start, (3 * 60 + 10) * 75);
            assert_eq!(*end, Some((7 * 60) * 75 + 30));
        }
        other => panic!("expected a virtual track, got {other:?}"),
    }
    assert!(matches!(
        tracks[2].source,
        TrackSource::Virtual { end: None, .. }
    ));
    assert!(tracks.iter().all(|t| t.format == AudioFormat::Ape));

    // CUE beats folder name; folder name still supplies the edition.
    let m = &deng.metadata;
    assert_eq!(
        m.title,
        Some(Field::new("淡淡幽情".into(), FieldSource::Cue))
    );
    assert_eq!(
        m.album_artist,
        Some(Field::new("邓丽君".into(), FieldSource::Cue))
    );
    assert_eq!(m.year, Some(Field::new(1983, FieldSource::Cue)));
    assert_eq!(
        m.edition,
        Some(Field::new("台湾首版".into(), FieldSource::FolderName))
    );
    // Genre comes from tags only, never from the CUE.
    assert_eq!(m.genre, None);
    // Named cover wins over other images.
    assert!(matches!(
        &deng.cover,
        Some(Cover { source: CoverSource::File(p) }) if p.ends_with("Cover.jpg")
    ));
}

#[test]
fn cue_pairs_split_files_and_broken_sheets() {
    let lib = library();
    let scan = scan_root(lib.path(), &mut NoTags);

    let pair = album(&scan, "Pair");
    assert_eq!(pair.discs.len(), 2);
    assert!(pair.discs.iter().all(|d| d.tracks.len() == 2));

    let split = album(&scan, "Split");
    assert_eq!(split.discs.len(), 1);
    let titles: Vec<_> = split.discs[0]
        .tracks
        .iter()
        .map(|t| t.title.as_ref().unwrap().value.as_str())
        .collect();
    assert_eq!(titles, vec!["Song A", "Song B"]);
    assert!(
        split.discs[0]
            .tracks
            .iter()
            .all(|t| matches!(t.source, TrackSource::File { .. }))
    );

    let broken = album(&scan, "Broken");
    assert_eq!(broken.tracks().count(), 1);
    assert!(scan.issues.iter().any(|i| matches!(
        &i.kind,
        IssueKind::CueTargetMissing { file, .. } if file == "missing.wav"
    )));
    assert!(
        scan.issues
            .iter()
            .any(|i| i.path == broken.path && i.kind == IssueKind::TitleFromFolderOnly)
    );
}

#[test]
fn junk_and_non_music_are_ignored() {
    let lib = library();
    let scan = scan_root(lib.path(), &mut NoTags);
    let all: Vec<_> = scan
        .albums
        .iter()
        .flat_map(|a| a.tracks())
        .map(|t| t.source.audio_path().to_string_lossy().into_owned())
        .collect();
    assert!(
        all.iter()
            .all(|p| !p.contains("#recycle") && !p.contains("/._"))
    );
    assert!(scan.albums.iter().all(|a| a.folder_name != "Videos"));
}

#[test]
fn facets_from_structure() {
    let lib = library();
    let scan = scan_root(lib.path(), &mut NoTags);
    let kob = facets::facets(album(&scan, "Kind of Blue [DSD64]"));
    assert_eq!(kob.encoding, facets::Encoding::Dsd);
    assert_eq!(kob.resolution, facets::Resolution::Dsd(64));
    let bach = facets::facets(album(&scan, "Bach Partitas [24-96]"));
    assert_eq!(bach.resolution, facets::Resolution::HiRes);
    assert_eq!(bach.max_sample_rate, Some(96_000));
    let deng = facets::facets(album(&scan, DENG));
    assert_eq!(deng.language, facets::Language::Chinese);
    assert_eq!(deng.decade, Some(1980));
}

#[test]
fn progress_and_cancel() {
    let lib = library();
    let mut seen = 0;
    let full = scan::scan_root_with(lib.path(), &mut NoTags, &AtomicBool::new(false), &mut |e| {
        if let ScanEvent::Album(_) = e {
            seen += 1;
        }
    });
    assert_eq!(seen, full.albums.len());
    assert!(!full.cancelled);

    let cancel = AtomicBool::new(false);
    let partial = scan::scan_root_with(lib.path(), &mut NoTags, &cancel, &mut |e| {
        if let ScanEvent::Album(_) = e {
            cancel.store(true, Ordering::Relaxed);
        }
    });
    assert!(partial.cancelled);
    assert!(partial.albums.len() < full.albums.len());
}

#[test]
fn index_round_trip_user_edits_and_incremental_tags() {
    let lib = library();
    let mut index = Index::open_in_memory().unwrap();
    assert_eq!(
        index.schema_version().unwrap(),
        voon_core::db::SCHEMA_VERSION
    );

    let mut reader = CachedTags::new(&index, DiskTags);
    let scan = scan_root(lib.path(), &mut reader);
    let first_reads = reader.reads;
    assert!(first_reads > 0);
    index.store_scan(lib.path(), &scan).unwrap();

    let albums = index.albums().unwrap();
    assert_eq!(albums.len(), scan.albums.len());
    let deng = albums.iter().find(|a| a.folder_name == DENG).unwrap();
    assert_eq!(deng.display_title(), "淡淡幽情");
    assert_eq!(deng.track_count, 3);
    assert_eq!(index.collections().unwrap().len(), 2);
    assert!(!index.issues().unwrap().is_empty());

    // A user edit beats the CUE and survives a rescan.
    index
        .set_user_edit(&deng.path, "title", "淡淡幽情（修订）")
        .unwrap();
    index.add_user_tag(&deng.path, "华语经典").unwrap();
    let mix = index.create_manual_collection("周末").unwrap();
    index.add_to_manual_collection(mix, &deng.path).unwrap();

    // Nothing changed on disk: no file is read again.
    let mut reader = CachedTags::new(&index, DiskTags);
    let rescan = scan_root(lib.path(), &mut reader);
    assert_eq!(reader.reads, 0);
    index.store_scan(lib.path(), &rescan).unwrap();

    let albums = index.albums().unwrap();
    let deng = albums.iter().find(|a| a.folder_name == DENG).unwrap();
    assert_eq!(
        deng.metadata.title,
        Some(Field::new("淡淡幽情（修订）".into(), FieldSource::UserEdit))
    );
    assert_eq!(index.user_tags(&deng.path).unwrap(), vec!["华语经典"]);
    assert_eq!(
        index.manual_collection_albums(mix).unwrap(),
        vec![deng.path.clone()]
    );

    index.clear_user_edit(&deng.path, "title").unwrap();
    let albums = index.albums().unwrap();
    let deng = albums.iter().find(|a| a.folder_name == DENG).unwrap();
    assert_eq!(deng.display_title(), "淡淡幽情");
}

#[test]
fn index_on_disk_reopens() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("voon.sqlite3");
    Index::open(&db)
        .unwrap()
        .add_root(Path::new("/Volumes/NAS1/Music"))
        .unwrap();
    let index = Index::open(&db).unwrap();
    assert_eq!(
        index.schema_version().unwrap(),
        voon_core::db::SCHEMA_VERSION
    );
    assert_eq!(index.roots().unwrap().len(), 1);
}
