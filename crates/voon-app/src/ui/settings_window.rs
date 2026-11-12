use gpui::{Context, Entity, FocusHandle, Render, Subscription, Window, div, prelude::*, px, rgb};

use super::{application_menu_bar::ApplicationMenuBar, settings, theme::Theme};
use crate::view_model::AppState;

pub struct SettingsWindow {
    state: Entity<AppState>,
    menu_bar: Entity<ApplicationMenuBar>,
    focus: FocusHandle,
    _subscription: Subscription,
}

impl SettingsWindow {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window);
        let subscription = cx.observe(&state, |_, _, cx| cx.notify());
        let menu_bar = cx.new(|cx| ApplicationMenuBar::new(state.clone(), cx));
        Self {
            state,
            menu_bar,
            focus,
            _subscription: subscription,
        }
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window);
        div()
            .size_full()
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .bg(rgb(theme.window))
            .text_color(rgb(theme.ink))
            .font_family(".SystemUIFont")
            .text_size(px(13.))
            .when(cfg!(target_os = "linux"), |root| {
                root.child(self.menu_bar.clone())
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(settings::settings(&self.state, cx)),
            )
    }
}
