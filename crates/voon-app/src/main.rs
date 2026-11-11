//! Voon's GPUI shell. The real layout is being designed as an HTML prototype
//! first; for now the window only proves the wiring: it scans one Library
//! root with voon-core in the background and shows how many albums it found.
//!
//! Root: first command-line argument, else `VOON_ROOT`, else
//! `/Volumes/NAS1/Music`.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use gpui::{
    App, Application, Bounds, Context, SharedString, Timer, Window, WindowBounds, WindowOptions,
    div, prelude::*, px, rgb, size,
};
use voon_core::scan::{NoTags, scan_root};

const DEFAULT_ROOT: &str = "/Volumes/NAS1/Music";

enum Status {
    Scanning,
    Done { albums: usize, collections: usize },
    Missing,
}

struct VoonWindow {
    root: PathBuf,
    status: Status,
}

impl VoonWindow {
    fn new(root: PathBuf, cx: &mut Context<Self>) -> Self {
        let status = if root.is_dir() {
            let rx = start_scan(root.clone());
            cx.spawn(async move |this, cx| {
                loop {
                    Timer::after(Duration::from_millis(200)).await;
                    let Ok(result) = rx.try_recv() else {
                        continue;
                    };
                    let _ = this.update(cx, |this, cx| {
                        this.status = result;
                        cx.notify();
                    });
                    break;
                }
            })
            .detach();
            Status::Scanning
        } else {
            Status::Missing
        };
        Self { root, status }
    }
}

/// Structure-only scan on a plain thread; tags are left for the real UI.
fn start_scan(root: PathBuf) -> Receiver<Status> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let result = scan_root(&root, &mut NoTags);
        let _ = tx.send(Status::Done {
            albums: result.albums.len(),
            collections: result.collections.len(),
        });
    });
    rx
}

impl Render for VoonWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let line: SharedString = match &self.status {
            Status::Scanning => "正在扫描…".into(),
            Status::Done {
                albums,
                collections,
            } => format!("找到 {albums} 张专辑，{collections} 个合集").into(),
            Status::Missing => "音乐库目录不存在或未挂载".into(),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .bg(rgb(0xf7f7f5))
            .text_color(rgb(0x1f1f1f))
            .child(div().text_3xl().child("Voon"))
            .child(div().text_sm().child(self.root.display().to_string()))
            .child(div().text_lg().child(line))
    }
}

fn library_root() -> PathBuf {
    std::env::args()
        .nth(1)
        .or_else(|| std::env::var("VOON_ROOT").ok())
        .unwrap_or_else(|| DEFAULT_ROOT.to_string())
        .into()
}

fn main() {
    let root = library_root();
    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(720.0), px(480.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            move |window, cx| {
                window.set_window_title("Voon");
                cx.new(|cx| VoonWindow::new(root, cx))
            },
        )
        .expect("打开 Voon 窗口");
        cx.on_window_closed(|cx| cx.quit()).detach();
        cx.activate(true);
    });
}
