//! Visible window menus for platforms without a native application menu.

use gpui::{
    Action, Context, Div, Entity, FocusHandle, KeyDownEvent, MouseButton, MouseDownEvent, Render,
    Stateful, Subscription, Window, deferred, div, prelude::*, px, rgb,
};

use super::theme::Theme;
use crate::{
    application::{CloseCurrentWindow, OpenSettings, Quit, RescanRoot},
    view_model::AppState,
};

const MENU_HEIGHT: f32 = 32.;

#[derive(Clone, Copy, PartialEq)]
enum Menu {
    Application,
    Library,
}

pub struct ApplicationMenuBar {
    state: Entity<AppState>,
    open: Option<Menu>,
    rescan_open: bool,
    focus: FocusHandle,
    previous_focus: Option<FocusHandle>,
    _subscription: Subscription,
}

impl ApplicationMenuBar {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&state, |_, _, cx| cx.notify());
        Self {
            state,
            open: None,
            rescan_open: false,
            focus: cx.focus_handle(),
            previous_focus: None,
            _subscription: subscription,
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
        self.rescan_open = false;
        cx.notify();
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = None;
        self.rescan_open = false;
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

    fn library_menu(&self, window: &Window, theme: Theme, cx: &mut Context<Self>) -> Div {
        let snapshot = self.state.read(cx);
        let blocked = snapshot.root_changes_blocked_reason();
        let mut menu = div().flex().items_start().gap_1().child(
            menu_panel("library-menu", theme).w(px(180.)).child(
                div()
                    .id("open-rescan-menu")
                    .px_3()
                    .py_2()
                    .flex()
                    .justify_between()
                    .rounded_sm()
                    .cursor_pointer()
                    .bg(rgb(if self.rescan_open {
                        theme.selected
                    } else {
                        theme.window
                    }))
                    .hover(move |style| style.bg(rgb(theme.well)))
                    .child("重新扫描")
                    .child("›")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.rescan_open = !this.rescan_open;
                        cx.notify();
                    })),
            ),
        );
        if self.rescan_open {
            let width = (window.viewport_size().width - px(268.)).min(px(420.));
            let mut roots = menu_panel("rescan-roots-menu", theme)
                .w(width)
                .max_h(window.viewport_size().height - px(MENU_HEIGHT + 16.))
                .overflow_y_scroll();
            if let Some(reason) = blocked {
                roots = roots.child(
                    div()
                        .px_3()
                        .py_2()
                        .text_color(rgb(theme.secondary))
                        .child(reason),
                );
            }
            if snapshot.roots().is_empty() {
                roots = roots.child(
                    div()
                        .px_3()
                        .py_2()
                        .text_color(rgb(theme.secondary))
                        .child("尚未添加音乐目录"),
                );
            }
            for (position, root) in snapshot.roots().iter().enumerate() {
                let offline = snapshot.offline_roots().contains(&root.path);
                let enabled = blocked.is_none() && !offline;
                let path = root.path.clone();
                roots = roots.child(
                    div()
                        .id(("menu-rescan-root", position))
                        .px_3()
                        .py_2()
                        .rounded_sm()
                        .text_color(rgb(if enabled { theme.ink } else { theme.tertiary }))
                        .child(root.path.display().to_string())
                        .when(offline, |row| {
                            row.child(div().text_sm().child("目录离线，暂不可扫描"))
                        })
                        .when(enabled, |row| {
                            row.cursor_pointer()
                                .hover(move |style| style.bg(rgb(theme.well)))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.close(window, cx);
                                    // The shared action rechecks the current roots and task state.
                                    window.dispatch_action(
                                        Box::new(RescanRoot { root: path.clone() }),
                                        cx,
                                    );
                                }))
                        }),
                );
            }
            menu = menu.child(roots);
        }
        menu
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
            (Menu::Library, "资料库", "open-library-menu"),
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
                        "menu-close-window",
                        "关闭当前窗口",
                        "Ctrl+W",
                        CloseCurrentWindow,
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
                Menu::Library => self.library_menu(window, theme, cx).into_any_element(),
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
