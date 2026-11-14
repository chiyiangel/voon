use std::{fs, path::PathBuf};
use voon_core::{
    TrackSource,
    scan::{IssueKind, NoTags, scan_root},
};

#[test]
fn western_cue_matches_decomposed_audio_names_and_preserves_tracks() {
    let root = tempfile::tempdir().unwrap();
    let album = root.path().join("Dvorak");
    fs::create_dir(&album).unwrap();
    let audio = album.join("Dvora\u{301}k - Disk 1.ape");
    fs::write(&audio, b"fixture").unwrap();
    let text = "PERFORMER \"Dvorák\"\nTITLE \"Symphonies\"\nFILE \"Dvorák - Disk 1.ape\" WAVE\n TRACK 01 AUDIO\n TITLE \"First\"\n INDEX 01 00:00:00\n TRACK 02 AUDIO\n INDEX 01 05:00:00\n";
    let (bytes, _, errors) = encoding_rs::WINDOWS_1252.encode(text);
    assert!(!errors);
    let cue = album.join("unrelated-name.cue");
    fs::write(&cue, &bytes).unwrap();
    let result = scan_root(root.path(), &mut NoTags);
    assert!(
        !result
            .issues
            .iter()
            .any(|issue| matches!(issue.kind, IssueKind::CueTargetMissing { .. })),
        "{:?}",
        result.issues
    );
    assert_eq!(result.albums.len(), 1);
    let album = &result.albums[0];
    assert_eq!(
        album.metadata.album_artist.as_ref().unwrap().value,
        "Dvorák"
    );
    assert_eq!(album.tracks().count(), 2);
    assert!(
        album
            .tracks()
            .all(|track| track.source.audio_path() == &audio
                && matches!(track.source, TrackSource::Virtual { .. }))
    );
    assert_eq!(fs::read(&cue).unwrap(), bytes.as_ref());
}

#[test]
fn cue_resolution_rejects_ambiguous_normalized_names() {
    // Case variants are possible on some filesystems. Exercise resolution in
    // memory instead of depending on the host's normalization behavior.
    let audio = vec![
        PathBuf::from("/music/DVORÁK.ape"),
        PathBuf::from("/music/Dvorák.ape"),
    ];
    assert!(
        voon_core::cue::resolve_audio(
            std::path::Path::new("/music/other.cue"),
            "dvorák.ape",
            &audio,
            true
        )
        .is_none()
    );
}

#[test]
fn encoding_fallback_does_not_override_utf8_or_choose_between_conflicting_targets() {
    let text =
        "PERFORMER \"Dvorák\"\nFILE \"Dvorák.ape\" WAVE\n TRACK 01 AUDIO\n INDEX 01 00:00:00\n";
    let audio = vec![
        PathBuf::from("/music/Dvorák.ape"),
        PathBuf::from("/music/Dvor醟.ape"),
    ];
    let utf8 = voon_core::cue::parse_bytes_with_audio(text.as_bytes(), &audio);
    assert_eq!(utf8.encoding, "UTF-8");
    assert_eq!(utf8.performer.as_deref(), Some("Dvorák"));
    let (bytes, _, _) = encoding_rs::WINDOWS_1252.encode(text);
    let original = voon_core::cue::parse_bytes(&bytes);
    let detected = voon_core::cue::parse_bytes_with_audio(&bytes, &audio);
    assert_eq!(
        detected, original,
        "existing name evidence must not be overruled by another guess"
    );
    let unresolved =
        voon_core::cue::parse_bytes_with_audio(&bytes, &[PathBuf::from("/music/unrelated.ape")]);
    assert_eq!(unresolved, original);
}
