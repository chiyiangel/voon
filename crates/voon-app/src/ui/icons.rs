use std::borrow::Cow;

use gpui::{AssetSource, SharedString, Svg, prelude::*, px, rgb, svg};

/// Small vector assets are embedded so the shell also works from a packaged app.
pub struct Assets;

const ICONS: &[(&str, &str)] = &[
    ("chevron-right", "M9 6l6 6-6 6"),
    ("chevron-down", "M6 9l6 6 6-6"),
    ("folder", "M3 6h7l2 2h9v12H3z M3 6V4h7l2 2"),
    (
        "refresh",
        "M20 7a9 9 0 0 0-15-2L2 8 M2 3v5h5 M4 17a9 9 0 0 0 15 2l3-3 M22 21v-5h-5",
    ),
    (
        "database",
        "M3 6a9 3 0 1 0 18 0 9 3 0 1 0-18 0 M3 6v12a9 3 0 0 0 18 0V6 M3 12a9 3 0 0 0 18 0",
    ),
    ("library", "M3 3h18v18H3z M7 3v18 M11 7h6 M11 11h6 M11 15h4"),
    (
        "box",
        "M3 7l9-4 9 4v11l-9 4-9-4z M3 7l9 4 9-4 M12 11v11 M7 5l10 4",
    ),
    (
        "composer",
        "M8 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8 M2 21v-3a5 5 0 0 1 10 0v3 M17 15V5l5-1v9 M17 15a2 2 0 1 1-2-2 2 2 0 0 1 2 2 M22 13a2 2 0 1 1-2-2 2 2 0 0 1 2 2",
    ),
    ("facet", "M3 5h18 M6 12h12 M9 19h6 M8 3v4 M16 10v4 M12 17v4"),
    ("tag", "M3 3h8l10 10-8 8L3 11z M7 7h.01"),
    ("stack", "M3 7l9-4 9 4-9 4z M3 12l9 4 9-4 M3 17l9 4 9-4"),
    ("inbox", "M3 4h18v16H3z M3 14h5l2 3h4l2-3h5"),
    (
        "search",
        "M16 10a6 6 0 1 1-12 0 6 6 0 0 1 12 0 M14.5 14.5L21 21",
    ),
    (
        "grid",
        "M3 3h7v7H3z M14 3h7v7h-7z M3 14h7v7H3z M14 14h7v7h-7z",
    ),
    ("list", "M3 5h2 M9 5h12 M3 12h2 M9 12h12 M3 19h2 M9 19h12"),
    ("sort", "M7 3v18 M3 17l4 4 4-4 M14 5h7 M14 10h5 M14 15h3"),
    (
        "music",
        "M9 17V5l11-2v12 M9 17a3 3 0 1 1-3-3 3 3 0 0 1 3 3 M20 15a3 3 0 1 1-3-3 3 3 0 0 1 3 3",
    ),
    (
        "eye",
        "M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12 M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0",
    ),
];

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        let name = path
            .strip_prefix("icons/")
            .and_then(|p| p.strip_suffix(".svg"));
        Ok(ICONS.iter().find(|(key, _)| Some(*key) == name).map(|(_, data)| {
            Cow::Owned(format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="{data}" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>"#).into_bytes())
        }))
    }

    fn list(&self, _path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(ICONS
            .iter()
            .map(|(name, _)| format!("icons/{name}.svg").into())
            .collect())
    }
}

pub fn icon(name: &str, size: f32, color: u32) -> Svg {
    svg()
        .path(format!("icons/{name}.svg"))
        .size(px(size))
        .flex_none()
        .text_color(rgb(color))
}
