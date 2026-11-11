//! Voon's core: the music library model, the read-only scanner and the
//! local index. Nothing here depends on the GUI.

pub mod covers;
pub mod cue;
pub mod db;
pub mod facets;
pub mod folder_name;
pub mod model;
pub mod scan;
pub mod tags;

pub use model::*;
pub use scan::{ScanResult, scan_root, scan_root_with};
