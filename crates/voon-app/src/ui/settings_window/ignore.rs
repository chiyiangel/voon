use gpui::{Context, Div, SharedString, div, prelude::*};

use super::{SettingsPage, SettingsWindow};
use crate::ui::{
    settings::{button, caption, card, choose_ignored_folder},
    theme::Theme,
};

impl SettingsWindow {
    pub(super) fn ignore(&self, theme: Theme, cx: &Context<Self>) -> Div {
        let state = self.state.read(cx);
        let mut panel = div().flex_none().flex().flex_col().gap_4();
        if state.roots().is_empty() {
            panel = panel.child(
                card(theme)
                    .child("请先添加音乐目录")
                    .child(div().flex().child(self.page_link(
                        "ignore-to-directories",
                        "管理音乐目录 ›",
                        SettingsPage::Directories,
                        theme,
                        cx,
                    ))),
            );
        }
        for (position, root) in state.roots().iter().enumerate() {
            let offline = state.offline_roots().contains(&root.path);
            let path = root.path.clone();
            let add = self.state.clone();
            let mut rules = card(theme)
                .child(root.path.display().to_string())
                .child(caption("忽略的子文件夹", theme));
            match state.ignored_paths(&root.path) {
                Ok(paths) if paths.is_empty() => rules = rules.child(caption("尚未添加", theme)),
                Ok(paths) => {
                    for (rule_position, path) in paths.into_iter().enumerate() {
                        let remove = self.state.clone();
                        let root_path = root.path.clone();
                        rules = rules.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_3()
                                .child(
                                    caption(
                                        path.strip_prefix(&root.path)
                                            .unwrap_or(&path)
                                            .display()
                                            .to_string(),
                                        theme,
                                    )
                                    .flex_1()
                                    .min_w_0(),
                                )
                                .child(
                                    button(
                                        SharedString::from(format!(
                                            "remove-ignore-rule-{position}-{rule_position}"
                                        )),
                                        "移除规则",
                                        true,
                                        theme,
                                    )
                                    .on_click(
                                        move |_, _, cx| {
                                            remove.update(cx, |state, cx| {
                                                state.clear_settings_error(cx);
                                                if let Err(error) =
                                                    state.remove_ignored_path(&root_path, &path, cx)
                                                {
                                                    state.report_settings_error(error, cx);
                                                }
                                            })
                                        },
                                    ),
                                ),
                        );
                    }
                }
                Err(error) => rules = rules.child(caption(format!("无法读取规则：{error}"), theme)),
            }
            panel = panel.child(
                rules
                    .child(
                        div().flex().child(
                            button(
                                ("add-ignore-rule", position),
                                "添加子文件夹…",
                                !offline,
                                theme,
                            )
                            .when(!offline, |button| {
                                button.on_click(move |_, _, cx| {
                                    choose_ignored_folder(add.clone(), path.clone(), cx)
                                })
                            }),
                        ),
                    )
                    .when(offline, |card| {
                        card.child(caption(
                            "目录未连接；连接后可添加规则，已保存的规则仍可移除。",
                            theme,
                        ))
                    }),
            );
        }
        panel.child(caption("添加或移除规则后，下次重新扫描生效。当前扫描沿用启动时的规则。", theme))
            .child(card(theme).child("内置规则 · 只读")
                .child(button("toggle-built-in-rules", if self.built_in_rules_open { "收起系统、隐藏文件与回收站规则" } else { "查看系统、隐藏文件与回收站规则" }, true, theme)
                    .on_click(cx.listener(|this, _, _, cx| { this.built_in_rules_open = !this.built_in_rules_open; cx.notify(); })))
                .when(self.built_in_rules_open, |card| card
                    .child(caption(".DS_Store、._* 及所有 .、@、# 开头的文件和文件夹（含 @eaDir、#recycle）", theme))
                    .child(caption("$RECYCLE.BIN、System Volume Information、Thumbs.db、desktop.ini", theme)))
                .child(caption("非音乐文件只识别封面；忽略 .log、.pdf、.txt、.m4v、.url 等文件。", theme)))
    }
}
