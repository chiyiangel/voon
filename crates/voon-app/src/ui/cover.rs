use gpui::{AnyElement, ObjectFit, div, img, prelude::*, px, rgb};

use super::{icons::icon, theme::Theme};
use crate::view_model::grid::CoverStatus;

pub fn render(status: Option<CoverStatus>, edge: f32, selected: bool, theme: Theme) -> AnyElement {
    let placeholder = match &status {
        Some(CoverStatus::Pending) => "尚未缓存封面",
        Some(CoverStatus::Loading) => "正在缓存封面",
        Some(CoverStatus::Unavailable) => "封面不可用",
        _ => "无封面",
    };
    let picture = match status {
        Some(CoverStatus::Ready(image)) => img(image)
            .size_full()
            .object_fit(ObjectFit::Cover)
            .into_any_element(),
        _ => div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .bg(rgb(theme.well))
            .text_color(rgb(theme.tertiary))
            .child(icon("music", (edge * 0.28).clamp(14., 30.), theme.tertiary))
            .when(edge >= 64., |cover| {
                cover.child(div().text_size(px(11.)).child(placeholder))
            })
            .into_any_element(),
    };
    div()
        .size(px(edge))
        .flex_none()
        .rounded(px(6.))
        .overflow_hidden()
        .when(selected, |cover| {
            cover.border_3().border_color(rgb(theme.selection))
        })
        .child(picture)
        .into_any_element()
}
