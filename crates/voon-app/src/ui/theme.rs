use gpui::{Window, WindowAppearance};

/// Colors and dimensions mirror the reviewed prototype's product.css.
#[derive(Clone, Copy)]
pub struct Theme {
    pub window: u32,
    pub sidebar: u32,
    pub toolbar: u32,
    pub well: u32,
    pub line: u32,
    pub ink: u32,
    pub secondary: u32,
    pub tertiary: u32,
    pub accent: u32,
    pub selected: u32,
    pub selection: u32,
}

impl Theme {
    pub fn for_window(window: &Window) -> Self {
        match window.appearance() {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self {
                window: 0x1e1e20,
                sidebar: 0x2a2a2e,
                toolbar: 0x222225,
                well: 0x262628,
                line: 0x38383c,
                ink: 0xf2f2f5,
                secondary: 0xa1a1a8,
                tertiary: 0x7c7c84,
                accent: 0xe08257,
                selected: 0x414145,
                selection: 0x3d8bfd,
            },
            _ => Self {
                window: 0xffffff,
                sidebar: 0xeceef1,
                toolbar: 0xfafafc,
                well: 0xf4f4f6,
                line: 0xe2e2e6,
                ink: 0x1d1d1f,
                secondary: 0x6e6e73,
                tertiary: 0x8e8e93,
                accent: 0xb4532a,
                selected: 0xdadddf,
                selection: 0x0a64d6,
            },
        }
    }
}

pub const SIDEBAR_WIDTH: f32 = 220.0;
pub const DETAIL_WIDTH: f32 = 340.0;
pub const TOOLBAR_HEIGHT: f32 = 52.0;
pub const FACET_PANEL_WIDTH: f32 = 236.0;
pub const COMPOSER_PANEL_WIDTH: f32 = 236.0;
