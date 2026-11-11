use crate::view_model::{AppState, ScanState};
use gpui::{App, Entity, IntoElement, ParentElement, Styled, div, prelude::*, rgb};

pub fn scan_banner(state: &Entity<AppState>, cx: &App) -> impl IntoElement {
    let snapshot = state.read(cx);
    let mut banner = div().flex().flex_col().text_sm();
    let status = match snapshot.scan_state() {
        ScanState::Idle => None,
        ScanState::Scanning {
            current_directory,
            discovered,
            ..
        } => Some(format!(
            "正在扫描 · 已发现 {discovered} 张专辑\n{}",
            current_directory.display()
        )),
        ScanState::Done {
            root,
            discovered,
            reads,
        } => Some(
            if snapshot
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
            "已取消 · 本次发现 {discovered} 张专辑 · 扫描结果已保留"
        )),
        ScanState::Missing { .. } => None,
        ScanState::Empty { root } => Some(format!(
            "本次未发现专辑，已保留上次索引。请确认音乐库已挂载。\n{}",
            root.display()
        )),
    };
    if !snapshot.offline_roots().is_empty() {
        let reconnect_state = state.clone();
        let paths = snapshot
            .offline_roots()
            .iter()
            .map(|root| root.display().to_string())
            .collect::<Vec<_>>()
            .join("、");
        banner = banner.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .px_4()
                .py_2()
                .bg(rgb(0xfff4dc))
                .text_color(rgb(0x8a5a16))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child("音乐库目录不存在或未挂载")
                        .child(div().text_xs().child(format!(
                            "{paths} · 正在使用本地索引，浏览、搜索、编辑照常可用。"
                        ))),
                )
                .child(
                    div()
                        .id("reconnect-library")
                        .cursor_pointer()
                        .child("重新连接")
                        .on_click(move |_, _, cx| {
                            reconnect_state
                                .update(cx, |state, cx| state.refresh_root_availability(cx))
                        }),
                ),
        );
    }
    if let Some(status) = status {
        let cancel_state = state.clone();
        let mut row = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .px_4()
            .py_2()
            .bg(rgb(0xf4f4f6))
            .child(div().flex_1().child(status));
        if matches!(snapshot.scan_state(), ScanState::Scanning { .. }) {
            row = row.child(
                div()
                    .id("cancel-scan")
                    .cursor_pointer()
                    .child("取消")
                    .on_click(move |_, _, cx| {
                        cancel_state.update(cx, |state, cx| state.cancel_scan(cx))
                    }),
            );
        }
        if matches!(snapshot.scan_state(), ScanState::Empty { .. }) {
            let keep = state.clone();
            let confirm = state.clone();
            row = row.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .id("keep-previous-scan")
                            .cursor_pointer()
                            .child("保留上次索引")
                            .on_click(move |_, _, cx| {
                                keep.update(cx, |state, cx| state.keep_previous_scan(cx))
                            }),
                    )
                    .child(
                        div()
                            .id("confirm-empty-scan")
                            .cursor_pointer()
                            .child("确认目录为空，更新本地索引")
                            .on_click(move |_, _, cx| {
                                confirm.update(cx, |state, cx| {
                                    if let Err(error) = state.confirm_empty_scan(cx) {
                                        state.report_error(error, cx);
                                    }
                                })
                            }),
                    ),
            );
        }
        banner = banner.child(row);
    }
    if let Some(error) = snapshot.error() {
        banner = banner.child(
            div()
                .px_4()
                .py_2()
                .bg(rgb(0xfdecea))
                .text_color(rgb(0xc0352b))
                .child(error.to_owned()),
        );
    }
    banner
}
