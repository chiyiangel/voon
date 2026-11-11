//! Native GPUI entry point. Startup builds an empty local shell; adding Library
//! roots and starting background scans are separate application operations.

use gpui::{
    App, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, point, prelude::*, px,
    size,
};
use voon_app::{
    platform::{index_path, library_root},
    ui::{Assets, VoonWindow},
    view_model::AppState,
};
use voon_core::db::Index;

fn main() {
    let suggested_root = library_root();
    let index = match index_path()
        .map_err(|error| error.to_string())
        .and_then(|path| Index::open(&path).map_err(|error| error.to_string()))
    {
        Ok(index) => index,
        Err(error) => {
            eprintln!("无法打开本地索引：{error}");
            return;
        }
    };
    Application::new()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
            let state = cx.new(|_| AppState::new(index, suggested_root).expect("读取音乐库状态"));
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(1100.), px(650.))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("音乐库".into()),
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(18.), px(20.))),
                    }),
                    ..Default::default()
                },
                move |_, cx| cx.new(|cx| VoonWindow::new(state, cx)),
            )
            .expect("打开音乐库窗口");
            cx.on_window_closed(|cx| cx.quit()).detach();
            cx.activate(true);
        });
}
