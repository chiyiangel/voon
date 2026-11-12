//! GPUI views only render state and forward user operations.

mod album_detail;
mod album_grid;
mod album_list;
pub mod application_menu_bar;
mod collection_view;
mod composer_view;
mod cover;
mod cover_picker;
mod edit_sheet;
mod empty;
mod facet_view;
mod icons;
mod inbox_view;
mod merge_sheet;
mod metadata_sources;
mod scan_banner;
mod settings;
mod settings_window;
mod sidebar;
mod startup;
mod tags_view;
pub mod text_input;
mod theme;
mod toolbar;
mod window;

pub use album_grid::AlbumGrid;
pub use icons::Assets;
pub use settings_window::{SettingsPage, SettingsWindow};
pub use window::VoonWindow;
