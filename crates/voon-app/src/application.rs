//! Window lifetime and commands shared by menus, shortcuts and onboarding.

use std::path::PathBuf;

use gpui::{
    AnyWindowHandle, App, AppContext, Bounds, Entity, KeyBinding, TitlebarOptions, WindowBounds,
    WindowOptions, actions, px, size,
};

use crate::{
    ui::{SettingsPage, SettingsWindow},
    view_model::AppState,
};

#[cfg(target_os = "macos")]
mod native_menu;

pub const APP_ID: &str = "com.chiyiangel.voon";

actions!(voon, [OpenSettings, CloseCurrentWindow, Quit]);

#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = voon, no_json)]
pub struct RescanRoot {
    pub root: PathBuf,
}

#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = voon, no_json)]
pub struct OpenSettingsPage {
    pub page: SettingsPage,
}

pub fn install(state: Entity<AppState>, main_window: AnyWindowHandle, cx: &mut App) {
    #[cfg(target_os = "macos")]
    let modifier = "cmd";
    #[cfg(not(target_os = "macos"))]
    let modifier = "ctrl";
    cx.bind_keys([
        KeyBinding::new(&format!("{modifier}-,"), OpenSettings, None),
        KeyBinding::new(&format!("{modifier}-w"), CloseCurrentWindow, None),
        KeyBinding::new(&format!("{modifier}-q"), Quit, None),
    ]);
    let settings_state = state.clone();
    cx.on_action(move |_: &OpenSettings, cx| open_settings(settings_state.clone(), cx));
    let page_state = state.clone();
    cx.on_action(move |action: &OpenSettingsPage, cx| {
        open_settings_page(page_state.clone(), action.page, cx)
    });
    cx.on_action(|_: &CloseCurrentWindow, cx| close_current_window(cx));
    cx.on_action(|_: &Quit, cx| cx.quit());
    let rescan_state = state.clone();
    cx.on_action(move |action: &RescanRoot, cx| {
        rescan_root(rescan_state.clone(), action.root.clone(), cx)
    });
    #[cfg(target_os = "macos")]
    native_menu::install(cx);
    cx.on_window_closed(move |cx| {
        if !cx.windows().contains(&main_window) {
            cx.quit();
        }
    })
    .detach();
}

pub fn open_settings(state: Entity<AppState>, cx: &mut App) {
    open_settings_at(state, None, cx);
}

pub fn open_settings_page(state: Entity<AppState>, page: SettingsPage, cx: &mut App) {
    open_settings_at(state, Some(page), cx);
}

fn open_settings_at(state: Entity<AppState>, page: Option<SettingsPage>, cx: &mut App) {
    // An action's current window is temporarily borrowed out of App. Deferring
    // also serializes repeated opens before checking for an existing window.
    cx.defer(move |cx| {
        if let Some(handle) = cx
            .windows()
            .into_iter()
            .find_map(|handle| handle.downcast::<SettingsWindow>())
        {
            let _ = handle.update(cx, |settings, window, cx| {
                if let Some(page) = page {
                    settings.set_page(page, cx);
                }
                window.activate_window();
            });
            return;
        }
        let bounds = Bounds::centered(None, size(px(840.), px(700.)), cx);
        let error_state = state.clone();
        if let Err(error) = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(680.), px(480.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("设置".into()),
                    ..Default::default()
                }),
                app_id: Some(APP_ID.into()),
                ..Default::default()
            },
            move |window, cx| {
                cx.new(|cx| {
                    let mut settings = SettingsWindow::new(state, window, cx);
                    if let Some(page) = page {
                        settings.set_page(page, cx);
                    }
                    settings
                })
            },
        ) {
            error_state.update(cx, |state, cx| {
                state.report_error(format!("无法打开设置窗口：{error}"), cx)
            });
        }
    });
}

pub fn close_current_window(cx: &mut App) {
    let target = cx.active_window();
    cx.defer(move |cx| {
        if let Some(target) = target {
            let _ = target.update(cx, |_, window, _| window.remove_window());
        }
    });
}

pub fn rescan_root(state: Entity<AppState>, root: PathBuf, cx: &mut App) {
    cx.defer(move |cx| {
        state.update(cx, |state, cx| {
            let result = if let Some(reason) = state.scan_blocked_reason() {
                Err(reason.into())
            } else {
                state.start_scan(root, cx)
            };
            if let Err(error) = result {
                state.report_error(error, cx);
            }
        });
    });
}
