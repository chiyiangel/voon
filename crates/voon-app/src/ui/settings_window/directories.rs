use gpui::{Context, Div, div, prelude::*, rgb};

use super::{SettingsPage, SettingsWindow};
use crate::ui::{
    icons::icon,
    settings::{button, caption, card, choose_root},
    theme::Theme,
};

impl SettingsWindow {
    pub(super) fn directories(&self, theme: Theme, cx: &Context<Self>) -> Div {
        let state = self.state.read(cx);
        let blocked = state.root_changes_blocked_reason();
        let add = self.state.clone();
        let mut panel = div()
            .flex_none()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(format!("已添加的目录 · {}", state.roots().len()))
                    .child(
                        button("add-library-root", "＋ 添加目录…", blocked.is_none(), theme)
                            .when(blocked.is_none(), |button| {
                                button.on_click(move |_, _, cx| choose_root(add.clone(), cx))
                            }),
                    ),
            )
            .children(blocked.map(|reason| caption(reason, theme)));
        if state.roots().is_empty() {
            let suggested = state.suggested_root().to_path_buf();
            let add = self.state.clone();
            panel = panel.child(
                card(theme)
                    .child("尚未添加音乐目录")
                    .child(caption(format!("建议目录：{}", suggested.display()), theme))
                    .child(
                        div().flex().child(
                            button(
                                "use-suggested-root",
                                "使用建议目录并开始扫描",
                                blocked.is_none(),
                                theme,
                            )
                            .when(blocked.is_none(), |button| {
                                button.on_click(move |_, _, cx| {
                                    add.update(cx, |state, cx| {
                                        state.clear_settings_error(cx);
                                        if let Err(error) = state.add_root(suggested.clone(), cx) {
                                            state.report_settings_error(error, cx);
                                        }
                                    });
                                })
                            }),
                        ),
                    ),
            );
        }
        for (position, root) in state.roots().iter().enumerate() {
            let offline = state.offline_roots().contains(&root.path);
            let path = root.path.clone();
            let remove = self.state.clone();
            let reconnect = self.state.clone();
            let name = path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned();
            panel = panel.child(
                card(theme)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(icon("folder", 26., theme.accent))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(name)
                                    .child(caption(path.display().to_string(), theme)),
                            )
                            .child(caption(if offline { "未连接" } else { "已连接" }, theme)),
                    )
                    .when(offline, |row| {
                        row.child(caption(
                            "目录离线，仍可浏览本地索引；连接音乐目录后可重新检测。",
                            theme,
                        ))
                    })
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .when(offline, |actions| {
                                actions.child(
                                    button(
                                        ("reconnect-root", position),
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
                            })
                            .child(
                                button(
                                    ("remove-root", position),
                                    "移除目录…",
                                    blocked.is_none(),
                                    theme,
                                )
                                .when(
                                    blocked.is_none(),
                                    |button| {
                                        button.on_click(move |_, _, cx| {
                                            remove.update(cx, |state, cx| {
                                                state.clear_settings_error(cx);
                                                state.request_root_removal(Some(path.clone()), cx);
                                            });
                                        })
                                    },
                                ),
                            ),
                    ),
            );
        }
        if let Some(path) = state.pending_root_removal() {
            let confirm = self.state.clone();
            let cancel = self.state.clone();
            panel = panel.child(card(theme).border_color(rgb(theme.accent))
                .child("移除音乐目录？").child(caption(path.display().to_string(), theme))
                .child(caption("相关专辑将从音乐库隐藏。字段编辑、标签和手动合集成员关系会保留；重新添加相同目录并扫描后恢复。不会删除音乐文件。", theme))
                .child(div().flex().gap_2()
                    .child(button("cancel-remove-root", "取消", true, theme).on_click(move |_, _, cx| cancel.update(cx, |state, cx| state.request_root_removal(None, cx))))
                    .child(button("confirm-remove-root", "确认移除", blocked.is_none(), theme)
                        .when(blocked.is_none(), |button| button.on_click(move |_, _, cx| confirm.update(cx, |state, cx| {
                            state.clear_settings_error(cx);
                            if let Err(error) = state.confirm_root_removal(cx) { state.report_settings_error(error, cx); }
                        }))))));
        }
        panel
            .child(
                card(theme).child("目录设置").child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .child(self.page_link(
                            "directories-to-scan",
                            "扫描与读取 ›",
                            SettingsPage::Scanning,
                            theme,
                            cx,
                        ))
                        .child(self.page_link(
                            "directories-to-ignore",
                            "管理忽略规则 ›",
                            SettingsPage::Ignore,
                            theme,
                            cx,
                        )),
                ),
            )
            .child(caption(
                "Voon 只读取音乐文件；扫描结果和你的整理保存在本机。",
                theme,
            ))
    }
}
