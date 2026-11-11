use gpui::{AppContext, TestAppContext};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::AtomicBool,
        mpsc::{self, Receiver},
    },
};
use voon_app::view_model::{
    AppState,
    cover_picker::{CoverLoadResult, CoverLoader},
};
use voon_core::{covers::CoverCandidate, db::Index, *};

struct ReadyLoader(Vec<CoverCandidate>);
impl CoverLoader for ReadyLoader {
    fn start(&self, _: Album, _: Vec<PathBuf>, _: Arc<AtomicBool>) -> Receiver<CoverLoadResult> {
        let (sender, receiver) = mpsc::channel();
        sender.send(Ok(self.0.clone())).unwrap();
        receiver
    }
}

#[gpui::test]
fn cover_drafts_apply_cancel_and_restore_without_changing_automatic_cover(cx: &mut TestAppContext) {
    let auto = Cover {
        source: CoverSource::File("/假库/专辑/cover.jpg".into()),
    };
    let chosen = Cover {
        source: CoverSource::File("/假库/专辑/Booklet/front.jpg".into()),
    };
    let album = Album {
        source_directories: vec![],
        candidates: vec![],
        path: "/假库/专辑".into(),
        folder_name: "专辑".into(),
        metadata: AlbumMetadata::default(),
        cover: Some(auto.clone()),
        discs: vec![],
        collection: None,
        signature: "cover-fixture".into(),
    };
    let mut index = Index::open_in_memory().unwrap();
    index
        .store_scan(
            std::path::Path::new("/假库"),
            &ScanResult {
                albums: vec![album],
                ..ScanResult::default()
            },
        )
        .unwrap();
    let state = cx.new(|_| {
        AppState::new(index, "/建议目录".into())
            .unwrap()
            .with_cover_loader(Box::new(ReadyLoader(vec![CoverCandidate {
                cover: chosen.clone(),
                label: "front.jpg".into(),
            }])))
    });
    state.update(cx, |state, cx| {
        state.select_album(Some("/假库/专辑".into()), cx);
        state.begin_cover_picker(cx).unwrap();
        state.poll_scan(cx);
        state
            .select_cover_candidate(Some(chosen.clone()), cx)
            .unwrap();
        assert_eq!(state.selected_album().unwrap().cover, Some(auto.clone()));
        state.cancel_cover_picker(cx);
        assert_eq!(state.selected_album().unwrap().cover, Some(auto.clone()));
        state.begin_cover_picker(cx).unwrap();
        state.poll_scan(cx);
        state
            .select_cover_candidate(Some(chosen.clone()), cx)
            .unwrap();
        state.apply_cover_choice(cx).unwrap();
        assert_eq!(state.selected_album().unwrap().cover, Some(chosen.clone()));
        assert_eq!(state.album_detail().unwrap().cover, Some(chosen));
        state.begin_cover_picker(cx).unwrap();
        state.select_cover_candidate(None, cx).unwrap();
        state.apply_cover_choice(cx).unwrap();
        assert_eq!(state.selected_album().unwrap().cover, Some(auto));
        assert!(state.cover_picker().is_none());
    });
}
