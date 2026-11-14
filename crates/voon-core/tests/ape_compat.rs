//! Synthetic MAC headers and the diagnostic report's legacy Album bytes exercise
//! the real Lofty path. No audio decoder or NAS files are needed by these tests.
use std::{
    fs,
    path::{Path, PathBuf},
};
use voon_core::{
    AudioFormat,
    tags::{self, TagReadResult},
};

const ALBUM: &str =
    "Symphonies Nos. 4, 5 & 6 Pathétique - Igor Markevitch, London SO (1962-66 Philips) (Disc 1)";
// A small PNG payload; compatibility must leave binary items byte-for-byte intact.
const COVER: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x02\0\0\0\x90wS\xde";

fn item(key: &str, value: &[u8], flags: u32) -> Vec<u8> {
    let mut bytes = (value.len() as u32).to_le_bytes().to_vec();
    bytes.extend(flags.to_le_bytes());
    bytes.extend(key.as_bytes());
    bytes.push(0);
    bytes.extend(value);
    bytes
}

fn audio(items: &[Vec<u8>], header: bool, id3: bool) -> Vec<u8> {
    let mut bytes = vec![0; 512];
    bytes[..4].copy_from_slice(b"MAC ");
    bytes[4..6].copy_from_slice(&3990u16.to_le_bytes());
    bytes[8..12].copy_from_slice(&52u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&24u32.to_le_bytes());
    bytes[52..54].copy_from_slice(&2000u16.to_le_bytes());
    bytes[56..60].copy_from_slice(&132300u32.to_le_bytes());
    bytes[60..64].copy_from_slice(&132300u32.to_le_bytes());
    bytes[64..68].copy_from_slice(&1u32.to_le_bytes());
    bytes[68..70].copy_from_slice(&16u16.to_le_bytes());
    bytes[70..72].copy_from_slice(&2u16.to_le_bytes());
    bytes[72..76].copy_from_slice(&44100u32.to_le_bytes());
    let mut footer = b"APETAGEX".to_vec();
    footer.extend(2000u32.to_le_bytes());
    footer.extend((32 + items.iter().map(Vec::len).sum::<usize>() as u32).to_le_bytes());
    footer.extend((items.len() as u32).to_le_bytes());
    let flags = 0x40000000u32 | if header { 0x80000000 } else { 0 };
    footer.extend(flags.to_le_bytes());
    footer.extend([0; 8]);
    if header {
        let mut header = footer.clone();
        header[20..24].copy_from_slice(&(flags | 0x20000000).to_le_bytes());
        bytes.extend(header);
    }
    for item in items {
        bytes.extend(item);
    }
    bytes.extend(footer);
    if id3 {
        bytes.extend(b"TAG");
        bytes.extend([0; 125]);
    }
    bytes
}

fn western_album() -> Vec<u8> {
    let (bytes, _, errors) = encoding_rs::WINDOWS_1252.encode(ALBUM);
    assert!(!errors);
    let bytes = bytes.into_owned();
    assert_eq!(std::str::from_utf8(&bytes).unwrap_err().valid_up_to(), 29);
    bytes
}

fn fixture(root: &Path, extra: Vec<Vec<u8>>, header: bool, id3: bool) -> PathBuf {
    fs::create_dir_all(root).unwrap();
    let path = root.join("CDImage.ape");
    let mut items = vec![
        item("Year", b"1993", 0),
        item("Genre", b"Classical", 0),
        item("Comment", b"Exact Audio Copy", 0),
        item("Title", b"C:\\Archive\\CDImage", 0),
        item("Album", &western_album(), 0),
    ];
    items.extend(extra);
    fs::write(&path, audio(&items, header, id3)).unwrap();
    path
}

fn cue(root: &Path, filename: &str, target: &str, album: &str) -> PathBuf {
    let text = format!(
        "PERFORMER \"Tchaikovsky\"\nTITLE \"{album}\"\nFILE \"{target}\" WAVE\n TRACK 01 AUDIO\n TITLE \"First movement\"\n INDEX 01 00:00:00\n TRACK 02 AUDIO\n TITLE \"Second movement\"\n INDEX 01 00:01:00\n"
    );
    let (bytes, _, errors) = encoding_rs::GBK.encode(&text);
    assert!(!errors);
    let path = root.join(filename);
    fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn targeted_retry_uses_legacy_ape_and_gbk_cue_without_a_library_walk() {
    use std::sync::atomic::AtomicBool;
    use voon_core::{
        db::{CachedTags, Index},
        scan::{DiskTags, FileStamp, NoTags, retry_tag_paths, scan_root},
    };
    let library = tempfile::tempdir().unwrap();
    let root = library.path();
    let album = root.join("Legacy");
    let path = fixture(&album, vec![], false, false);
    let sheet = cue(&album, "CDImage.cue", "CDImage.wav", ALBUM);
    let audio_before = fs::read(&path).unwrap();
    let cue_before = fs::read(&sheet).unwrap();
    let stamps = (FileStamp::of(&path), FileStamp::of(&sheet));
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(root, &scan_root(root, &mut NoTags))
        .unwrap();
    let mut reader = CachedTags::new(&index, DiskTags);
    let scan = retry_tag_paths(
        root,
        std::slice::from_ref(&path),
        &mut reader,
        &AtomicBool::new(false),
    );
    assert_eq!(reader.reads, 1);
    assert!(scan.coverage.listed_directories.is_empty());
    assert_eq!(
        scan.tag_observations[&path]
            .info()
            .unwrap()
            .album
            .as_deref(),
        Some(ALBUM)
    );
    index.store_tag_retry(root, &scan).unwrap();
    let saved = index.album(&album).unwrap().unwrap();
    assert_eq!(saved.tracks().count(), 2);
    assert_eq!(saved.metadata.title.unwrap().value, ALBUM);
    assert_eq!(fs::read(&path).unwrap(), audio_before);
    assert_eq!(fs::read(&sheet).unwrap(), cue_before);
    assert_eq!((FileStamp::of(&path), FileStamp::of(&sheet)), stamps);
}

fn must_fail(path: &Path) {
    assert!(matches!(
        tags::observe(path, AudioFormat::Ape).result,
        TagReadResult::ParseFailure(_)
    ));
}

fn v1_audio(title: &[u8], album: &str) -> Vec<u8> {
    let mut bytes = audio(
        &[
            item("Album", format!("{album}\0").as_bytes(), 0),
            item("Title", title, 0),
            item("Year", b"1997\0", 0),
            item("Genre", b"Classical\0", 0),
        ],
        false,
        false,
    );
    let end = bytes.len();
    bytes[end - 24..end - 20].copy_from_slice(&1000u32.to_le_bytes());
    bytes[end - 12..end - 8].fill(0);
    bytes
}

#[test]
fn v1_legacy_path_titles_use_matching_cues_without_changing_either_source() {
    let temp = tempfile::tempdir().unwrap();
    let (legacy, _, _) =
        encoding_rs::GBK.encode("C:\\Documents and Settings\\用户\\桌面\\disc1\\CDImage\0");
    for disc in ["A", "B"] {
        let name = format!("[莫扎特五重奏全集].{disc}");
        let album = format!("Quintets CD {disc}");
        let ape = temp.path().join(format!("{name}.ape"));
        fs::write(&ape, v1_audio(&legacy, &album)).unwrap();
        cue(temp.path(), &format!("{name}.cue"), "CDImage.ape", &album);
    }
    for disc in ["A", "B"] {
        let ape = temp.path().join(format!("[莫扎特五重奏全集].{disc}.ape"));
        let sheet = temp.path().join(format!("[莫扎特五重奏全集].{disc}.cue"));
        let before = (fs::read(&ape).unwrap(), fs::read(&sheet).unwrap());
        let stamp = (
            voon_core::scan::FileStamp::of(&ape),
            voon_core::scan::FileStamp::of(&sheet),
        );
        let info = tags::read(&ape, AudioFormat::Ape).unwrap();
        assert_eq!(
            info.album.as_deref(),
            Some(format!("Quintets CD {disc}").as_str())
        );
        assert_eq!(info.year, Some(1997));
        assert_eq!(info.genre.as_deref(), Some("Classical"));
        assert!(
            info.title.is_none(),
            "a ripping path must not become a track title"
        );
        assert_eq!((fs::read(&ape).unwrap(), fs::read(&sheet).unwrap()), before);
        assert_eq!(
            (
                voon_core::scan::FileStamp::of(&ape),
                voon_core::scan::FileStamp::of(&sheet)
            ),
            stamp
        );
    }
}

#[test]
fn v1_recovery_requires_an_image_path_and_a_matching_nonempty_cue() {
    let temp = tempfile::tempdir().unwrap();
    let ape = temp.path().join("CDImage.ape");
    let (title, _, _) = encoding_rs::GBK.encode("C:\\新建文件夹\\CDImage\0");
    fs::write(&ape, v1_audio(&title, "Album")).unwrap();
    must_fail(&ape);
    let sheet = cue(temp.path(), "other.cue", "other.ape", "Album");
    must_fail(&ape);
    fs::remove_file(sheet).unwrap();
    cue(temp.path(), "CDImage.cue", "CDImage.ape", "Album");
    let (not_path, _, _) = encoding_rs::GBK.encode("真正的中文曲名\0");
    fs::write(&ape, v1_audio(&not_path, "Album")).unwrap();
    must_fail(&ape);
    let (unrelated, _, _) = encoding_rs::GBK.encode("C:\\新建文件夹\\other\0");
    fs::write(&ape, v1_audio(&unrelated, "Album")).unwrap();
    must_fail(&ape);
}

#[test]
fn report_encoding_is_recovered_without_writing_sources_and_keeps_properties() {
    for (header, id3) in [(false, false), (true, false), (false, true), (true, true)] {
        let temp = tempfile::tempdir().unwrap();
        let mut picture = b"front.png\0".to_vec();
        picture.extend(COVER);
        let ape = fixture(
            temp.path(),
            vec![item("Cover Art (Front)", &picture, 2)],
            header,
            id3,
        );
        let cue_path = cue(temp.path(), "CDImage.CUE", "CDImage.ape", ALBUM);
        assert!(
            lofty::read_from_path(&ape).is_err(),
            "strict parser must reproduce the original failure"
        );
        let before: Vec<_> = [&ape, &cue_path]
            .into_iter()
            .map(|p| {
                let bytes = fs::read(p).unwrap();
                let modified = fs::metadata(p).unwrap().modified().unwrap();
                let mut permissions = fs::metadata(p).unwrap().permissions();
                permissions.set_readonly(true);
                fs::set_permissions(p, permissions).unwrap();
                (bytes, modified)
            })
            .collect();
        let info = tags::read(&ape, AudioFormat::Ape).unwrap();
        assert_eq!(info.album.as_deref(), Some(ALBUM));
        assert_eq!(info.year, Some(1993));
        assert_eq!(info.genre.as_deref(), Some("Classical"));
        assert_eq!(info.title.as_deref(), Some("C:\\Archive\\CDImage"));
        assert_eq!(info.properties.sample_rate, Some(44100));
        assert_eq!(info.properties.bit_depth, Some(16));
        assert_eq!(info.properties.channels, Some(2));
        assert_eq!(info.properties.duration_ms, Some(3000));
        for (path, (bytes, modified)) in [&ape, &cue_path].into_iter().zip(before) {
            assert_eq!(fs::read(path).unwrap(), bytes);
            assert_eq!(fs::metadata(path).unwrap().modified().unwrap(), modified);
        }
    }
}

#[test]
fn missing_unrelated_or_conflicting_cues_never_turn_guesses_into_success() {
    let temp = tempfile::tempdir().unwrap();
    let ape = fixture(temp.path(), vec![], false, false);
    must_fail(&ape);
    let cue_path = cue(temp.path(), "other.cue", "Other.ape", ALBUM);
    must_fail(&ape);
    fs::remove_file(cue_path).unwrap();
    cue(temp.path(), "matching.cue", "CDImage.ape", ALBUM);
    cue(
        temp.path(),
        "conflicting.cue",
        "CDImage.ape",
        "Different title",
    );
    must_fail(&ape);
}

#[test]
fn cue_can_reference_precompression_wav_only_if_the_target_is_unambiguous() {
    let temp = tempfile::tempdir().unwrap();
    let ape = fixture(temp.path(), vec![], false, false);
    cue(temp.path(), "CDImage.cue", "C:\\Old\\CDImage.wav", ALBUM);
    assert_eq!(
        tags::read(&ape, AudioFormat::Ape).unwrap().album.as_deref(),
        Some(ALBUM)
    );
    fs::write(temp.path().join("CDImage.wav"), b"different audio").unwrap();
    must_fail(&ape);
}

#[test]
fn valid_utf8_uses_the_normal_path_even_without_cue_and_gbk_requires_matching_evidence() {
    let temp = tempfile::tempdir().unwrap();
    let ape = temp.path().join("CDImage.ape");
    fs::write(
        &ape,
        audio(&[item("Album", ALBUM.as_bytes(), 0)], false, false),
    )
    .unwrap();
    assert_eq!(
        tags::read(&ape, AudioFormat::Ape).unwrap().album.as_deref(),
        Some(ALBUM)
    );
    let title = "柴可夫斯基交响曲";
    let (bytes, _, _) = encoding_rs::GBK.encode(title);
    fs::write(&ape, audio(&[item("Album", &bytes, 0)], false, false)).unwrap();
    must_fail(&ape);
    cue(temp.path(), "CDImage.cue", "CDImage.ape", title);
    assert_eq!(
        tags::read(&ape, AudioFormat::Ape).unwrap().album.as_deref(),
        Some(title)
    );
}

#[test]
fn unmapped_bad_fields_and_malformed_items_are_not_silently_discarded() {
    let temp = tempfile::tempdir().unwrap();
    cue(temp.path(), "CDImage.cue", "CDImage.ape", ALBUM);
    let ape = fixture(temp.path(), vec![item("Unknown", b"\xe9", 0)], false, false);
    must_fail(&ape);
    let ape = fixture(
        temp.path(),
        vec![item("Album", &western_album(), 0)],
        false,
        false,
    );
    must_fail(&ape);
    let ape = fixture(temp.path(), vec![], true, false);
    let mut bytes = fs::read(&ape).unwrap();
    // A header/footer disagreement must not be "repaired" along with text.
    bytes[512 + 16..512 + 20].copy_from_slice(&999u32.to_le_bytes());
    fs::write(&ape, bytes).unwrap();
    must_fail(&ape);
}

#[test]
fn cached_failure_is_retried_and_cleared_by_a_normal_scan_without_changing_cue_tracks() {
    use voon_core::{
        db::{CachedTags, Index},
        scan::{DiskTags, FileStamp, TagReader, scan_root},
        tags::{TagInfo, TagObservation},
    };
    struct OldReader;
    impl TagReader for OldReader {
        fn read(&mut self, _: &Path, _: AudioFormat, _: FileStamp) -> Option<TagInfo> {
            None
        }
        fn read_observation(&mut self, _: &Path, _: AudioFormat, _: FileStamp) -> TagObservation {
            TagObservation::new(TagReadResult::ParseFailure("invalid utf-8 sequence".into()))
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let album = temp.path().join("Album");
    let ape = fixture(&album, vec![], false, false);
    cue(&album, "CDImage.cue", "CDImage.ape", ALBUM);
    let stamp = FileStamp::of(&ape).unwrap();
    let mut index = Index::open_in_memory().unwrap();
    let old = scan_root(temp.path(), &mut CachedTags::new(&index, OldReader));
    index.store_scan(temp.path(), &old).unwrap();
    assert!(
        index
            .attention_items()
            .unwrap()
            .iter()
            .any(|item| item.kind == "TagParseFailure")
    );
    let mut reader = CachedTags::new(&index, DiskTags);
    let scan = scan_root(temp.path(), &mut reader);
    assert_eq!(reader.reads, 1);
    index.store_scan(temp.path(), &scan).unwrap();
    assert_eq!(FileStamp::of(&ape).unwrap(), stamp);
    assert!(
        !index
            .attention_items()
            .unwrap()
            .iter()
            .any(|item| item.kind == "TagParseFailure")
    );
    let restored = index.album(&album).unwrap().unwrap();
    assert_eq!(restored.metadata.title.unwrap().value, ALBUM);
    let tracks = &restored.discs[0].tracks;
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0].title.as_ref().unwrap().value, "First movement");
    assert_eq!(tracks[1].properties.duration_ms, Some(2000));
    let mut reader = CachedTags::new(&index, DiskTags);
    scan_root(temp.path(), &mut reader);
    assert_eq!(reader.reads, 0);
}
