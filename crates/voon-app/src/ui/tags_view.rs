//! User tag and Manual collection controls forward all mutations to state.

use std::path::Path;

use gpui::{
    App, Context, Entity, Focusable, FontWeight, MouseButton, Render, Subscription, Window,
    deferred, div, prelude::*, px, rgb, rgba,
};

use super::{
    text_input::{TextChanged, TextInput},
    theme::Theme,
};
use crate::view_model::{
    AppState, Navigation,
    organization::{OrganizationDialog as Dialog, OrganizationScope as Scope},
};

pub struct TagsView {
    state: Entity<AppState>,
    input: Entity<TextInput>,
    last_dialog: Option<Dialog>,
    _subscriptions: Vec<Subscription>,
}

impl TagsView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextInput::new("输入名称", cx));
        let draft_state = state.clone();
        let subscriptions = vec![
            cx.subscribe(&input, move |_, _, event: &TextChanged, cx| {
                draft_state.update(cx, |state, cx| {
                    state.set_organization_draft(event.text.clone(), cx)
                })
            }),
            cx.observe(&state, |this, state, cx| {
                let value = state.read(cx).organization_draft().to_string();
                this.input.update(cx, |input, cx| input.set_text(value, cx));
                cx.notify();
            }),
        ];
        Self {
            state,
            input,
            last_dialog: None,
            _subscriptions: subscriptions,
        }
    }
}

fn open(state: &Entity<AppState>, dialog: Dialog, cx: &mut App) {
    state.update(cx, |state, cx| {
        if let Err(error) = state.open_organization_dialog(dialog, cx) {
            state.report_error(error, cx);
        }
    });
}

fn button(
    id: &'static str,
    label: impl Into<gpui::SharedString>,
    state: Entity<AppState>,
    dialog: Dialog,
    theme: Theme,
) -> impl IntoElement {
    let label: gpui::SharedString = label.into();
    div()
        .id(id)
        .cursor_pointer()
        .text_size(px(11.5))
        .text_color(rgb(theme.accent))
        .child(label)
        .on_click(move |_, _, cx| open(&state, dialog, cx))
}

pub fn navigation_tags(
    state: &AppState,
    entity: &Entity<AppState>,
    theme: Theme,
    manual: bool,
) -> impl IntoElement {
    let mut rows = div()
        .id(if manual {
            "manual-collection-navigation"
        } else {
            "user-tag-navigation"
        })
        .max_h(px(128.))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(px(2.))
        .pl(px(20.));
    let entries: Vec<_> = if manual {
        state
            .manual_collections()
            .iter()
            .map(|collection| {
                (
                    Scope::ManualCollection(collection.id),
                    collection.name.clone(),
                )
            })
            .collect()
    } else {
        state
            .user_tag_catalog()
            .iter()
            .map(|tag| (Scope::Tag(tag.id), tag.name.clone()))
            .collect()
    };
    for (scope, name) in entries {
        let entity = entity.clone();
        let active = state.organization_scope() == Some(scope)
            && state.navigation()
                == if manual {
                    Navigation::ManualCollections
                } else {
                    Navigation::UserTags
                };
        rows = rows.child(
            div()
                .id(gpui::SharedString::from(format!(
                    "organization-nav-{scope:?}"
                )))
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .px_2()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .bg(rgb(if active {
                    theme.selected
                } else {
                    theme.sidebar
                }))
                .text_size(px(11.5))
                .child(div().min_w_0().truncate().child(name))
                .child(
                    div()
                        .text_color(rgb(theme.secondary))
                        .child(state.organization_album_count(scope).to_string()),
                )
                .on_click(move |_, _, cx| {
                    entity.update(cx, |state, cx| {
                        if let Err(error) = state.select_organization(Some(scope), cx) {
                            state.report_error(error, cx);
                        }
                    })
                }),
        );
    }
    rows.child(button(
        if manual {
            "new-manual-collection"
        } else {
            "new-user-tag"
        },
        if manual {
            "+ 新建手动合集…"
        } else {
            "+ 新建标签…"
        },
        entity.clone(),
        if manual {
            Dialog::NewCollection
        } else {
            Dialog::NewTag
        },
        theme,
    ))
}

pub fn organization_toolbar(entity: &Entity<AppState>, theme: Theme, cx: &App) -> impl IntoElement {
    let state = entity.read(cx);
    let count = state.selected_album_paths().len();
    let navigation = state.navigation();
    if count == 0
        && !matches!(
            navigation,
            Navigation::UserTags | Navigation::ManualCollections
        )
    {
        return div().into_any_element();
    }
    let mut panel = div()
        .flex()
        .flex_col()
        .gap_2()
        .px_4()
        .py_2()
        .border_b_1()
        .border_color(rgb(theme.line));
    if count > 0 {
        panel = panel.child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_3()
                .text_size(px(11.5))
                .child(format!("已选 {count} 张"))
                .child(button(
                    "batch-add-tag",
                    "添加标签…",
                    entity.clone(),
                    Dialog::AddTag,
                    theme,
                ))
                .child(button(
                    "batch-remove-tag",
                    "移除标签…",
                    entity.clone(),
                    Dialog::RemoveTag,
                    theme,
                ))
                .child(button(
                    "batch-add-manual",
                    "加入手动合集…",
                    entity.clone(),
                    Dialog::AddToCollection,
                    theme,
                )),
        );
    }
    let active = state.organization_scope().filter(|scope| {
        matches!(
            (navigation, scope),
            (Navigation::UserTags, Scope::Tag(_))
                | (Navigation::ManualCollections, Scope::ManualCollection(_))
        )
    });
    if let Some(scope) = active {
        let clear_scope = entity.clone();
        panel = panel.child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(
                    div().font_weight(FontWeight::MEDIUM).child(
                        state
                            .organization_name(scope)
                            .unwrap_or_default()
                            .to_string(),
                    ),
                )
                .child(button(
                    "rename-organization",
                    "重命名…",
                    entity.clone(),
                    Dialog::Rename(scope),
                    theme,
                ))
                .child(button(
                    "delete-organization",
                    "删除…",
                    entity.clone(),
                    Dialog::Delete(scope),
                    theme,
                ))
                .child(
                    div()
                        .id("clear-organization-scope")
                        .text_size(px(11.5))
                        .cursor_pointer()
                        .text_color(rgb(theme.accent))
                        .child("显示全部")
                        .on_click(move |_, _, cx| {
                            clear_scope.update(cx, |state, cx| {
                                if let Err(error) = state.select_organization(None, cx) {
                                    state.report_error(error, cx);
                                }
                            })
                        }),
                ),
        );
        if let Scope::ManualCollection(collection) = scope {
            let mut controls = div().flex().gap_3().text_size(px(11.5));
            if let Some(album) = state.selected_album() {
                for (id, label, offset) in [
                    ("manual-member-up", "当前专辑上移", -1),
                    ("manual-member-down", "当前专辑下移", 1),
                ] {
                    let entity = entity.clone();
                    let path = album.path.clone();
                    controls = controls.child(
                        div()
                            .id(id)
                            .cursor_pointer()
                            .text_color(rgb(theme.accent))
                            .child(label)
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |state, cx| {
                                    if let Err(error) =
                                        state.move_manual_album(collection, &path, offset, cx)
                                    {
                                        state.report_error(error, cx);
                                    }
                                })
                            }),
                    );
                }
            }
            if count > 0 {
                let entity = entity.clone();
                controls = controls.child(
                    div()
                        .id("remove-manual-members")
                        .cursor_pointer()
                        .text_color(rgb(theme.accent))
                        .child("从合集中移除所选")
                        .on_click(move |_, _, cx| {
                            entity.update(cx, |state, cx| {
                                if let Err(error) =
                                    state.set_selected_manual_collection(collection, false, cx)
                                {
                                    state.report_error(error, cx);
                                }
                            })
                        }),
                );
            }
            panel = panel.child(controls);
        }
    } else {
        panel = panel.child(
            div()
                .text_sm()
                .text_color(rgb(theme.secondary))
                .child("在侧栏选择标签或手动合集；⌘ / Ctrl 点击可多选，Shift 点击可连续选择。"),
        );
    }
    panel.into_any_element()
}

pub fn album_organization(
    entity: &Entity<AppState>,
    path: &Path,
    theme: Theme,
    cx: &App,
) -> impl IntoElement {
    let state = entity.read(cx);
    let mut section = div()
        .flex()
        .flex_col()
        .gap_2()
        .px_4()
        .py_3()
        .border_b_1()
        .border_color(rgb(theme.line))
        .text_size(px(11.5));
    section = section.child(div().text_color(rgb(theme.secondary)).child("我的标签"));
    let mut tags = div().flex().flex_wrap().gap_2();
    if state.album_user_tags(path).is_empty() {
        tags = tags.child(div().text_color(rgb(theme.tertiary)).child("尚未添加标签"));
    }
    for tag in state.album_user_tags(path) {
        let entity = entity.clone();
        let path = path.to_path_buf();
        let name = tag.name.clone();
        tags = tags.child(
            div()
                .id(gpui::SharedString::from(format!("album-tag-{}", tag.id)))
                .cursor_pointer()
                .px_2()
                .py_1()
                .rounded_md()
                .bg(rgb(theme.well))
                .child(format!("{} ×", tag.name))
                .on_click(move |_, _, cx| {
                    entity.update(cx, |state, cx| {
                        if let Err(error) = state.set_tag_for_album(&path, &name, false, cx) {
                            state.report_error(error, cx);
                        }
                    })
                }),
        );
    }
    section = section
        .child(tags)
        .child(div().text_color(rgb(theme.secondary)).child("手动合集"));
    if state.album_manual_collections(path).is_empty() {
        section = section.child(
            div()
                .text_color(rgb(theme.tertiary))
                .child("尚未加入手动合集"),
        );
    }
    for collection in state.album_manual_collections(path) {
        let entity = entity.clone();
        let path = path.to_path_buf();
        let id = collection.id;
        section = section.child(
            div()
                .id(("album-manual-membership", id as usize))
                .cursor_pointer()
                .child(format!("{} · 移除", collection.name))
                .on_click(move |_, _, cx| {
                    entity.update(cx, |state, cx| {
                        if let Err(error) =
                            state.set_manual_collection_for_album(id, &path, false, cx)
                        {
                            state.report_error(error, cx);
                        }
                    })
                }),
        );
    }
    let mut actions = div().flex().gap_3().text_color(rgb(theme.accent));
    for (id, label, dialog) in [
        ("album-add-tag", "添加标签…", Dialog::AddTag),
        ("album-add-manual", "加入手动合集…", Dialog::AddToCollection),
    ] {
        let entity = entity.clone();
        let path = path.to_path_buf();
        actions = actions.child(div().id(id).cursor_pointer().child(label).on_click(
            move |_, _, cx| {
                entity.update(cx, |state, cx| {
                    state.select_album(Some(path.clone()), cx);
                    if let Err(error) = state.open_organization_dialog(dialog, cx) {
                        state.report_error(error, cx);
                    }
                })
            },
        ));
    }
    section.child(actions)
}

impl Render for TagsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let state = self.state.read(cx);
        let Some(dialog) = state.organization_dialog() else {
            self.last_dialog = None;
            return div().into_any_element();
        };
        if self.last_dialog != Some(dialog) {
            if !matches!(dialog, Dialog::Delete(_)) {
                window.focus(&self.input.read(cx).focus_handle(cx));
            }
            self.last_dialog = Some(dialog);
        }
        let title = match dialog {
            Dialog::NewTag => "新建标签",
            Dialog::AddTag => "为所选专辑添加标签",
            Dialog::RemoveTag => "从所选专辑移除标签",
            Dialog::NewCollection => "新建手动合集",
            Dialog::AddToCollection => "加入手动合集",
            Dialog::Rename(_) => "重命名",
            Dialog::Delete(_) => "删除本机分类",
        };
        let mut sheet = div()
            .id("organization-sheet")
            .w(px(420.))
            .max_h(px(560.))
            .p_5()
            .rounded_lg()
            .bg(rgb(theme.window))
            .border_1()
            .border_color(rgb(theme.line))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_3()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            );
        if let Dialog::Delete(scope) = dialog {
            sheet = sheet.child(format!(
                "删除“{}”？只删除本机分类及关联，不会删除音乐文件。",
                state.organization_name(scope).unwrap_or_default()
            ));
        } else {
            sheet = sheet.child(
                div()
                    .h(px(30.))
                    .px_2()
                    .flex()
                    .items_center()
                    .border_1()
                    .border_color(rgb(theme.line))
                    .rounded_md()
                    .child(self.input.clone()),
            );
        }
        if dialog == Dialog::NewCollection && !state.selected_album_paths().is_empty() {
            sheet = sheet.child(format!(
                "创建后加入已选的 {} 张专辑。",
                state.selected_album_paths().len()
            ));
        }
        if matches!(
            dialog,
            Dialog::AddTag | Dialog::RemoveTag | Dialog::AddToCollection
        ) {
            let choices: Vec<String> = if dialog == Dialog::AddToCollection {
                state
                    .manual_collections()
                    .iter()
                    .map(|collection| collection.name.clone())
                    .collect()
            } else {
                state
                    .user_tag_catalog()
                    .iter()
                    .map(|tag| tag.name.clone())
                    .collect()
            };
            let mut options = div()
                .id("organization-options")
                .max_h(px(260.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_1();
            for (position, name) in choices.into_iter().enumerate() {
                let entity = self.state.clone();
                options = options.child(
                    div()
                        .id(("organization-choice", position))
                        .cursor_pointer()
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .hover(move |style| style.bg(rgb(theme.well)))
                        .child(name.clone())
                        .on_click(move |_, _, cx| {
                            entity.update(cx, |state, cx| {
                                state.set_organization_draft(name.clone(), cx);
                                if let Err(error) = state.confirm_organization_dialog(cx) {
                                    state.report_error(error, cx);
                                }
                            })
                        }),
                );
            }
            sheet = sheet.child(options);
            if dialog == Dialog::AddToCollection {
                sheet = sheet.child(button(
                    "new-collection-from-picker",
                    "新建手动合集…",
                    self.state.clone(),
                    Dialog::NewCollection,
                    theme,
                ));
            }
        }
        if let Some(error) = state.error() {
            sheet = sheet.child(
                div()
                    .text_sm()
                    .text_color(rgb(0xc0352b))
                    .child(error.to_string()),
            );
        }
        let cancel = self.state.clone();
        let confirm = self.state.clone();
        sheet = sheet
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(theme.secondary))
                    .child("只保存在 Voon 本机索引，不写入音乐库。"),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_4()
                    .child(
                        div()
                            .id("cancel-organization-dialog")
                            .cursor_pointer()
                            .child("取消")
                            .on_click(move |_, _, cx| {
                                cancel.update(cx, |state, cx| state.close_organization_dialog(cx))
                            }),
                    )
                    .child(
                        div()
                            .id("confirm-organization-dialog")
                            .cursor_pointer()
                            .text_color(rgb(theme.accent))
                            .child(if matches!(dialog, Dialog::Delete(_)) {
                                "确认删除"
                            } else {
                                "确定"
                            })
                            .on_click(move |_, _, cx| {
                                confirm.update(cx, |state, cx| {
                                    if let Err(error) = state.confirm_organization_dialog(cx) {
                                        state.report_error(error, cx);
                                    }
                                })
                            }),
                    ),
            );
        deferred(
            div()
                .absolute()
                .inset_0()
                .bg(rgba(0x00000066))
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(sheet),
        )
        .with_priority(20)
        .into_any_element()
    }
}
