use gpui::{
    App, Context, Entity, FocusHandle, Focusable, Render, Subscription, Timer, Window, div,
    prelude::*, px, rgb,
};

use super::{
    application_menu_bar::ApplicationMenuBar,
    scan_banner, settings, sidebar, startup,
    theme::{DETAIL_WIDTH, Theme},
    toolbar,
};
use crate::view_model::AppState;

pub struct VoonWindow {
    focus: FocusHandle,
    state: Entity<AppState>,
    menu_bar: Entity<ApplicationMenuBar>,
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
        state.update(cx, |state, cx| state.prefetch_covers(cx));
        let subscription = cx.observe(&state, |_, _, cx| cx.notify());
        let menu_bar = cx.new(ApplicationMenuBar::new);
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
            focus: cx.focus_handle(),
            state,
            menu_bar,
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
        if state.index_open_problem().is_some() {
            return div()
                .size_full()
                .track_focus(&self.focus)
                .flex()
                .flex_col()
                .when(cfg!(target_os = "linux"), |root| {
                    root.child(self.menu_bar.clone())
                })
                .child(div().flex_1().min_h_0().child(settings::index_unavailable(
                    &self.state,
                    theme,
                    cx,
                )))
                .into_any_element();
        }
        let content = if state.navigation() == crate::view_model::Navigation::NeedsAttention {
            super::inbox_view::render(&self.state, theme, cx).into_any_element()
        } else if state.roots().is_empty() {
            startup::first_use(&self.state, theme, cx).into_any_element()
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
        let browser = div()
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
                    .child(scan_banner::scan_banner(&self.state, theme, cx))
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
            .into_any_element();
        div()
            .size_full()
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .when(cfg!(target_os = "linux"), |root| {
                root.child(self.menu_bar.clone())
            })
            .child(div().relative().flex_1().min_h_0().child(browser))
            .into_any_element()
    }
}

impl Focusable for VoonWindow {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
