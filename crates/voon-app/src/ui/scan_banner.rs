use super::theme::Theme;
use crate::{
    ui::SettingsPage,
    view_model::{AppState, ScanState},
};
use gpui::{App, Entity, IntoElement, div, prelude::*, rgb};

pub(super) fn scan_status(snapshot: &AppState) -> Option<String> {
    let operation = snapshot.scan_operation();
    match snapshot.scan_state() {
        ScanState::Idle => None,
        ScanState::Scanning {
            current_directory,
            discovered,
            ..
        } => Some(format!(
            "正在{} · 已发现 {discovered} 张专辑\n{}",
            operation.label(),
            current_directory.display()
        )),
        ScanState::Done {
            root,
            discovered,
            reads,
        } => Some(
            if operation != crate::view_model::scanning::ScanOperation::Incremental {
                format!(
                    "本轮{}结束 · 实际读取 {reads} 个音频文件；读取状态见专辑详情和待处理",
                    operation.label()
                )
            } else if snapshot
                .attention_items()
                .iter()
                .any(|item| item.stage.is_some() && item.path.starts_with(root))
            {
                format!(
                    "本次读取不完整 · 已保留完整资料 · {discovered} 张专辑 · 读取 {reads} 个音频文件；请查看待处理"
                )
            } else {
                format!("扫描完成 · {discovered} 张专辑 · 读取 {reads} 个音频文件")
            },
        ),
        ScanState::Cancelled { discovered, .. } => Some(format!(
            "{}已取消 · 本次发现 {discovered} 张专辑 · 已接受的完整资料已保留",
            operation.label()
        )),
        ScanState::Missing { root } => Some(format!("目录未连接：{}", root.display())),
        ScanState::Empty { root } => Some(format!(
            "本次未发现专辑，已保留上次索引。请确认音乐库已挂载。\n{}",
            root.display()
        )),
    }
}

/// Browsing shows task status; maintenance controls live in Settings.
pub fn scan_banner(state: &Entity<AppState>, theme: Theme, cx: &App) -> impl IntoElement {
    let snapshot = state.read(cx);
    let mut banner = div().flex().flex_col().text_sm();
    if !snapshot.offline_roots().is_empty() {
        let settings = state.clone();
        banner = banner.child(
            div()
                .px_4()
                .py_2()
                .flex()
                .items_center()
                .gap_3()
                .bg(rgb(theme.well))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child("音乐目录未连接 · 正在使用本地索引"),
                )
                .child(
                    super::settings::button(
                        "offline-library-settings",
                        "管理音乐目录…",
                        true,
                        theme,
                    )
                    .on_click(move |_, _, cx| {
                        crate::application::open_settings_page(
                            settings.clone(),
                            SettingsPage::Directories,
                            cx,
                        )
                    }),
                ),
        );
    }
    if let Some(status) = scan_status(snapshot) {
        let settings = state.clone();
        banner = banner.child(
            div()
                .px_4()
                .py_2()
                .flex()
                .items_center()
                .gap_3()
                .bg(rgb(theme.well))
                .child(div().flex_1().min_w_0().child(status))
                .child(
                    super::settings::button("scan-settings", "扫描与读取…", true, theme).on_click(
                        move |_, _, cx| {
                            crate::application::open_settings_page(
                                settings.clone(),
                                SettingsPage::Scanning,
                                cx,
                            )
                        },
                    ),
                ),
        );
    }
    if let Some(error) = snapshot.error() {
        banner = banner.child(
            div()
                .px_4()
                .py_2()
                .text_color(rgb(theme.accent))
                .child(error.to_owned()),
        );
    }
    banner
}
