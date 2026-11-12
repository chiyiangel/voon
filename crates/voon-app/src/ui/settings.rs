use crate::view_model::AppState;
use gpui::{
    App, Div, ElementId, Entity, IntoElement, ParentElement, PathPromptOptions, Stateful, Styled,
    div, prelude::*, px, rgb,
};
use std::path::{Path, PathBuf};

pub fn index_unavailable(
    state: &Entity<AppState>,
    theme: super::theme::Theme,
    cx: &App,
) -> impl IntoElement {
    let snapshot = state.read(cx);
    let problem = snapshot.index_open_problem().expect("索引错误状态");
    let create = state.clone();
    div()
        .size_full()
        .p_6()
        .flex()
        .flex_col()
        .gap_4()
        .bg(rgb(theme.window))
        .text_color(rgb(theme.ink))
        .child(div().text_lg().child("本地资料库暂不可用"))
        .child(problem.message.clone())
        .children(problem.path.as_ref().map(|path| {
            div()
                .text_sm()
                .text_color(rgb(theme.secondary))
                .child(path.display().to_string())
        }))
        .children(
            snapshot
                .settings_error()
                .map(|error| div().text_color(rgb(theme.accent)).child(error.to_owned())),
        )
        .child(
            div()
                .id("create-index-after-error")
                .cursor_pointer()
                .text_color(rgb(theme.accent))
                .child("前往设置，新建索引…")
                .on_click(move |_, _, cx| {
                    crate::application::open_settings_page(
                        create.clone(),
                        super::SettingsPage::LocalData,
                        cx,
                    )
                }),
        )
}

pub(super) fn choose_new_index(state: Entity<AppState>, cx: &mut App) {
    if let Some(reason) = state.read(cx).new_index_blocked_reason() {
        state.update(cx, |state, cx| state.report_settings_error(reason, cx));
        return;
    }
    state.update(cx, |state, cx| state.clear_settings_error(cx));
    let snapshot = state.read(cx);
    let directory = snapshot
        .index_path()
        .or_else(|| snapshot.index_open_problem()?.path.as_deref())
        .and_then(Path::parent)
        .unwrap_or_else(|| Path::new("/"));
    let choice = cx.prompt_for_new_path(directory, Some("library-0002-new.sqlite3"));
    cx.spawn(async move |cx| match choice.await {
        Ok(Ok(Some(path))) => {
            let _ = state.update(cx, |state, cx| {
                if let Err(error) = state.create_new_index(path, cx) {
                    state.report_settings_error(format!("无法新建本地索引：{error}"), cx);
                }
            });
        }
        Ok(Ok(None)) => {}
        Ok(Err(error)) => {
            let _ = state.update(cx, |state, cx| {
                state.report_settings_error(format!("无法选择索引位置：{error}"), cx)
            });
        }
        Err(error) => {
            let _ = state.update(cx, |state, cx| {
                state.report_settings_error(format!("索引位置选择器已关闭：{error}"), cx)
            });
        }
    })
    .detach();
}

pub fn choose_root(state: Entity<AppState>, cx: &mut App) {
    if let Some(reason) = state.read(cx).root_changes_blocked_reason() {
        state.update(cx, |state, cx| state.report_settings_error(reason, cx));
        return;
    }
    state.update(cx, |state, cx| state.clear_settings_error(cx));
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
                        state.report_settings_error(format!("无法添加音乐目录：{error}"), cx);
                    }
                });
            }
        }
        Ok(Ok(None)) => {}
        Ok(Err(error)) => {
            let _ = state.update(cx, |state, cx| {
                state.report_settings_error(format!("无法选择文件夹：{error}"), cx)
            });
        }
        Err(error) => {
            let _ = state.update(cx, |state, cx| {
                state.report_settings_error(format!("文件夹选择器已关闭：{error}"), cx)
            });
        }
    })
    .detach();
}

pub(super) fn choose_ignored_folder(state: Entity<AppState>, root: PathBuf, cx: &mut App) {
    state.update(cx, |state, cx| state.clear_settings_error(cx));
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
                        state.report_settings_error(format!("无法保存忽略规则：{error}"), cx);
                    }
                });
            }
        }
        Ok(Ok(None)) => {}
        Ok(Err(error)) => {
            let _ = state.update(cx, |state, cx| {
                state.report_settings_error(format!("无法选择文件夹：{error}"), cx)
            });
        }
        Err(error) => {
            let _ = state.update(cx, |state, cx| {
                state.report_settings_error(format!("文件夹选择器已关闭：{error}"), cx)
            });
        }
    })
    .detach();
}

/// Shared settings controls use the current window palette, including dark mode.
pub(super) fn card(theme: super::theme::Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_none()
        .p_4()
        .gap_3()
        .border_1()
        .border_color(rgb(theme.line))
        .rounded_lg()
        .bg(rgb(theme.window))
}

pub(super) fn caption(text: impl Into<gpui::SharedString>, theme: super::theme::Theme) -> Div {
    div()
        .text_size(px(12.))
        .text_color(rgb(theme.secondary))
        .child(text.into())
}

pub(super) fn button(
    id: impl Into<ElementId>,
    label: impl Into<gpui::SharedString>,
    enabled: bool,
    theme: super::theme::Theme,
) -> Stateful<Div> {
    let id = id.into();
    div()
        .debug_selector(|| id.to_string())
        .id(id)
        .flex_none()
        .px_3()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(rgb(theme.line))
        .bg(rgb(theme.window))
        .text_size(px(12.))
        .text_color(rgb(if enabled {
            theme.accent
        } else {
            theme.tertiary
        }))
        .child(label.into())
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(move |style| style.bg(rgb(theme.well)))
        })
}
