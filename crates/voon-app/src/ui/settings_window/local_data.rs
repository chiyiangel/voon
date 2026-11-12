use gpui::{Context, Div, div, prelude::*, px, relative, rgb};

use super::SettingsWindow;
use crate::ui::{
    settings::{button, caption, card, choose_new_index},
    theme::Theme,
};

impl SettingsWindow {
    pub(super) fn local_data(&self, theme: Theme, cx: &Context<Self>) -> Div {
        let state = self.state.read(cx);
        let blocked = state.new_index_blocked_reason();
        let path = state
            .index_path()
            .or_else(|| state.index_open_problem()?.path.as_deref());
        let mut panel = div()
            .flex_none()
            .flex()
            .flex_col()
            .gap_4()
            .children(state.index_open_problem().map(|problem| {
                card(theme)
                    .child("本地资料库暂不可用")
                    .child(caption(problem.message.clone(), theme))
            }))
            .child(
                card(theme)
                    .child("本地资料库")
                    .child(caption(
                        path.map(|path| path.display().to_string())
                            .unwrap_or_else(|| "当前为临时内存索引".into()),
                        theme,
                    ))
                    .child(caption("保留当前资料库，在新位置创建空索引。", theme))
                    .child(
                        div().flex().child(
                            button(
                                "create-separate-index",
                                "新建索引…",
                                blocked.is_none(),
                                theme,
                            )
                            .when(blocked.is_none(), |button| {
                                button.on_click(cx.listener(|this, _, _, cx| {
                                    this.confirm_new_index = true;
                                    cx.notify();
                                }))
                            }),
                        ),
                    )
                    .children(blocked.map(|reason| caption(reason, theme))),
            );
        if self.confirm_new_index {
            let create = self.state.clone();
            panel = panel.child(card(theme).border_color(rgb(theme.accent)).child("新建本地资料库？")
                .child(caption("选择新索引的保存位置后，将切换到空资料库。当前资料库保留在原位置；音乐目录、扫描结果和整理内容不会自动复制。", theme))
                .child(div().flex().flex_wrap().gap_2()
                    .child(button("cancel-new-index", "取消", true, theme).on_click(cx.listener(|this, _, _, cx| { this.confirm_new_index = false; cx.notify(); })))
                    .child(button("confirm-new-index", "选择保存位置…", blocked.is_none(), theme)
                        .when(blocked.is_none(), |button| button.on_click(cx.listener(move |this, _, _, cx| {
                            this.confirm_new_index = false;
                            choose_new_index(create.clone(), cx);
                            cx.notify();
                        }))))));
        }
        let progress = state.cover_cache_progress();
        let total = progress.total.saturating_sub(progress.no_cover);
        let retry = self.state.clone();
        panel
            .child(
                card(theme)
                    .child("封面缓存")
                    .child(format!("{} / {} 张封面已就绪", progress.ready, total))
                    .child(
                        div()
                            .h(px(4.))
                            .rounded_full()
                            .bg(rgb(theme.selected))
                            .child(div().h_full().rounded_full().bg(rgb(theme.accent)).w(
                                relative(if total == 0 {
                                    0.
                                } else {
                                    progress.ready as f32 / total as f32
                                }),
                            )),
                    )
                    .child(caption(
                        format!(
                            "待缓存 {} · 处理中 {} · 失败 {} · 无封面 {}",
                            progress.pending, progress.loading, progress.failed, progress.no_cover
                        ),
                        theme,
                    ))
                    .children(
                        state
                            .cover_thumbnail_directory()
                            .map(|path| caption(format!("缓存位置：{}", path.display()), theme)),
                    )
                    .child(
                        div().flex().child(
                            button(
                                "retry-failed-covers",
                                "重试失败封面",
                                progress.failed > 0,
                                theme,
                            )
                            .when(progress.failed > 0, |button| {
                                button.on_click(move |_, _, cx| {
                                    retry.update(cx, |state, cx| state.retry_failed_covers(cx))
                                })
                            }),
                        ),
                    ),
            )
            .child(caption(
                "索引保存扫描结果、字段编辑、标签和手动合集；封面缓存用于本地浏览。",
                theme,
            ))
    }
}
