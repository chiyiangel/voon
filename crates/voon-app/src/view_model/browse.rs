//! One cached query result feeds both the grid and list views.

use gpui::Context;
use voon_core::db::AlbumRow;

use super::{AppState, SortOrder};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewMode {
    #[default]
    Grid,
    List,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    #[default]
    Descending,
}

#[derive(Default)]
pub(super) struct BrowseState {
    mode: ViewMode,
    visible: Vec<AlbumRow>,
    direction: SortDirection,
}

impl AppState {
    pub fn view_mode(&self) -> ViewMode {
        self.browse.mode
    }

    pub fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        self.browse.mode = mode;
        cx.notify();
    }

    pub fn browsed_albums(&self) -> &[AlbumRow] {
        &self.browse.visible
    }

    pub fn sort_direction(&self) -> SortDirection {
        self.browse.direction
    }

    pub fn toggle_sort(&mut self, order: SortOrder, cx: &mut Context<Self>) {
        if self.sort_order == order {
            self.browse.direction = match self.browse.direction {
                SortDirection::Ascending => SortDirection::Descending,
                SortDirection::Descending => SortDirection::Ascending,
            };
        } else {
            self.sort_order = order;
            self.reset_sort_direction();
        }
        self.refresh_browse();
        cx.notify();
    }

    pub(super) fn reset_sort_direction(&mut self) {
        self.browse.direction = if self.sort_order == SortOrder::RecentlyAdded {
            SortDirection::Descending
        } else {
            SortDirection::Ascending
        };
    }

    pub(super) fn refresh_browse(&mut self) {
        self.browse.visible = self
            .albums
            .iter()
            .filter(|album| {
                self.matches_collection(album)
                    && self.matches_search(album)
                    && self.matches_facets(album)
                    && self.matches_organization(album)
                    && self.matches_composer(album)
            })
            .cloned()
            .collect();
        let direction = self.browse.direction;
        let manual_rank = &self.organization.rank;
        self.browse.visible.sort_by(|a, b| {
            let comparison = match self.sort_order {
                SortOrder::RecentlyAdded => {
                    (a.id == 0, a.added_order).cmp(&(b.id == 0, b.added_order))
                }
                SortOrder::Title => a
                    .display_title()
                    .to_lowercase()
                    .cmp(&b.display_title().to_lowercase()),
                SortOrder::Artist => a
                    .metadata
                    .album_artist
                    .as_ref()
                    .map(|field| field.value.to_lowercase())
                    .cmp(
                        &b.metadata
                            .album_artist
                            .as_ref()
                            .map(|field| field.value.to_lowercase()),
                    ),
                SortOrder::Composer => a
                    .metadata
                    .composer
                    .as_ref()
                    .map(|field| field.value.to_lowercase())
                    .cmp(
                        &b.metadata
                            .composer
                            .as_ref()
                            .map(|field| field.value.to_lowercase()),
                    ),
                SortOrder::Year => a
                    .metadata
                    .year
                    .as_ref()
                    .map(|field| field.value)
                    .cmp(&b.metadata.year.as_ref().map(|field| field.value)),
                SortOrder::Format => formats_label(a)
                    .to_lowercase()
                    .cmp(&formats_label(b).to_lowercase()),
                SortOrder::Resolution => resolution_key(a).cmp(&resolution_key(b)),
                SortOrder::DiscTracks => {
                    (a.disc_count, a.track_count).cmp(&(b.disc_count, b.track_count))
                }
                SortOrder::Collection => a.collection.cmp(&b.collection),
                SortOrder::Manual => manual_rank.get(&a.path).cmp(&manual_rank.get(&b.path)),
            };
            let comparison = if direction == SortDirection::Descending {
                comparison.reverse()
            } else {
                comparison
            };
            comparison.then_with(|| a.path.cmp(&b.path))
        });
        self.refresh_facet_counts();
        self.refresh_composer_counts();
    }

    pub(super) fn matches_search(&self, album: &AlbumRow) -> bool {
        let query = self.search.trim().to_lowercase();
        query.is_empty()
            || [
                Some(album.display_title()),
                album
                    .metadata
                    .album_artist
                    .as_ref()
                    .map(|field| field.value.as_str()),
                album
                    .metadata
                    .composer
                    .as_ref()
                    .map(|field| field.value.as_str()),
                album
                    .metadata
                    .catalog_number
                    .as_ref()
                    .map(|field| field.value.as_str()),
            ]
            .into_iter()
            .flatten()
            .any(|value| value.to_lowercase().contains(&query))
    }
}

impl SortOrder {
    pub const ALL: [Self; 10] = [
        Self::RecentlyAdded,
        Self::Title,
        Self::Artist,
        Self::Composer,
        Self::Year,
        Self::Format,
        Self::Resolution,
        Self::DiscTracks,
        Self::Collection,
        Self::Manual,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::RecentlyAdded => "最近添加",
            Self::Title => "专辑",
            Self::Artist => "艺术家 / 演奏者",
            Self::Composer => "作曲家",
            Self::Year => "年份",
            Self::Format => "格式",
            Self::Resolution => "规格",
            Self::DiscTracks => "碟 / 轨",
            Self::Collection => "所属目录",
            Self::Manual => "合集顺序",
        }
    }

    pub fn cell_text(self, album: &AlbumRow) -> String {
        let text = match self {
            Self::Title => album.display_title().to_string(),
            Self::Artist => album
                .metadata
                .album_artist
                .as_ref()
                .map(|field| field.value.clone())
                .unwrap_or_default(),
            Self::Composer => album
                .metadata
                .composer
                .as_ref()
                .map(|field| field.value.clone())
                .unwrap_or_default(),
            Self::Year => album
                .metadata
                .year
                .as_ref()
                .map(|field| field.value.to_string())
                .unwrap_or_default(),
            Self::Format => formats_label(album),
            Self::Resolution => resolution_label(album),
            Self::DiscTracks => format!("{} 碟 · {} 轨", album.disc_count, album.track_count),
            Self::Collection => album
                .collection
                .as_deref()
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            Self::RecentlyAdded | Self::Manual => String::new(),
        };
        if text.is_empty() { "—".into() } else { text }
    }
}

pub fn formats_label(album: &AlbumRow) -> String {
    let mut formats: Vec<_> = album
        .facets
        .formats
        .iter()
        .map(|format| format.label())
        .collect();
    formats.sort_unstable();
    formats.join(" / ")
}

pub fn resolution_label(album: &AlbumRow) -> String {
    use voon_core::facets::Resolution;
    match album.facets.resolution {
        Resolution::Dsd(rate) => format!("DSD{rate}"),
        Resolution::Lossy => "有损".into(),
        Resolution::Unknown => "未知规格".into(),
        _ => {
            let bits = album
                .facets
                .max_bit_depth
                .map(|bits| bits.to_string())
                .unwrap_or_else(|| "?".into());
            let rate = album
                .facets
                .max_sample_rate
                .map(|rate| {
                    let value = format!("{:.1}", f64::from(rate) / 1000.);
                    value.strip_suffix(".0").unwrap_or(&value).to_string()
                })
                .unwrap_or_else(|| "?".into());
            format!("{bits} bit / {rate} kHz")
        }
    }
}

fn resolution_key(album: &AlbumRow) -> (u8, u32, u8) {
    use voon_core::facets::Resolution;
    let class = match album.facets.resolution {
        Resolution::Unknown => 0,
        Resolution::Lossy => 1,
        Resolution::Cd => 2,
        Resolution::HiRes => 3,
        Resolution::Dsd(_) => 4,
    };
    (
        class,
        album.facets.max_sample_rate.unwrap_or_default(),
        album.facets.max_bit_depth.unwrap_or_default(),
    )
}
