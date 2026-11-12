//! Application state is a GPUI entity. Views read it and forward operations;
//! library access stays here, behind the injected local Index.

use std::path::{Path, PathBuf};

use gpui::Context;
use voon_core::{
    AudioFormat, LibraryRoot,
    db::{AlbumRow, Index},
};

pub mod attention;
pub mod browse;
pub mod collections;
pub mod composers;
pub mod cover_picker;
pub mod cover_thumbnails;
pub mod detail;
pub mod edit;
pub mod facets;
pub mod grid;
pub mod ignore;
pub mod library_index;
mod offline;
pub mod organization;
pub mod player;
pub mod scanning;
pub mod structure;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Navigation {
    #[default]
    AllAlbums,
    Collections,
    Composers,
    Facets,
    UserTags,
    ManualCollections,
    NeedsAttention,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ScanState {
    #[default]
    Idle,
    Scanning {
        root: PathBuf,
        current_directory: PathBuf,
        discovered: usize,
    },
    Done {
        root: PathBuf,
        discovered: usize,
        reads: usize,
    },
    Cancelled {
        root: PathBuf,
        discovered: usize,
    },
    Missing {
        root: PathBuf,
    },
    Empty {
        root: PathBuf,
    },
}

/// Filter choices belong to the state even before the Facet view is added.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FilterState {
    pub formats: Vec<AudioFormat>,
    pub resolutions: Vec<voon_core::facets::Resolution>,
    pub encodings: Vec<voon_core::facets::Encoding>,
    pub languages: Vec<voon_core::facets::Language>,
    pub decades: Vec<u16>,
    pub genres: Vec<String>,
    pub min_sample_rate: Option<u32>,
    pub min_bit_depth: Option<u8>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortOrder {
    #[default]
    RecentlyAdded,
    Title,
    Artist,
    Composer,
    Year,
    Format,
    Resolution,
    DiscTracks,
    Collection,
    Manual,
}

pub struct AppState {
    index: Index,
    library_index: library_index::LibraryIndexState,
    roots: Vec<LibraryRoot>,
    albums: Vec<AlbumRow>,
    suggested_root: PathBuf,
    navigation: Navigation,
    scan_state: ScanState,
    scan_operation: scanning::ScanOperation,
    selected_album: Option<PathBuf>,
    detail: detail::DetailState,
    edit_draft: Option<edit::AlbumEditDraft>,
    search: String,
    filters: FilterState,
    sort_order: SortOrder,
    browse: browse::BrowseState,
    organization: organization::OrganizationState,
    collections: collections::CollectionState,
    facet_state: facets::FacetState,
    composers: composers::ComposerState,
    cover_picker: cover_picker::CoverPickerState,
    structure: structure::StructureState,
    cover_revision: u64,
    thumbnails: cover_thumbnails::CoverThumbnails,
    scanner: Box<dyn scanning::Scanner>,
    scan_session: Option<scanning::ScanSession>,
    pending_empty_scan: Option<scanning::PendingEmptyScan>,
    availability: offline::RootAvailability,
    error: Option<String>,
    settings_error: Option<String>,
    pending_root_removal: Option<PathBuf>,
    attention: Vec<voon_core::db::AttentionItem>,
    attention_path: Option<PathBuf>,
}

impl AppState {
    /// An in-memory Index can be supplied by tests. Construction only queries
    /// the local index; it never checks or scans the suggested Library root.
    pub fn new(index: Index, suggested_root: PathBuf) -> Result<Self> {
        let collections = collections::CollectionState::load(&index)?;
        let organization = organization::OrganizationState::load(&index)?;
        let structure = structure::StructureState::load(&index)?;
        let scanner = Box::new(scanning::DiskScanner::new(
            index.database_path().map(Path::to_path_buf),
        ));
        let thumbnails = cover_thumbnails::CoverThumbnails::new(index.database_path());
        let mut state = Self {
            roots: index.roots()?,
            albums: index.albums()?,
            attention: index.attention_items()?,
            attention_path: None,
            index,
            library_index: library_index::LibraryIndexState::default(),
            suggested_root,
            navigation: Navigation::AllAlbums,
            scan_state: ScanState::Idle,
            scan_operation: scanning::ScanOperation::default(),
            selected_album: None,
            detail: detail::DetailState::default(),
            edit_draft: None,
            search: String::new(),
            filters: FilterState::default(),
            sort_order: SortOrder::RecentlyAdded,
            browse: browse::BrowseState::default(),
            organization,
            structure,
            collections,
            facet_state: facets::FacetState::default(),
            composers: composers::ComposerState::default(),
            cover_picker: cover_picker::CoverPickerState::default(),
            cover_revision: 0,
            thumbnails,
            scanner,
            scan_session: None,
            pending_empty_scan: None,
            availability: offline::RootAvailability::default(),
            error: None,
            settings_error: None,
            pending_root_removal: None,
        };
        state.refresh_browse();
        Ok(state)
    }

    pub fn roots(&self) -> &[LibraryRoot] {
        &self.roots
    }

    pub fn albums(&self) -> &[AlbumRow] {
        &self.albums
    }

    pub fn suggested_root(&self) -> &Path {
        &self.suggested_root
    }

    pub fn navigation(&self) -> Navigation {
        self.navigation
    }

    pub fn navigate(&mut self, destination: Navigation, cx: &mut Context<Self>) {
        if self.navigation != destination {
            self.navigation = destination;
            if destination != Navigation::ManualCollections && self.sort_order == SortOrder::Manual
            {
                self.sort_order = SortOrder::RecentlyAdded;
                self.reset_sort_direction();
            }
            if destination == Navigation::AllAlbums {
                self.collections.clear_selection();
                self.composers.clear_selection();
            }
            self.clear_album_selection();
            self.refresh_browse();
            cx.notify();
        }
    }

    pub fn scan_state(&self) -> &ScanState {
        &self.scan_state
    }

    pub fn search(&self) -> &str {
        &self.search
    }

    pub fn set_search(&mut self, search: String, cx: &mut Context<Self>) {
        self.search = search;
        self.refresh_browse();
        cx.notify();
    }

    pub fn filters(&self) -> &FilterState {
        &self.filters
    }

    pub fn set_filters(&mut self, filters: FilterState, cx: &mut Context<Self>) {
        self.filters = filters;
        self.refresh_browse();
        cx.notify();
    }

    pub fn sort_order(&self) -> SortOrder {
        self.sort_order
    }

    pub fn set_sort_order(&mut self, sort_order: SortOrder, cx: &mut Context<Self>) {
        self.sort_order = sort_order;
        self.reset_sort_direction();
        self.refresh_browse();
        cx.notify();
    }

    pub fn selected_album(&self) -> Option<&AlbumRow> {
        let selected = self.selected_album.as_ref()?;
        self.albums.iter().find(|album| &album.path == selected)
    }

    pub fn select_album(&mut self, path: Option<PathBuf>, cx: &mut Context<Self>) {
        self.selected_album =
            path.filter(|path| self.albums.iter().any(|album| &album.path == path));
        self.organization
            .select_single(self.selected_album.as_deref());
        if self
            .edit_draft
            .as_ref()
            .is_some_and(|draft| Some(draft.album_path()) != self.selected_album.as_deref())
        {
            self.edit_draft = None;
        }
        if let Err(error) = self.refresh_selected_album(cx) {
            self.detail.album = None;
            self.error = Some(format!("无法读取专辑详情：{error}"));
        }
        cx.notify();
    }

    /// Refresh the view-model after a local Index operation. Load both queries
    /// before replacing state so an error does not leave a partial snapshot.
    pub fn reload_library(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let roots = self.index.roots()?;
        let albums = self.index.albums()?;
        let collections = self.index.collection_tree()?;
        let attention = self.index.attention_items()?;
        self.roots = roots;
        self.albums = albums;
        self.collections.replace_tree(collections);
        self.attention = attention;
        self.structure.last = self.index.last_structure_operation()?;
        self.refresh_organization()?;
        self.refresh_browse();
        if self.selected_album().is_none() {
            self.selected_album = None;
            self.edit_draft = None;
        }
        self.refresh_selected_album(cx)?;
        self.refresh_covers();
        self.prefetch_covers(cx);
        cx.notify();
        Ok(())
    }
}
