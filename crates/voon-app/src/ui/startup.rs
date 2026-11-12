use gpui::{App, Div, Entity, div, prelude::*, rgb};

use crate::view_model::AppState;

use super::{empty, theme::Theme};

pub fn first_use(state: &Entity<AppState>, theme: Theme, cx: &App) -> Div {
    let settings = state.clone();
    empty::albums(state.read(cx), theme).child(
        div()
            .id("add-first-library-root")
            .cursor_pointer()
            .px_4()
            .py_2()
            .rounded_md()
            .bg(rgb(theme.accent))
            .text_color(rgb(theme.window))
            .child("添加音乐目录…")
            .on_click(move |_, _, cx| crate::application::open_settings(settings.clone(), cx)),
    )
}
