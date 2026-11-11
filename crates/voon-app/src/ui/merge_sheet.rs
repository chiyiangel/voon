use super::theme::Theme;
use crate::view_model::{AppState, Navigation, Result, structure::StructureDraft};
use gpui::{
    App, Context, Entity, FocusHandle, FontWeight, KeyBinding, MouseButton, Render, Subscription,
    Window, actions, deferred, div, prelude::*, px, relative, rgb, rgba,
};
use voon_core::db::TrackRef;

actions!(voon_structure, [CancelStructure]);

fn action(
    id: impl Into<gpui::SharedString>,
    label: impl Into<gpui::SharedString>,
    state: Entity<AppState>,
    callback: impl Fn(&mut AppState, &mut Context<AppState>) -> Result<()> + 'static,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.into())
        .cursor_pointer()
        .px_2()
        .py_1()
        .rounded_md()
        .child(label.into())
        .on_click(move |_, _, cx| {
            state.update(cx, |state, cx| {
                if let Err(error) = callback(state, cx) {
                    state.report_error(error, cx);
                }
            })
        })
}

pub fn toolbar(entity: &Entity<AppState>, theme: Theme, cx: &App) -> impl IntoElement {
    let state = entity.read(cx);
    let count = state.selected_album_paths().len();
    let attention = state.navigation() == Navigation::NeedsAttention;
    let mut bar = div()
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .text_size(px(11.5))
        .text_color(rgb(theme.accent));
    if attention {
        bar = bar.child(action(
            "choose-structure-albums",
            "在音乐库选择要合并或拆分的专辑",
            entity.clone(),
            |state, cx| {
                state.navigate(Navigation::AllAlbums, cx);
                Ok(())
            },
        ));
    }
    if count >= 2 {
        bar = bar.child(action(
            "begin-merge",
            "合并所选专辑…",
            entity.clone(),
            AppState::begin_merge,
        ));
    } else if count == 1 {
        bar = bar.child(action(
            "begin-split",
            "拆分专辑…",
            entity.clone(),
            AppState::begin_split,
        ));
    }
    if let Some(last) = state.last_structure_operation() {
        bar = bar.child(action(
            "undo-structure",
            format!("撤销上次{}…", last.kind),
            entity.clone(),
            AppState::begin_structure_undo,
        ));
    }
    bar
}

pub struct MergeSheet {
    state: Entity<AppState>,
    focus: FocusHandle,
    was_open: bool,
    _subscription: Subscription,
}

impl MergeSheet {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.bind_keys([KeyBinding::new(
            "escape",
            CancelStructure,
            Some("VoonStructure"),
        )]);
        let subscription = cx.observe(&state, |_, _, cx| cx.notify());
        Self {
            state,
            focus: cx.focus_handle(),
            was_open: false,
            _subscription: subscription,
        }
    }
}

impl Render for MergeSheet {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(draft) = self.state.read(cx).structure_draft().cloned() else {
            self.was_open = false;
            return div().into_any_element();
        };
        if !self.was_open {
            window.focus(&self.focus);
            self.was_open = true;
        }
        let theme = Theme::for_window(window);
        let mut contents = div()
            .id("structure-options")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .p_5();
        let title;
        match draft {
            StructureDraft::Merge {
                albums,
                primary,
                order,
            } => {
                title = "合并专辑";
                contents = contents.child(format!(
                    "{} 张专辑将合并为一张、共 {} 碟。请选择主信息源：",
                    albums.len(),
                    order.len()
                ));
                for (i, album) in albums.iter().enumerate() {
                    let path = album.path.clone();
                    contents = contents.child(
                        action(
                            format!("merge-primary-{i}"),
                            format!(
                                "{} {}",
                                if path == primary { "●" } else { "○" },
                                album.display_title()
                            ),
                            self.state.clone(),
                            move |state, cx| state.choose_merge_primary(path.clone(), cx),
                        )
                        .bg(rgb(if album.path == primary {
                            theme.selected
                        } else {
                            theme.well
                        })),
                    );
                }
                contents = contents.child(div().text_xs().text_color(rgb(theme.secondary)).child("主专辑的非空信息优先，其他专辑补缺；标签和手动合集取并集，封面优先主信息源。"))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("碟片顺序"));
                for (i, reference) in order.iter().enumerate() {
                    let album = albums
                        .iter()
                        .find(|album| album.path == reference.album)
                        .unwrap();
                    let disc = &album.discs[reference.disc];
                    contents = contents.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().flex_1().child(format!(
                                "{}. {} · 原碟 {} · {} 轨",
                                i + 1,
                                album.display_title(),
                                disc.number,
                                disc.tracks.len()
                            )))
                            .child(action(
                                format!("disc-up-{i}"),
                                "上移",
                                self.state.clone(),
                                move |state, cx| state.move_merge_disc(i, -1, cx),
                            ))
                            .child(action(
                                format!("disc-down-{i}"),
                                "下移",
                                self.state.clone(),
                                move |state, cx| state.move_merge_disc(i, 1, cx),
                            )),
                    );
                }
            }
            StructureDraft::Split {
                album,
                by_tracks,
                tracks,
            } => {
                title = "拆分专辑";
                contents = contents.child(album.display_title().to_string()).child(
                    div()
                        .flex()
                        .gap_3()
                        .child(action(
                            "split-by-disc",
                            if by_tracks {
                                "○ 按碟拆分"
                            } else {
                                "● 按碟拆分"
                            },
                            self.state.clone(),
                            |state, cx| state.set_split_by_tracks(false, cx),
                        ))
                        .child(action(
                            "split-by-tracks",
                            if by_tracks {
                                "● 按所选曲目拆分"
                            } else {
                                "○ 按所选曲目拆分"
                            },
                            self.state.clone(),
                            |state, cx| state.set_split_by_tracks(true, cx),
                        )),
                );
                if by_tracks {
                    contents =
                        contents.child(div().text_xs().text_color(rgb(theme.secondary)).child(
                            format!(
                                "已选 {} 轨。所选曲目成为一张专辑，其余曲目成为另一张。",
                                tracks.len()
                            ),
                        ));
                    for (disc_index, disc) in album.discs.iter().enumerate() {
                        contents = contents.child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(format!("碟 {}", disc.number)),
                        );
                        for (track_index, track) in disc.tracks.iter().enumerate() {
                            let reference = TrackRef {
                                disc: disc_index,
                                track: track_index,
                            };
                            let title = track
                                .title
                                .as_ref()
                                .map(|title| title.value.clone())
                                .unwrap_or_else(|| format!("曲目 {}", track_index + 1));
                            contents = contents.child(action(
                                format!("split-track-{disc_index}-{track_index}"),
                                format!(
                                    "{} {}",
                                    if tracks.contains(&reference) {
                                        "☑"
                                    } else {
                                        "☐"
                                    },
                                    title
                                ),
                                self.state.clone(),
                                move |state, cx| state.toggle_split_track(reference, cx),
                            ));
                        }
                    }
                } else {
                    contents = contents.child(format!(
                        "将按 {} 张碟生成独立专辑。至少需要两张碟。",
                        album.discs.len()
                    ));
                }
                contents = contents.child(div().text_xs().text_color(rgb(theme.secondary)).child(
                    "专辑信息、标签和手动合集会保留到拆分结果中。音频文件与 CUE 不会被切分或移动。",
                ));
            }
            StructureDraft::Undo(operation) => {
                title = "撤销分组操作";
                contents = contents.child(format!("撤销上次{}：{} 张结果专辑恢复到原来的 {} 张分组。", operation.kind, operation.outputs.len(), operation.inputs.len()))
                    .child("后续在结果上做的编辑会保留到原专辑。若不同结果有相冲突的新值，请先统一这些字段、封面或合集顺序，再撤销。");
            }
        }
        if let Some(error) = self.state.read(cx).error() {
            contents = contents.child(div().text_color(rgb(0xc0352b)).child(error.to_string()));
        }
        deferred(
            div()
                .absolute()
                .inset_0()
                .size_full()
                .occlude()
                .bg(rgba(0x00000066))
                .flex()
                .items_center()
                .justify_center()
                .track_focus(&self.focus)
                .key_context("VoonStructure")
                .on_action(cx.listener(|this, _: &CancelStructure, _, cx| {
                    this.state
                        .update(cx, |state, cx| state.cancel_structure(cx))
                }))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .w(px(680.))
                        .h(px(600.))
                        .max_w(relative(0.94))
                        .max_h(relative(0.94))
                        .rounded_lg()
                        .shadow_lg()
                        .bg(rgb(theme.window))
                        .text_color(rgb(theme.ink))
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .p_5()
                                .flex_none()
                                .border_b_1()
                                .border_color(rgb(theme.line))
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(title),
                        )
                        .child(contents)
                        .child(
                            div()
                                .p_4()
                                .flex_none()
                                .border_t_1()
                                .border_color(rgb(theme.line))
                                .flex()
                                .items_center()
                                .gap_3()
                                .child(
                                    div()
                                        .flex_1()
                                        .text_xs()
                                        .text_color(rgb(theme.secondary))
                                        .child("只更改 Voon 本地分组，可撤销，不修改音乐文件。"),
                                )
                                .child(action(
                                    "cancel-structure",
                                    "取消",
                                    self.state.clone(),
                                    |state, cx| {
                                        state.cancel_structure(cx);
                                        Ok(())
                                    },
                                ))
                                .child(
                                    action(
                                        "confirm-structure",
                                        "确认",
                                        self.state.clone(),
                                        AppState::confirm_structure,
                                    )
                                    .bg(rgb(theme.accent))
                                    .text_color(rgb(theme.window)),
                                ),
                        ),
                ),
        )
        .with_priority(25)
        .into_any_element()
    }
}
