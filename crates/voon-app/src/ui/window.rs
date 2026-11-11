use gpui::{Context, Entity, Render, Subscription, Timer, Window, div, prelude::*, px, rgb};

use super::{
    empty, scan_banner, settings, sidebar,
    theme::{DETAIL_WIDTH, Theme},
    toolbar,
};
use crate::view_model::AppState;

pub struct VoonWindow {
    state: Entity<AppState>,
    grid: Entity<super::album_grid::AlbumGrid>,
    list: Entity<super::album_list::AlbumList>,
    toolbar: Entity<toolbar::Toolbar>,
    detail: Entity<super::album_detail::AlbumDetail>,
    edit_sheet: Entity<super::edit_sheet::EditSheet>,
    cover_picker: Entity<super::cover_picker::CoverPicker>,
    organization: Entity<super::tags_view::TagsView>,
    structure: Entity<super::merge_sheet::MergeSheet>,
    _state_subscription: Subscription,
}

impl VoonWindow {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&state, |_, _, cx| cx.notify());
        let grid = cx.new(|cx| super::album_grid::AlbumGrid::new(state.clone(), cx));
        let covers = grid.read(cx).cover_cache();
        let list =
            cx.new(|cx| super::album_list::AlbumList::new(state.clone(), covers.clone(), cx));
        let toolbar = cx.new(|cx| toolbar::Toolbar::new(state.clone(), cx));
        let detail =
            cx.new(|cx| super::album_detail::AlbumDetail::new(state.clone(), covers.clone(), cx));
        let cover_picker =
            cx.new(|cx| super::cover_picker::CoverPicker::new(state.clone(), covers, cx));
        let edit_sheet = cx.new(|cx| super::edit_sheet::EditSheet::new(state.clone(), cx));
        let organization = cx.new(|cx| super::tags_view::TagsView::new(state.clone(), cx));
        let structure = cx.new(|cx| super::merge_sheet::MergeSheet::new(state.clone(), cx));
        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(std::time::Duration::from_millis(40)).await;
                if this
                    .update(cx, |this, cx| {
                        this.state.update(cx, |state, cx| state.poll_scan(cx))
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Self {
            state,
            grid,
            list,
            toolbar,
            detail,
            edit_sheet,
            cover_picker,
            organization,
            structure,
            _state_subscription: subscription,
        }
    }
}

impl Render for VoonWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let state = self.state.read(cx);
        let settings_state = self.state.clone();
        let choose_state = self.state.clone();
        let start_state = self.state.clone();
        let content = if state.settings_open() {
            settings::settings(&self.state, cx).into_any_element()
        } else if state.navigation() == crate::view_model::Navigation::NeedsAttention {
            super::inbox_view::render(&self.state, theme, cx).into_any_element()
        } else if state.roots().is_empty() {
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(div().flex_1().child(empty::albums(state, theme)))
                .child(
                    div()
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .items_center()
                        .child(state.suggested_root().display().to_string())
                        .child(
                            div()
                                .flex()
                                .gap_4()
                                .child(
                                    div()
                                        .id("choose-first-root")
                                        .cursor_pointer()
                                        .child("选择文件夹…")
                                        .on_click(move |_, _, cx| {
                                            settings::choose_root(choose_state.clone(), cx)
                                        }),
                                )
                                .child(
                                    div()
                                        .id("scan-default-root")
                                        .cursor_pointer()
                                        .px_3()
                                        .py_2()
                                        .rounded_md()
                                        .bg(rgb(theme.accent))
                                        .text_color(rgb(theme.window))
                                        .child("开始扫描")
                                        .on_click(move |_, _, cx| {
                                            start_state.update(cx, |state, cx| {
                                                if let Err(error) = state.add_root(
                                                    state.suggested_root().to_path_buf(),
                                                    cx,
                                                ) {
                                                    state.report_error(error, cx);
                                                }
                                            })
                                        }),
                                ),
                        ),
                )
                .into_any_element()
        } else {
            let browse = match state.view_mode() {
                crate::view_model::browse::ViewMode::Grid => self.grid.clone().into_any_element(),
                crate::view_model::browse::ViewMode::List => self.list.clone().into_any_element(),
            };
            div()
                .size_full()
                .flex()
                .when(
                    state.navigation() == crate::view_model::Navigation::Facets,
                    |content| content.child(super::facet_view::panel(state, &self.state, theme)),
                )
                .when(
                    state.navigation() == crate::view_model::Navigation::Composers,
                    |content| content.child(super::composer_view::panel(state, &self.state, theme)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .flex()
                        .flex_col()
                        .child(super::facet_view::chips(state, &self.state, theme))
                        .child(div().flex_1().min_h_0().child(browse)),
                )
                .into_any_element()
        };
        div()
            .relative()
            .size_full()
            .flex()
            .overflow_hidden()
            .bg(rgb(theme.window))
            .text_color(rgb(theme.ink))
            .font_family(".SystemUIFont")
            .text_size(px(13.))
            .child(sidebar::render(state, &self.state, theme))
            .child(
                div()
                    .h_full()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(self.toolbar.clone())
                    .child(
                        div().px_4().py_2().child(
                            div()
                                .id("open-settings")
                                .cursor_pointer()
                                .text_color(rgb(theme.accent))
                                .child("设置 · 音乐库")
                                .on_click(move |_, _, cx| {
                                    settings_state
                                        .update(cx, |state, cx| state.show_settings(true, cx))
                                }),
                        ),
                    )
                    .child(scan_banner::scan_banner(&self.state, cx))
                    .child(super::collection_view::summary(state, theme))
                    .child(super::tags_view::organization_toolbar(
                        &self.state,
                        theme,
                        cx,
                    ))
                    .child(super::merge_sheet::toolbar(&self.state, theme, cx))
                    .child(div().flex_1().min_h_0().child(content)),
            )
            .child(
                div()
                    .w(px(DETAIL_WIDTH))
                    .h_full()
                    .flex_none()
                    .border_l_1()
                    .border_color(rgb(theme.line))
                    .child(self.detail.clone()),
            )
            .child(self.edit_sheet.clone())
            .child(self.cover_picker.clone())
            .child(self.organization.clone())
            .child(self.structure.clone())
    }
}
