//! One intersection and one count cache feed every browse view. Within a Facet
//! dimension choices are alternatives; different dimensions form an intersection.
use super::{AppState, FilterState};
use gpui::Context;
use std::collections::BTreeSet;
use voon_core::{
    AudioFormat,
    db::AlbumRow,
    facets::{Encoding, Facets, Language, Resolution},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FacetDimension {
    Encoding,
    Format,
    Resolution,
    SampleRate,
    BitDepth,
    Language,
    Decade,
    Genre,
}

impl FacetDimension {
    pub const ALL: [Self; 8] = [
        Self::Encoding,
        Self::Format,
        Self::Resolution,
        Self::SampleRate,
        Self::BitDepth,
        Self::Language,
        Self::Decade,
        Self::Genre,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Encoding => "类型",
            Self::Format => "格式",
            Self::Resolution => "规格",
            Self::SampleRate => "采样率",
            Self::BitDepth => "位深",
            Self::Language => "语种",
            Self::Decade => "年代",
            Self::Genre => "流派",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FacetValue {
    Encoding(Encoding),
    Format(AudioFormat),
    Resolution(Resolution),
    MinSampleRate(u32),
    MinBitDepth(u8),
    Language(Language),
    Decade(u16),
    Genre(String),
}

impl FacetValue {
    pub fn dimension(&self) -> FacetDimension {
        match self {
            Self::Encoding(_) => FacetDimension::Encoding,
            Self::Format(_) => FacetDimension::Format,
            Self::Resolution(_) => FacetDimension::Resolution,
            Self::MinSampleRate(_) => FacetDimension::SampleRate,
            Self::MinBitDepth(_) => FacetDimension::BitDepth,
            Self::Language(_) => FacetDimension::Language,
            Self::Decade(_) => FacetDimension::Decade,
            Self::Genre(_) => FacetDimension::Genre,
        }
    }

    fn label(&self) -> String {
        match self {
            Self::Encoding(Encoding::Dsd) => "DSD".into(),
            Self::Encoding(Encoding::Pcm) => "PCM".into(),
            Self::Format(value) => value.label().into(),
            Self::Resolution(value) => match value {
                Resolution::Lossy => "有损".into(),
                Resolution::Cd => "光盘规格".into(),
                Resolution::HiRes => "高解析度".into(),
                Resolution::Dsd(rate) => format!("DSD{rate}"),
                Resolution::Unknown => "未知规格".into(),
            },
            Self::MinSampleRate(rate) => format!("至少 {} 千赫", f64::from(*rate) / 1000.),
            Self::MinBitDepth(bits) => format!("至少 {bits} 位"),
            Self::Language(value) => value.label().into(),
            Self::Decade(year) => format!("{year} 年代"),
            Self::Genre(value) => value.clone(),
        }
    }

    fn matches(&self, facets: &Facets) -> bool {
        match self {
            Self::Encoding(value) => facets.encoding == *value,
            Self::Format(value) => facets.formats.contains(value),
            Self::Resolution(value) => facets.resolution == *value,
            Self::MinSampleRate(value) => facets.max_sample_rate.is_some_and(|rate| rate >= *value),
            Self::MinBitDepth(value) => facets.max_bit_depth.is_some_and(|bits| bits >= *value),
            Self::Language(value) => facets.language == *value,
            Self::Decade(value) => facets.decade == Some(*value),
            Self::Genre(value) => facets.genre.as_ref() == Some(value),
        }
    }
}

pub struct FacetOption {
    pub value: FacetValue,
    pub label: String,
    pub count: usize,
    pub selected: bool,
}

#[derive(Default)]
pub(super) struct FacetState {
    options: Vec<FacetOption>,
}

impl FilterState {
    fn matches(&self, facets: &Facets, except: Option<FacetDimension>) -> bool {
        let skip = |dimension| except == Some(dimension);
        (skip(FacetDimension::Format)
            || self.formats.is_empty()
            || self
                .formats
                .iter()
                .any(|format| facets.formats.contains(format)))
            && (skip(FacetDimension::Encoding)
                || self.encodings.is_empty()
                || self.encodings.contains(&facets.encoding))
            && (skip(FacetDimension::Resolution)
                || self.resolutions.is_empty()
                || self.resolutions.contains(&facets.resolution))
            && (skip(FacetDimension::Language)
                || self.languages.is_empty()
                || self.languages.contains(&facets.language))
            && (skip(FacetDimension::Decade)
                || self.decades.is_empty()
                || facets
                    .decade
                    .is_some_and(|decade| self.decades.contains(&decade)))
            && (skip(FacetDimension::Genre)
                || self.genres.is_empty()
                || facets
                    .genre
                    .as_ref()
                    .is_some_and(|genre| self.genres.contains(genre)))
            && (skip(FacetDimension::SampleRate)
                || self.min_sample_rate.is_none_or(|minimum| {
                    facets.max_sample_rate.is_some_and(|rate| rate >= minimum)
                }))
            && (skip(FacetDimension::BitDepth)
                || self
                    .min_bit_depth
                    .is_none_or(|minimum| facets.max_bit_depth.is_some_and(|bits| bits >= minimum)))
    }

    fn selected_values(&self) -> BTreeSet<FacetValue> {
        self.formats
            .iter()
            .copied()
            .map(FacetValue::Format)
            .chain(self.encodings.iter().copied().map(FacetValue::Encoding))
            .chain(self.resolutions.iter().copied().map(FacetValue::Resolution))
            .chain(self.languages.iter().copied().map(FacetValue::Language))
            .chain(self.decades.iter().copied().map(FacetValue::Decade))
            .chain(self.genres.iter().cloned().map(FacetValue::Genre))
            .chain(self.min_sample_rate.map(FacetValue::MinSampleRate))
            .chain(self.min_bit_depth.map(FacetValue::MinBitDepth))
            .collect()
    }
}

fn toggle<T: PartialEq>(values: &mut Vec<T>, value: T) {
    if let Some(position) = values.iter().position(|existing| existing == &value) {
        values.remove(position);
    } else {
        values.push(value);
    }
}

impl AppState {
    pub fn facet_options(&self) -> &[FacetOption] {
        &self.facet_state.options
    }

    pub fn toggle_facet(&mut self, value: FacetValue, cx: &mut Context<Self>) {
        match value {
            FacetValue::Format(value) => toggle(&mut self.filters.formats, value),
            FacetValue::Encoding(value) => toggle(&mut self.filters.encodings, value),
            FacetValue::Resolution(value) => toggle(&mut self.filters.resolutions, value),
            FacetValue::Language(value) => toggle(&mut self.filters.languages, value),
            FacetValue::Decade(value) => toggle(&mut self.filters.decades, value),
            FacetValue::Genre(value) => toggle(&mut self.filters.genres, value),
            FacetValue::MinSampleRate(value) => {
                self.filters.min_sample_rate =
                    (self.filters.min_sample_rate != Some(value)).then_some(value)
            }
            FacetValue::MinBitDepth(value) => {
                self.filters.min_bit_depth =
                    (self.filters.min_bit_depth != Some(value)).then_some(value)
            }
        }
        self.refresh_browse();
        cx.notify();
    }

    pub fn clear_facets(&mut self, cx: &mut Context<Self>) {
        self.set_filters(FilterState::default(), cx);
    }

    pub(super) fn matches_facets(&self, album: &AlbumRow) -> bool {
        self.filters.matches(&album.facets, None)
    }

    pub(super) fn refresh_facet_counts(&mut self) {
        let selected = self.filters.selected_values();
        let mut values = selected.clone();
        values.extend([
            FacetValue::Encoding(Encoding::Pcm),
            FacetValue::Encoding(Encoding::Dsd),
        ]);
        for album in &self.albums {
            let facets = &album.facets;
            values.extend(facets.formats.iter().copied().map(FacetValue::Format));
            values.insert(FacetValue::Resolution(facets.resolution));
            values.insert(FacetValue::Language(facets.language));
            values.extend(facets.decade.map(FacetValue::Decade));
            values.extend(facets.genre.clone().map(FacetValue::Genre));
            if facets.max_sample_rate.is_some() {
                values.extend(
                    [44_100, 48_000, 88_200, 96_000, 176_400, 192_000, 352_800]
                        .map(FacetValue::MinSampleRate),
                );
            }
            if facets.max_bit_depth.is_some() {
                values.extend([16, 24, 32].map(FacetValue::MinBitDepth));
            }
        }
        let base: Vec<_> = self
            .albums
            .iter()
            .filter(|album| {
                self.matches_collection(album)
                    && self.matches_search(album)
                    && self.matches_composer(album)
                    && self.matches_organization(album)
            })
            .collect();
        self.facet_state.options = values
            .into_iter()
            .map(|value| {
                let count = base
                    .iter()
                    .filter(|album| {
                        self.filters.matches(&album.facets, Some(value.dimension()))
                            && value.matches(&album.facets)
                    })
                    .count();
                FacetOption {
                    label: value.label(),
                    selected: selected.contains(&value),
                    value,
                    count,
                }
            })
            .collect();
    }
}
