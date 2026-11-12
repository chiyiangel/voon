use gpui::{Context, Div, div, prelude::*};

use super::{SettingsPage, SettingsWindow};
use crate::{
    ui::{
        scan_banner::scan_status,
        settings::{button, caption, card},
        theme::Theme,
    },
    view_model::ScanState,
};

impl SettingsWindow {
    pub(super) fn scanning(&self, theme: Theme, cx: &Context<Self>) -> Div {
        let state = self.state.read(cx);
        let blocked = state.scan_blocked_reason();
        let mut panel = div()
            .flex_none()
            .flex()
            .flex_col()
            .gap_4()
            .children(state.error().map(|error| caption(error.to_owned(), theme)))
            .children(blocked.map(|reason| caption(reason, theme)));
        if state.roots().is_empty() {
            panel = panel.child(
                card(theme)
                    .child("请先添加音乐目录")
                    .child(div().flex().child(self.page_link(
                        "scan-to-directories",
                        "管理音乐目录 ›",
                        SettingsPage::Directories,
                        theme,
                        cx,
                    ))),
            );
        }
        for (position, root) in state.roots().iter().enumerate() {
            let offline = state.offline_roots().contains(&root.path);
            let enabled = blocked.is_none() && !offline;
            let path = root.path.clone();
            let scan = self.state.clone();
            let reconnect = self.state.clone();
            panel = panel.child(
                card(theme)
                    .child(root.path.display().to_string())
                    .child(caption(
                        if offline {
                            "未连接 · 连接后才能扫描"
                        } else {
                            "已连接 · 读取新增、变更及上次失败的文件"
                        },
                        theme,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                button(
                                    ("settings-rescan-root", position),
                                    "重新扫描",
                                    enabled,
                                    theme,
                                )
                                .when(enabled, |button| {
                                    button.on_click(move |_, _, cx| {
                                        scan.update(cx, |state, cx| state.clear_settings_error(cx));
                                        crate::application::rescan_root(
                                            scan.clone(),
                                            path.clone(),
                                            cx,
                                        );
                                    })
                                }),
                            )
                            .when(offline, |actions| {
                                actions.child(
                                    button(
                                        ("scan-reconnect-root", position),
                                        "重新检测连接",
                                        true,
                                        theme,
                                    )
                                    .on_click(
                                        move |_, _, cx| {
                                            reconnect.update(cx, |state, cx| {
                                                state.refresh_root_availability(cx)
                                            })
                                        },
                                    ),
                                )
                            }),
                    ),
            );
        }
        let cancel = self.state.clone();
        let mut task = card(theme)
            .child("当前任务")
            .child(caption(
                scan_status(state).unwrap_or_else(|| "当前没有扫描任务".into()),
                theme,
            ))
            .when(
                matches!(state.scan_state(), ScanState::Scanning { .. }),
                |task| {
                    task.child(div().flex().child(
                        button("cancel-scan", "取消扫描", true, theme).on_click(move |_, _, cx| {
                            cancel.update(cx, |state, cx| state.cancel_scan(cx))
                        }),
                    ))
                    .child(caption("关闭设置窗口后，扫描仍会继续。", theme))
                },
            );
        if matches!(state.scan_state(), ScanState::Empty { .. }) {
            let keep = self.state.clone();
            let confirm = self.state.clone();
            task = task
                .child(caption(
                    "确认目录确实为空后，才会更新本地索引并隐藏旧专辑。",
                    theme,
                ))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            button("keep-previous-scan", "保留上次索引", true, theme).on_click(
                                move |_, _, cx| {
                                    keep.update(cx, |state, cx| state.keep_previous_scan(cx))
                                },
                            ),
                        )
                        .child(
                            button("confirm-empty-scan", "确认目录为空，更新索引", true, theme)
                                .on_click(move |_, _, cx| {
                                    confirm.update(cx, |state, cx| {
                                        state.clear_settings_error(cx);
                                        if let Err(error) = state.confirm_empty_scan(cx) {
                                            state.report_settings_error(error, cx);
                                        }
                                    })
                                }),
                        ),
                );
        }
        panel = panel.child(task);
        let failures: Vec<_> = state
            .attention_items()
            .iter()
            .filter(|item| item.can_retry_read())
            .collect();
        let mut issues = card(theme).child(format!("读取问题 · {} 项可重试", failures.len()));
        if failures.is_empty() {
            issues = issues.child(caption("当前没有需要重试的读取问题。", theme));
        }
        for (position, item) in failures.into_iter().enumerate() {
            let retry = self.state.clone();
            let path = item.path.clone();
            let kind = item.kind.clone();
            let offline = state
                .offline_roots()
                .iter()
                .any(|root| path.starts_with(root));
            let enabled = blocked.is_none() && !offline;
            issues = issues.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(item.title.clone())
                    .child(caption(item.path.display().to_string(), theme))
                    .child(caption(item.explanation.clone(), theme))
                    .when(offline, |row| {
                        row.child(caption("相关音乐目录未连接", theme))
                    })
                    .child(
                        div().flex().child(
                            button(
                                ("settings-retry-read", position),
                                "重试读取",
                                enabled,
                                theme,
                            )
                            .when(enabled, |button| {
                                button.on_click(move |_, _, cx| {
                                    retry.update(cx, |state, cx| {
                                        state.clear_settings_error(cx);
                                        if let Err(error) = state.retry_attention(&path, &kind, cx)
                                        {
                                            state.report_settings_error(error, cx);
                                        }
                                    })
                                })
                            }),
                        ),
                    ),
            );
        }
        panel = panel.child(issues);
        let mut reread = card(theme).child("强制重新读取标签");
        if let Some(album) = state.selected_album() {
            let offline = state.album_detail().is_none_or(|album| {
                album.tracks().any(|track| {
                    state
                        .offline_roots()
                        .iter()
                        .any(|root| track.source.audio_path().starts_with(root))
                })
            });
            let enabled = blocked.is_none() && !offline;
            let force = self.state.clone();
            reread = reread
                .child(format!("所选专辑：{}", album.display_title()))
                .child(caption(album.path.display().to_string(), theme))
                .when(offline, |row| {
                    row.child(caption("专辑尚未就绪或相关音乐目录未连接。", theme))
                })
                .children(
                    state
                        .album_tag_refresh_notice()
                        .map(|notice| caption(notice, theme)),
                )
                .child(div().flex().child(
                    button("force-album-tags", "强制重读所选专辑标签", enabled, theme).when(
                        enabled,
                        |button| {
                            button.on_click(move |_, _, cx| {
                                force.update(cx, |state, cx| {
                                    state.clear_settings_error(cx);
                                    if let Err(error) = state.force_selected_album_tags(cx) {
                                        state.report_settings_error(error, cx);
                                    }
                                })
                            })
                        },
                    ),
                ));
        } else {
            reread = reread.child(caption(
                "先在音乐库选择专辑，再回到这里重读其音频标签。",
                theme,
            ));
        }
        panel.child(reread.child(caption(
            "手动编辑优先保留，不会被重新读取的标签覆盖。",
            theme,
        )))
    }
}
