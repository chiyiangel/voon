use voon_core::{
    Cover, CoverSource, TrackSource,
    db::{DiscRef, Index, SplitMode, TrackRef},
    scan::{NoTags, scan_root},
};

#[test]
fn merged_albums_survive_restart_and_rescan_and_can_be_undone_without_touching_music() {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let first = library.path().join("Philips Duo 第一张");
    let second = library.path().join("Philips Duo 第二张");
    for path in [&first, &second] {
        std::fs::create_dir(path).unwrap();
        std::fs::write(path.join("01.flac"), b"audio fixture").unwrap();
    }
    let database = local.path().join("index.sqlite3");
    let mut index = Index::open(&database).unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    index.set_user_edit(&first, "title", "主信息源").unwrap();

    let merged = index
        .merge_albums(&[first.clone(), second.clone()], &first, &[])
        .unwrap();

    assert_eq!(index.albums().unwrap().len(), 1);
    let album = index.album(&merged).unwrap().unwrap();
    assert_eq!(album.display_title(), "主信息源");
    assert_eq!(album.discs.len(), 2);
    assert_eq!(
        album.discs[0].tracks[0].source.audio_path(),
        &first.join("01.flac")
    );
    assert_eq!(
        album.discs[1].tracks[0].source.audio_path(),
        &second.join("01.flac")
    );
    drop(index);

    let mut index = Index::open(&database).unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    assert_eq!(index.albums().unwrap().len(), 1);
    assert_eq!(index.album(&merged).unwrap().unwrap().discs.len(), 2);
    let mut interrupted = scan_root(library.path(), &mut NoTags);
    interrupted.albums.retain(|album| album.path != first);
    interrupted.issues.push(voon_core::scan::ScanIssue {
        path: first.clone(),
        kind: voon_core::scan::IssueKind::Unreadable {
            error: "连接中断".into(),
        },
    });
    index.store_scan(library.path(), &interrupted).unwrap();
    drop(index);
    let mut index = Index::open(&database).unwrap();
    assert_eq!(index.album(&merged).unwrap().unwrap().tracks().count(), 2);
    assert_eq!(
        index.album(&merged).unwrap().unwrap().source_directories,
        [first.clone(), second.clone()]
    );
    assert!(index.last_structure_operation().unwrap().is_some());
    assert!(index.rename_candidates().unwrap().is_empty());
    assert!(index.undo_structure().unwrap().is_some());
    assert_eq!(index.albums().unwrap().len(), 2);
    assert_eq!(
        index.album(&first).unwrap().unwrap().display_title(),
        "主信息源"
    );
    for path in [&first, &second] {
        assert_eq!(
            std::fs::read(path.join("01.flac")).unwrap(),
            b"audio fixture"
        );
        assert_eq!(std::fs::read_dir(path).unwrap().count(), 1);
    }
    assert_eq!(std::fs::read_dir(library.path()).unwrap().count(), 2);
}

#[test]
fn undo_after_rescan_restores_original_organization_and_keeps_later_result_edits() {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let first = library.path().join("第一张");
    let second = library.path().join("第二张");
    for path in [&first, &second] {
        std::fs::create_dir_all(path.join("Booklet")).unwrap();
        std::fs::write(path.join("01.flac"), b"fixture").unwrap();
        std::fs::write(path.join("Booklet/user.jpg"), b"image fixture").unwrap();
    }
    let cover_a = Cover {
        source: CoverSource::File(first.join("Booklet/user.jpg")),
    };
    let cover_b = Cover {
        source: CoverSource::File(second.join("Booklet/user.jpg")),
    };
    let database = local.path().join("index.sqlite3");
    let mut index = Index::open(&database).unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    index.set_user_edit(&first, "title", "主标题").unwrap();
    index.set_user_edit(&second, "title", "次标题").unwrap();
    index.add_user_tag(&first, "古典").unwrap();
    index.add_user_tag(&second, "夜听").unwrap();
    let weekend = index.create_manual_collection("周末").unwrap();
    let classics = index.create_manual_collection("名演").unwrap();
    index.add_to_manual_collection(weekend, &first).unwrap();
    index.add_to_manual_collection(classics, &second).unwrap();
    index.set_user_cover(&first, &cover_a).unwrap();
    index.set_user_cover(&second, &cover_b).unwrap();

    let merged = index
        .merge_albums(&[first.clone(), second.clone()], &first, &[])
        .unwrap();
    assert_eq!(index.user_tags(&merged).unwrap(), vec!["古典", "夜听"]);
    assert_eq!(index.user_cover(&merged).unwrap(), Some(cover_a));
    index
        .save_user_edits(&merged, &[("year".into(), Some("2001".into()))])
        .unwrap();
    index.add_user_tag(&merged, "新标签").unwrap();
    let daily = index.create_manual_collection("日常").unwrap();
    index.add_to_manual_collection(daily, &merged).unwrap();
    index.set_user_cover(&merged, &cover_b).unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    drop(index);

    let mut index = Index::open(&database).unwrap();
    index.undo_structure().unwrap().unwrap();

    assert_eq!(index.albums().unwrap().len(), 2);
    assert_eq!(
        index.album(&first).unwrap().unwrap().display_title(),
        "主标题"
    );
    assert_eq!(
        index.album(&second).unwrap().unwrap().display_title(),
        "次标题"
    );
    for path in [&first, &second] {
        assert_eq!(
            index
                .album(path)
                .unwrap()
                .unwrap()
                .metadata
                .year
                .unwrap()
                .value,
            2001
        );
        assert_eq!(index.user_cover(path).unwrap(), Some(cover_b.clone()));
    }
    assert_eq!(index.user_tags(&first).unwrap(), vec!["古典", "新标签"]);
    assert_eq!(index.user_tags(&second).unwrap(), vec!["夜听", "新标签"]);
    assert_eq!(
        index.manual_collection_albums(weekend).unwrap(),
        vec![first.clone()]
    );
    assert_eq!(
        index.manual_collection_albums(classics).unwrap(),
        vec![second.clone()]
    );
    assert_eq!(
        index.manual_collection_albums(daily).unwrap(),
        vec![first, second]
    );
}

#[test]
fn a_cross_root_merge_is_retained_when_a_root_is_removed_and_readded() {
    let first_root = tempfile::tempdir().unwrap();
    let second_root = tempfile::tempdir().unwrap();
    let first = first_root.path().join("第一张");
    let second = second_root.path().join("第二张");
    for path in [&first, &second] {
        std::fs::create_dir(path).unwrap();
        std::fs::write(path.join("01.flac"), b"fixture").unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(
            first_root.path(),
            &scan_root(first_root.path(), &mut NoTags),
        )
        .unwrap();
    index
        .store_scan(
            second_root.path(),
            &scan_root(second_root.path(), &mut NoTags),
        )
        .unwrap();
    let merged = index
        .merge_albums(&[first.clone(), second.clone()], &first, &[])
        .unwrap();
    index.add_user_tag(&merged, "后续标签").unwrap();

    index.remove_root(first_root.path()).unwrap();

    assert_eq!(
        index
            .albums()
            .unwrap()
            .iter()
            .map(|album| album.path.clone())
            .collect::<Vec<_>>(),
        vec![second.clone()]
    );
    index
        .store_scan(
            second_root.path(),
            &scan_root(second_root.path(), &mut NoTags),
        )
        .unwrap();
    assert_eq!(index.albums().unwrap().len(), 1);
    index
        .store_scan(
            first_root.path(),
            &scan_root(first_root.path(), &mut NoTags),
        )
        .unwrap();
    assert_eq!(index.albums().unwrap().len(), 1);
    assert_eq!(index.album(&merged).unwrap().unwrap().discs.len(), 2);
    assert_eq!(index.user_tags(&merged).unwrap(), vec!["后续标签"]);
}

#[test]
fn splitting_by_disc_survives_rescan_and_undo_restores_the_original_album() {
    let library = tempfile::tempdir().unwrap();
    let path = library.path().join("Philips Duo");
    for disc in ["CD1", "CD2"] {
        std::fs::create_dir_all(path.join(disc)).unwrap();
        std::fs::write(path.join(disc).join("01.flac"), b"fixture").unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    index.set_user_edit(&path, "title", "我的套装").unwrap();

    let outputs = index.split_album(&path, SplitMode::Discs).unwrap();

    assert_eq!(outputs.len(), 2);
    assert_eq!(index.albums().unwrap().len(), 2);
    assert_eq!(
        index.album(&outputs[0]).unwrap().unwrap().display_title(),
        "我的套装 · 碟 1"
    );
    assert_eq!(
        index
            .album(&outputs[0])
            .unwrap()
            .unwrap()
            .tracks()
            .next()
            .unwrap()
            .source
            .audio_path(),
        &path.join("CD1/01.flac")
    );
    let mut incomplete = scan_root(library.path(), &mut NoTags);
    incomplete.albums[0].discs.pop();
    incomplete.issues.push(voon_core::scan::ScanIssue {
        path: path.join("CD2"),
        kind: voon_core::scan::IssueKind::Unreadable {
            error: "部分断连".into(),
        },
    });
    index.store_scan(library.path(), &incomplete).unwrap();
    assert_eq!(index.albums().unwrap().len(), 2);
    assert!(
        outputs
            .iter()
            .all(|path| index.album(path).unwrap().unwrap().discs.len() == 1)
    );
    index.undo_structure().unwrap().unwrap();
    assert_eq!(index.albums().unwrap().len(), 1);
    assert_eq!(
        index.album(&path).unwrap().unwrap().display_title(),
        "我的套装"
    );
    assert_eq!(index.album(&path).unwrap().unwrap().discs.len(), 2);
}

#[test]
fn distinct_cue_discs_in_one_directory_can_be_merged_and_reordered() {
    let library = tempfile::tempdir().unwrap();
    let first = library.path().join("双整轨");
    let second = library.path().join("分轨");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&second).unwrap();
    for name in ["CD1", "CD2"] {
        std::fs::write(first.join(format!("{name}.flac")), b"fixture").unwrap();
        std::fs::write(first.join(format!("{name}.cue")),format!("TITLE \"两张整轨\"\nFILE \"{name}.flac\" WAVE\n TRACK 01 AUDIO\n TITLE \"乐章\"\n INDEX 01 00:00:00\n")).unwrap();
    }
    std::fs::write(second.join("01.flac"), b"fixture").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    assert_eq!(index.album(&first).unwrap().unwrap().discs.len(), 2);

    let merged = index
        .merge_albums(
            &[first.clone(), second.clone()],
            &first,
            &[
                DiscRef {
                    album: first.clone(),
                    disc: 1,
                },
                DiscRef {
                    album: first.clone(),
                    disc: 0,
                },
                DiscRef {
                    album: second,
                    disc: 0,
                },
            ],
        )
        .unwrap();

    let album = index.album(&merged).unwrap().unwrap();
    assert_eq!(album.discs.len(), 3);
    assert_eq!(
        album.discs[0].tracks[0].source.audio_path(),
        &first.join("CD2.flac")
    );
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    assert_eq!(
        index.album(&merged).unwrap().unwrap().discs[0].tracks[0]
            .source
            .audio_path(),
        &first.join("CD2.flac")
    );
}

#[test]
fn rescanning_with_ignored_inputs_does_not_revive_their_audio_from_structure_snapshots() {
    use std::sync::atomic::AtomicBool;
    use voon_core::scan::{ScanOptions, scan_root_with_options};
    let library = tempfile::tempdir().unwrap();
    let first = library.path().join("A");
    let second = library.path().join("B");
    for path in [&first, &second] {
        std::fs::create_dir(path).unwrap();
        std::fs::write(path.join("01.flac"), b"fixture").unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    let merged = index
        .merge_albums(&[first, second.clone()], &library.path().join("A"), &[])
        .unwrap();
    let mut options = ScanOptions {
        ignored_paths: vec!["A".into()],
    };
    index
        .store_scan(
            library.path(),
            &scan_root_with_options(
                library.path(),
                &mut NoTags,
                &AtomicBool::new(false),
                &mut |_| {},
                &options,
            ),
        )
        .unwrap();
    let album = index.album(&merged).unwrap().unwrap();
    assert_eq!(album.tracks().count(), 1);
    assert_eq!(
        album.tracks().next().unwrap().source.audio_path(),
        &second.join("01.flac")
    );
    options.ignored_paths.push("B".into());
    index
        .store_scan(
            library.path(),
            &scan_root_with_options(
                library.path(),
                &mut NoTags,
                &AtomicBool::new(false),
                &mut |_| {},
                &options,
            ),
        )
        .unwrap();
    assert!(index.albums().unwrap().is_empty());
    index.undo_structure().unwrap().unwrap();
    assert!(index.albums().unwrap().is_empty());
}

#[test]
fn selected_virtual_tracks_split_without_changing_the_image_or_cue_and_conflicting_edits_block_undo()
 {
    let library = tempfile::tempdir().unwrap();
    let path = library.path().join("整轨专辑");
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("CDImage.ape"), b"fixture").unwrap();
    std::fs::write(path.join("program.cue"),"TITLE \"奏鸣曲\"\nFILE \"CDImage.ape\" WAVE\n TRACK 01 AUDIO\n TITLE \"快板\"\n INDEX 01 00:00:00\n TRACK 02 AUDIO\n TITLE \"行板\"\n INDEX 01 01:00:00\n TRACK 03 AUDIO\n TITLE \"终曲\"\n INDEX 01 02:00:00\n").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    assert!(
        index
            .split_album(
                &path,
                SplitMode::SelectedTracks(vec![
                    TrackRef { disc: 0, track: 0 },
                    TrackRef { disc: 0, track: 1 },
                    TrackRef { disc: 0, track: 2 }
                ])
            )
            .is_err()
    );

    let outputs = index
        .split_album(
            &path,
            SplitMode::SelectedTracks(vec![
                TrackRef { disc: 0, track: 0 },
                TrackRef { disc: 0, track: 2 },
            ]),
        )
        .unwrap();

    assert_eq!(
        index.album(&outputs[0]).unwrap().unwrap().tracks().count(),
        2
    );
    assert_eq!(
        index.album(&outputs[1]).unwrap().unwrap().tracks().count(),
        1
    );
    assert!(
        index
            .album(&outputs[0])
            .unwrap()
            .unwrap()
            .tracks()
            .all(|track| matches!(track.source, TrackSource::Virtual { .. }))
    );
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    assert_eq!(
        index
            .album(&outputs[1])
            .unwrap()
            .unwrap()
            .tracks()
            .next()
            .unwrap()
            .title
            .as_ref()
            .unwrap()
            .value,
        "行板"
    );
    index
        .set_user_edit(&outputs[0], "composer", "作曲家甲")
        .unwrap();
    index
        .set_user_edit(&outputs[1], "composer", "作曲家乙")
        .unwrap();
    assert!(index.undo_structure().is_err());
    assert_eq!(index.albums().unwrap().len(), 2);
    index
        .set_user_edit(&outputs[1], "composer", "作曲家甲")
        .unwrap();
    index.undo_structure().unwrap().unwrap();
    let restored = index.album(&path).unwrap().unwrap();
    assert_eq!(restored.tracks().count(), 3);
    assert_eq!(restored.metadata.composer.unwrap().value, "作曲家甲");
    assert_eq!(std::fs::read(path.join("CDImage.ape")).unwrap(), b"fixture");
    assert_eq!(std::fs::read_dir(path).unwrap().count(), 2);
}

#[test]
fn confirming_a_physical_rename_updates_the_merge_sources_and_cover_paths() {
    let library = tempfile::tempdir().unwrap();
    let first = library.path().join("A");
    let second = library.path().join("B");
    let renamed = library.path().join("C");
    for (path, bytes) in [
        (&first, b"one".as_slice()),
        (&second, b"different fixture".as_slice()),
    ] {
        std::fs::create_dir(path).unwrap();
        std::fs::write(path.join("01.flac"), bytes).unwrap();
    }
    std::fs::write(first.join("chosen.jpg"), b"cover").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    index
        .set_user_cover(
            &first,
            &Cover {
                source: CoverSource::File(first.join("chosen.jpg")),
            },
        )
        .unwrap();
    let merged = index
        .merge_albums(&[first.clone(), second.clone()], &first, &[])
        .unwrap();
    std::fs::rename(&first, &renamed).unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();

    index.confirm_rename(&first, &renamed).unwrap();

    assert_eq!(index.albums().unwrap().len(), 1);
    let album = index.album(&merged).unwrap().unwrap();
    assert_eq!(album.tracks().count(), 2);
    assert!(
        album
            .tracks()
            .any(|track| track.source.audio_path() == &renamed.join("01.flac"))
    );
    assert_eq!(
        index.user_cover(&merged).unwrap(),
        Some(Cover {
            source: CoverSource::File(renamed.join("chosen.jpg"))
        })
    );
    index.undo_structure().unwrap().unwrap();
    assert_eq!(index.albums().unwrap().len(), 2);
    assert!(index.album(&renamed).unwrap().is_some());
    assert!(index.user_cover(&second).unwrap().is_none());
}

#[test]
fn removing_the_first_file_of_a_disc_keeps_the_same_split_result_and_user_data() {
    let library = tempfile::tempdir().unwrap();
    let path = library.path().join("套装");
    for disc in ["CD1", "CD2"] {
        std::fs::create_dir_all(path.join(disc)).unwrap();
        std::fs::write(path.join(disc).join("01.flac"), b"fixture").unwrap();
    }
    std::fs::write(path.join("CD1/02.flac"), b"fixture").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    let outputs = index.split_album(&path, SplitMode::Discs).unwrap();
    index.add_user_tag(&outputs[0], "第一碟整理").unwrap();
    std::fs::remove_file(path.join("CD1/01.flac")).unwrap();

    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();

    let first = index.album(&outputs[0]).unwrap().unwrap();
    assert_eq!(first.tracks().count(), 1);
    assert_eq!(
        first.tracks().next().unwrap().source.audio_path(),
        &path.join("CD1/02.flac")
    );
    assert_eq!(index.user_tags(&outputs[0]).unwrap(), vec!["第一碟整理"]);
    assert_eq!(index.albums().unwrap().len(), 2);
}

#[test]
fn splitting_a_merge_keeps_each_real_source_and_later_tags_survive_both_undos() {
    let library = tempfile::tempdir().unwrap();
    let first = library.path().join("A");
    let second = library.path().join("B");
    for path in [&first, &second] {
        std::fs::create_dir(path).unwrap();
        std::fs::write(path.join("01.flac"), b"fixture").unwrap();
    }
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    let merged = index
        .merge_albums(&[first.clone(), second.clone()], &first, &[])
        .unwrap();
    assert_eq!(
        index.album(&merged).unwrap().unwrap().source_directories,
        vec![first.clone(), second.clone()]
    );

    let parts = index.split_album(&merged, SplitMode::Discs).unwrap();

    assert_eq!(
        index.album(&parts[0]).unwrap().unwrap().source_directories,
        vec![first.clone()]
    );
    assert_eq!(
        index.album(&parts[1]).unwrap().unwrap().source_directories,
        vec![second.clone()]
    );
    index.add_user_tag(&parts[0], "拆分后编辑").unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    index.undo_structure().unwrap().unwrap();
    assert_eq!(index.albums().unwrap().len(), 1);
    assert_eq!(index.user_tags(&merged).unwrap(), vec!["拆分后编辑"]);
    index.undo_structure().unwrap().unwrap();
    assert_eq!(index.albums().unwrap().len(), 2);
    assert_eq!(index.user_tags(&first).unwrap(), vec!["拆分后编辑"]);
    assert_eq!(index.user_tags(&second).unwrap(), vec!["拆分后编辑"]);
}

#[test]
fn split_virtual_track_groups_can_be_merged_and_split_by_disc_without_duplicating_tracks() {
    let library = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let path = library.path().join("整轨专辑");
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("CDImage.ape"), b"fixture").unwrap();
    std::fs::write(
        path.join("program.cue"),
        "TITLE \"全曲\"\nFILE \"CDImage.ape\" WAVE\n TRACK 01 AUDIO\n TITLE \"第一乐章\"\n INDEX 01 00:00:00\n TRACK 02 AUDIO\n TITLE \"第二乐章\"\n INDEX 01 01:00:00\n TRACK 03 AUDIO\n TITLE \"第三乐章\"\n INDEX 01 02:00:00\n TRACK 04 AUDIO\n TITLE \"第四乐章\"\n INDEX 01 03:00:00\n",
    ).unwrap();
    let database = local.path().join("index.sqlite3");
    let mut index = Index::open(&database).unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    let groups = index
        .split_album(
            &path,
            SplitMode::SelectedTracks(vec![
                TrackRef { disc: 0, track: 0 },
                TrackRef { disc: 0, track: 2 },
            ]),
        )
        .unwrap();
    let merged = index.merge_albums(&groups, &groups[0], &[]).unwrap();
    assert_eq!(index.album(&merged).unwrap().unwrap().discs.len(), 2);

    let discs = index.split_album(&merged, SplitMode::Discs).unwrap();

    let starts = |index: &Index| {
        let mut starts = discs
            .iter()
            .flat_map(|path| {
                index
                    .album(path)
                    .unwrap()
                    .unwrap()
                    .tracks()
                    .map(|track| match track.source {
                        TrackSource::Virtual { start, .. } => start,
                        _ => panic!("整轨曲目应保持虚拟音轨"),
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        starts.sort_unstable();
        starts
    };
    assert_eq!(starts(&index), vec![0, 4_500, 9_000, 13_500]);
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    drop(index);
    let index = Index::open(&database).unwrap();
    assert_eq!(starts(&index), vec![0, 4_500, 9_000, 13_500]);
}

#[test]
fn unresolved_rename_candidates_must_be_decided_before_merging_or_splitting() {
    let library = tempfile::tempdir().unwrap();
    let old = library.path().join("A");
    let renamed = library.path().join("B");
    let other = library.path().join("C");
    for (path, bytes) in [
        (&old, b"first fixture".as_slice()),
        (&other, b"different fixture".as_slice()),
    ] {
        std::fs::create_dir(path).unwrap();
        std::fs::write(path.join("01.flac"), bytes).unwrap();
    }
    std::fs::write(old.join("02.flac"), b"second fixture").unwrap();
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    std::fs::rename(&old, &renamed).unwrap();
    index
        .store_scan(library.path(), &scan_root(library.path(), &mut NoTags))
        .unwrap();
    assert_eq!(index.rename_candidates().unwrap().len(), 1);

    let merge_error = index
        .merge_albums(&[renamed.clone(), other.clone()], &renamed, &[])
        .unwrap_err();
    assert!(
        merge_error
            .to_string()
            .contains("请先确认或忽略改名候选，再调整分组")
    );
    let split_error = index
        .split_album(
            &renamed,
            SplitMode::SelectedTracks(vec![TrackRef { disc: 0, track: 0 }]),
        )
        .unwrap_err();
    assert!(
        split_error
            .to_string()
            .contains("请先确认或忽略改名候选，再调整分组")
    );
    assert_eq!(index.albums().unwrap().len(), 2);
    assert!(index.last_structure_operation().unwrap().is_none());

    index.dismiss_rename(&old, &renamed).unwrap();
    let merged = index
        .merge_albums(&[renamed.clone(), other], &renamed, &[])
        .unwrap();
    assert_eq!(index.album(&merged).unwrap().unwrap().tracks().count(), 3);
}
