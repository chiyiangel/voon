//! Native GPUI entry point. Startup builds an empty local shell; adding Library
//! roots and starting background scans are separate application operations.

use gpui::{
    App, Application, Bounds, Focusable, TitlebarOptions, WindowBounds, WindowOptions, point,
    prelude::*, px, size,
};
use voon_app::{
    application::{self, APP_ID},
    platform::{index_location, library_root},
    ui::{Assets, VoonWindow},
    view_model::AppState,
};

fn main() {
    let suggested_root = library_root();
    let startup = match index_location() {
        Ok(location) => {
            AppState::open_index(location.path, location.selection_file, suggested_root)
        }
        Err(error) => AppState::unavailable_index(None, suggested_root, error),
    };
    let state = match startup {
        Ok(state) => state,
        Err(error) => {
            eprintln!("无法建立应用状态：{error}");
            return;
        }
    };
    Application::new()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
            let state = cx.new(|_| state);
            let window_state = state.clone();
            let main_window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        window_min_size: Some(size(px(1100.), px(650.))),
                        titlebar: Some(TitlebarOptions {
                            title: Some("音乐库".into()),
                            appears_transparent: true,
                            traffic_light_position: Some(point(px(18.), px(20.))),
                        }),
                        app_id: Some(APP_ID.into()),
                        ..Default::default()
                    },
                    move |window, cx| {
                        let view = cx.new(|cx| VoonWindow::new(window_state, cx));
                        view.focus_handle(cx).focus(window);
                        view
                    },
                )
                .expect("打开音乐库窗口");
            application::install(state, main_window.into(), cx);
            cx.activate(true);
        });
}
