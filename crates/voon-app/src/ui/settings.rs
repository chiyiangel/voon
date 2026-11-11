use crate::view_model::AppState;
use gpui::{
    App, Entity, IntoElement, ParentElement, PathPromptOptions, Styled, div, prelude::*, px, rgb,
};
use std::path::{Path, PathBuf};

pub fn choose_root(state: Entity<AppState>, cx: &mut App) {
    let choice = cx.prompt_for_paths(PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some("选择音乐库文件夹".into()),
    });
    cx.spawn(async move |cx| match choice.await {
        Ok(Ok(Some(paths))) => {
            if let Some(path) = paths.into_iter().next() {
                let _ = state.update(cx, |state, cx| {
                    if let Err(error) = state.add_root(path, cx) {
                        state.report_error(error, cx);
                    }
                });
            }
        }
        Ok(Ok(None)) => {}
        Ok(Err(error)) => {
            let _ = state.update(cx, |state, cx| {
                state.report_error(format!("无法选择文件夹：{error}"), cx)
            });
        }
        Err(error) => {
            let _ = state.update(cx, |state, cx| {
                state.report_error(format!("文件夹选择器已关闭：{error}"), cx)
            });
        }
    })
    .detach();
}

fn choose_ignored_folder(state: Entity<AppState>, root: PathBuf, cx: &mut App) {
    let choice = cx.prompt_for_paths(PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some("选择此音乐库中要忽略的子文件夹".into()),
    });
    cx.spawn(async move |cx| match choice.await {
        Ok(Ok(Some(paths))) => {
            if let Some(path) = paths.into_iter().next() {
                let _ = state.update(cx, |state, cx| {
                    if let Err(error) = state.add_ignored_path(&root, path, cx) {
                        state.report_error(error, cx);
                    }
                });
            }
        }
        Ok(Ok(None)) => {}
        Ok(Err(error)) => {
            let _ = state.update(cx, |state, cx| {
                state.report_error(format!("无法选择文件夹：{error}"), cx)
            });
        }
        Err(error) => {
            let _ = state.update(cx, |state, cx| {
                state.report_error(format!("文件夹选择器已关闭：{error}"), cx)
            });
        }
    })
    .detach();
}

fn ignored_subfolders(
    state: &Entity<AppState>,
    root: &Path,
    position: usize,
    cx: &App,
) -> impl IntoElement {
    let add_state = state.clone();
    let add_root = root.to_path_buf();
    let mut section = div()
        .flex()
        .flex_col()
        .gap_2()
        .mt_2()
        .child(div().text_sm().child("忽略的子文件夹"));
    match state.read(cx).ignored_paths(root) {
        Ok(paths) if paths.is_empty() => {
            section = section.child(div().text_sm().text_color(rgb(0x6e6e73)).child("尚未添加"));
        }
        Ok(paths) => {
            for (rule_position, path) in paths.into_iter().enumerate() {
                let remove_state = state.clone();
                let remove_root = root.to_path_buf();
                section = section.child(
                    div()
                        .flex()
                        .justify_between()
                        .gap_4()
                        .child(div().text_sm().child(path.display().to_string()))
                        .child(
                            div()
                                .id(gpui::SharedString::from(format!(
                                    "remove-ignore-rule-{position}-{rule_position}"
                                )))
                                .cursor_pointer()
                                .text_sm()
                                .text_color(rgb(0xc0352b))
                                .child("移除规则")
                                .on_click(move |_, _, cx| {
                                    remove_state.update(cx, |state, cx| {
                                        if let Err(error) =
                                            state.remove_ignored_path(&remove_root, &path, cx)
                                        {
                                            state.report_error(error, cx);
                                        }
                                    })
                                }),
                        ),
                );
            }
        }
        Err(error) => {
            section = section.child(
                div()
                    .text_sm()
                    .text_color(rgb(0xc0352b))
                    .child(format!("无法读取忽略规则：{error}")),
            );
        }
    }
    section
        .child(
            div()
                .id(("add-ignore-rule", position))
                .cursor_pointer()
                .text_sm()
                .text_color(rgb(0xb4532a))
                .child("选择要忽略的子文件夹…")
                .on_click(move |_, _, cx| {
                    choose_ignored_folder(add_state.clone(), add_root.clone(), cx)
                }),
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x6e6e73))
                .child("添加或移除规则后，下次重新扫描生效。"),
        )
}

fn built_in_ignore_rules() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .border_1()
        .border_color(rgb(0xe2e2e6))
        .rounded_lg()
        .child(div().text_sm().child("内置忽略规则 · 只读"))
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x6e6e73))
                .child(".DS_Store、._* 及所有 .、@、# 开头的文件和文件夹（含 @eaDir、#recycle）"),
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x6e6e73))
                .child("$RECYCLE.BIN、System Volume Information、Thumbs.db、desktop.ini"),
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x6e6e73))
                .child("非音乐文件只识别封面：忽略 .log、.pdf、.txt、.m4v、.url 等文件。"),
        )
}

pub fn settings(state: &Entity<AppState>, cx: &App) -> impl IntoElement {
    let snapshot = state.read(cx);
    let roots = snapshot.roots().to_vec();
    let removal = snapshot.pending_root_removal().map(|p| p.to_path_buf());
    let close_state = state.clone();
    let add_state = state.clone();
    let mut panel = div()
        .id("library-settings")
        .flex()
        .flex_col()
        .gap_4()
        .p_6()
        .size_full()
        .overflow_y_scroll()
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(div().text_lg().child("设置 · 音乐库"))
                .child(
                    div()
                        .id("close-settings")
                        .cursor_pointer()
                        .child("完成")
                        .on_click(move |_, _, cx| {
                            close_state.update(cx, |state, cx| state.show_settings(false, cx))
                        }),
                ),
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x6e6e73))
                .child("Voon 只读取音乐库；扫描结果和你的整理保存在本机。"),
        );
    for (position, root) in roots.into_iter().enumerate() {
        let remove_state = state.clone();
        let scan_state = state.clone();
        let root_path = root.path.clone();
        let offline = snapshot.offline_roots().contains(&root.path);
        let rescan = div()
            .id(("rescan-root", position))
            .text_color(rgb(if offline { 0xa0a0a6 } else { 0xb4532a }))
            .child("增量重新扫描");
        let rescan = if offline {
            rescan
        } else {
            rescan.cursor_pointer().on_click(move |_, _, cx| {
                scan_state.update(cx, |state, cx| {
                    if let Err(error) = state.start_scan(root_path.clone(), cx) {
                        state.report_error(error, cx);
                    }
                })
            })
        };
        panel = panel.child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .border_1()
                .border_color(rgb(0xe2e2e6))
                .rounded_lg()
                .child(div().text_sm().child(root.path.display().to_string()))
                .child(ignored_subfolders(state, &root.path, position, cx))
                .child(
                    div().flex().gap_4().child(rescan).child(
                        div()
                            .id(("remove-root", position))
                            .cursor_pointer()
                            .text_color(rgb(0xc0352b))
                            .child("移除")
                            .on_click(move |_, _, cx| {
                                remove_state.update(cx, |state, cx| {
                                    state.request_root_removal(Some(root.path.clone()), cx)
                                })
                            }),
                    ),
                ),
        );
    }
    panel = panel.child(
        div()
            .id("add-library-root")
            .cursor_pointer()
            .px_4()
            .py_2()
            .rounded_lg()
            .bg(rgb(0xb4532a))
            .text_color(rgb(0xffffff))
            .child("添加根目录…")
            .on_click(move |_, _, cx| choose_root(add_state.clone(), cx)),
    );
    if let Some(root) = removal {
        let confirm_state = state.clone();
        let cancel_state = state.clone();
        panel = panel.child(div().p_4().border_1().border_color(rgb(0xe2e2e6)).rounded_lg().bg(rgb(0xfff4dc)).flex().flex_col().gap_3()
            .child("移除音乐库？")
            .child(div().text_sm().child(root.display().to_string()))
            .child(div().text_sm().child("专辑将从音乐库隐藏。字段编辑、标签和手动合集成员关系会保留；重新添加相同目录并扫描后恢复。不会删除音乐文件。"))
            .child(div().flex().gap_4()
                .child(div().id("confirm-remove-root").cursor_pointer().text_color(rgb(0xc0352b)).child("确认移除").on_click(move |_, _, cx| confirm_state.update(cx, |state, cx| { if let Err(error) = state.confirm_root_removal(cx) { state.report_error(error, cx); } })))
                .child(div().id("cancel-remove-root").cursor_pointer().child("取消").on_click(move |_, _, cx| cancel_state.update(cx, |state, cx| state.request_root_removal(None, cx))))));
    }
    panel.child(built_in_ignore_rules()).child(div().h(px(1.0)))
}
