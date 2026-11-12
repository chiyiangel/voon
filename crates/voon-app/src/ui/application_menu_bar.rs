//! Visible window menus for platforms without a native application menu.

use gpui::{
    Action, Context, Div, FocusHandle, KeyDownEvent, MouseButton, MouseDownEvent, Render, Stateful,
    Window, deferred, div, prelude::*, px, rgb,
};

use super::theme::Theme;
use crate::application::{CloseCurrentWindow, OpenSettings, Quit};

const MENU_HEIGHT: f32 = 32.;

#[derive(Clone, Copy, PartialEq)]
enum Menu {
    Application,
    Window,
}

pub struct ApplicationMenuBar {
    open: Option<Menu>,
    focus: FocusHandle,
    previous_focus: Option<FocusHandle>,
}

impl ApplicationMenuBar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            open: None,
            focus: cx.focus_handle(),
            previous_focus: None,
        }
    }

    fn toggle(&mut self, menu: Menu, window: &mut Window, cx: &mut Context<Self>) {
        if self.open == Some(menu) {
            self.close(window, cx);
            return;
        }
        if self.open.is_none() {
            self.previous_focus = window.focused(cx);
            self.focus.focus(window);
        }
        self.open = Some(menu);
        cx.notify();
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = None;
        if let Some(focus) = self.previous_focus.take() {
            focus.focus(window);
        }
        cx.notify();
    }

    fn command<A: Action>(
        id: &'static str,
        label: &'static str,
        shortcut: &'static str,
        action: A,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id(id)
            .px_3()
            .py_2()
            .flex()
            .justify_between()
            .gap_6()
            .rounded_sm()
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(theme.well)))
            .child(label)
            .child(div().text_color(rgb(theme.secondary)).child(shortcut))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.close(window, cx);
                window.dispatch_action(action.boxed_clone(), cx);
            }))
    }
}

fn menu_panel(id: &'static str, theme: Theme) -> Stateful<Div> {
    div()
        .id(id)
        .p_1()
        .rounded_md()
        .border_1()
        .border_color(rgb(theme.line))
        .bg(rgb(theme.window))
        .shadow_lg()
        .occlude()
}

impl Render for ApplicationMenuBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let mut bar = div()
            .id("application-menu-bar")
            .relative()
            .h(px(MENU_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .px_2()
            .border_b_1()
            .border_color(rgb(theme.line))
            .bg(rgb(theme.toolbar))
            .text_color(rgb(theme.ink))
            .font_family(".SystemUIFont")
            .text_size(px(13.))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" && this.open.is_some() {
                    this.close(window, cx);
                    cx.stop_propagation();
                }
            }));
        for (menu, label, id) in [
            (Menu::Application, "Voon", "open-application-menu"),
            (Menu::Window, "窗口", "open-window-menu"),
        ] {
            bar = bar.child(
                div()
                    .id(id)
                    .h_full()
                    .px_3()
                    .flex()
                    .items_center()
                    .rounded_sm()
                    .cursor_pointer()
                    .bg(rgb(if self.open == Some(menu) {
                        theme.selected
                    } else {
                        theme.toolbar
                    }))
                    .hover(move |style| style.bg(rgb(theme.well)))
                    .child(label)
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.toggle(menu, window, cx)),
                    ),
            );
        }
        if let Some(open) = self.open {
            let contents = match open {
                Menu::Application => menu_panel("application-menu", theme)
                    .w(px(250.))
                    .child(Self::command(
                        "menu-open-settings",
                        "设置…",
                        "Ctrl+,",
                        OpenSettings,
                        theme,
                        cx,
                    ))
                    .child(Self::command(
                        "menu-quit",
                        "退出 Voon",
                        "Ctrl+Q",
                        Quit,
                        theme,
                        cx,
                    ))
                    .into_any_element(),
                Menu::Window => menu_panel("window-menu", theme)
                    .w(px(220.))
                    .child(Self::command(
                        "menu-close-window",
                        "关闭窗口",
                        "Ctrl+W",
                        CloseCurrentWindow,
                        theme,
                        cx,
                    ))
                    .into_any_element(),
            };
            bar = bar.child(
                deferred(
                    div()
                        .id("application-menu-popup")
                        .absolute()
                        .top(px(MENU_HEIGHT))
                        .left(px(if open == Menu::Application { 8. } else { 68. }))
                        .child(contents)
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_mouse_down_out(cx.listener(
                            |this, event: &MouseDownEvent, window, cx| {
                                // Menu headings handle clicks in the bar themselves.
                                if event.position.y > px(MENU_HEIGHT) {
                                    this.close(window, cx);
                                }
                            },
                        )),
                )
                .with_priority(3),
            );
        }
        bar
    }
}
