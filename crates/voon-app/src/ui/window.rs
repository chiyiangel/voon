use gpui::{
    App, Context, Entity, FocusHandle, Focusable, Render, Subscription, Timer, Window, div,
    prelude::*, px, rgb,
};

use super::{
    application_menu_bar::ApplicationMenuBar,
    settings, sidebar, startup,
    theme::{DETAIL_WIDTH, Theme},
    toolbar,
};
use crate::motion;
use crate::view_model::{AppState, Navigation, browse::ViewMode};

pub struct VoonWindow {
    focus: FocusHandle,
    state: Entity<AppState>,
    menu_bar: Entity<ApplicationMenuBar>,
    banner: Entity<super::scan_banner::ScanBanner>,
    grid: Entity<super::album_grid::AlbumGrid>,
    list: Entity<super::album_list::AlbumList>,
    toolbar: Entity<toolbar::Toolbar>,
    detail: Entity<super::album_detail::AlbumDetail>,
    edit_sheet: Entity<super::edit_sheet::EditSheet>,
    cover_picker: Entity<super::cover_picker::CoverPicker>,
    organization: Entity<super::tags_view::TagsView>,
    structure: Entity<super::merge_sheet::MergeSheet>,
    browse_surface: BrowseSurface,
    browse_previous: Option<(BrowseSurface, u64)>,
    browse_generation: u64,
    panel: Option<PanelKind>,
    panel_previous: Option<(PanelKind, u64)>,
    panel_generation: u64,
    sidebar_nav: Navigation,
    sidebar_previous: Option<(Navigation, u64)>,
    sidebar_generation: u64,
    _state_subscription: Subscription,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BrowseSurface {
    FirstUse,
    Inbox,
    Grid,
    List,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PanelKind {
    Facets,
    Composers,
}

impl VoonWindow {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        state.update(cx, |state, cx| state.prefetch_covers(cx));
        let subscription = cx.observe(&state, |_, _, cx| cx.notify());
        let menu_bar = cx.new(ApplicationMenuBar::new);
        let banner = cx.new(|cx| super::scan_banner::ScanBanner::new(state.clone(), cx));
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
            banner,
            grid,
            list,
            toolbar,
            detail,
            edit_sheet,
            cover_picker,
            organization,
            structure,
            browse_surface: BrowseSurface::FirstUse,
            browse_previous: None,
            browse_generation: 0,
            panel: None,
            panel_previous: None,
            panel_generation: 0,
            sidebar_nav: Navigation::AllAlbums,
            sidebar_previous: None,
            sidebar_generation: 0,
            _state_subscription: subscription,
        }
    }
}

impl Render for VoonWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window);
        if self.state.read(cx).index_open_problem().is_some() {
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
        let (live_surface, live_panel, navigation) = {
            let state = self.state.read(cx);
            let surface = if state.navigation() == Navigation::NeedsAttention {
                BrowseSurface::Inbox
            } else if state.roots().is_empty() {
                BrowseSurface::FirstUse
            } else if state.view_mode() == ViewMode::List {
                BrowseSurface::List
            } else {
                BrowseSurface::Grid
            };
            let panel = match state.navigation() {
                Navigation::Facets => Some(PanelKind::Facets),
                Navigation::Composers => Some(PanelKind::Composers),
                _ => None,
            };
            (surface, panel, state.navigation())
        };
        self.sync_browse(live_surface, cx);
        self.sync_panel(live_panel, cx);
        self.sync_sidebar(navigation, cx);
        let content = self.render_browse(theme, cx);
        let state = self.state.read(cx);
        let browser = div()
            .relative()
            .size_full()
            .flex()
            .overflow_hidden()
            .bg(rgb(theme.window))
            .text_color(rgb(theme.ink))
            .font_family(".SystemUIFont")
            .text_size(px(13.))
            .child(sidebar::render(
                state,
                &self.state,
                theme,
                self.sidebar_previous.map(|(navigation, _)| navigation),
            ))
            .child(
                div()
                    .h_full()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(self.toolbar.clone())
                    .child(self.banner.clone())
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

impl VoonWindow {
    fn sync_browse(&mut self, live: BrowseSurface, cx: &mut Context<Self>) {
        if live == self.browse_surface {
            return;
        }
        self.browse_generation = self.browse_generation.wrapping_add(1);
        let generation = self.browse_generation;
        let duration = motion::crossfade();
        if duration.is_zero() {
            self.browse_previous = None;
        } else {
            self.browse_previous = Some((self.browse_surface, generation));
            motion::schedule(cx, duration, move |this, cx| {
                if this
                    .browse_previous
                    .as_ref()
                    .is_some_and(|(_, current)| *current == generation)
                {
                    this.browse_previous = None;
                    cx.notify();
                }
            });
        }
        self.browse_surface = live;
    }

    fn sync_sidebar(&mut self, live: Navigation, cx: &mut Context<Self>) {
        if live == self.sidebar_nav {
            return;
        }
        self.sidebar_generation = self.sidebar_generation.wrapping_add(1);
        let generation = self.sidebar_generation;
        let duration = motion::selection();
        if duration.is_zero() {
            self.sidebar_previous = None;
        } else {
            self.sidebar_previous = Some((self.sidebar_nav, generation));
            motion::schedule(cx, duration, move |this, cx| {
                if this
                    .sidebar_previous
                    .as_ref()
                    .is_some_and(|(_, current)| *current == generation)
                {
                    this.sidebar_previous = None;
                    cx.notify();
                }
            });
        }
        self.sidebar_nav = live;
    }

    fn sync_panel(&mut self, live: Option<PanelKind>, cx: &mut Context<Self>) {
        if live == self.panel {
            return;
        }
        self.panel_generation = self.panel_generation.wrapping_add(1);
        let generation = self.panel_generation;
        let duration = motion::crossfade();
        if duration.is_zero() {
            self.panel_previous = None;
        } else if let Some(previous) = self.panel {
            self.panel_previous = Some((previous, generation));
            motion::schedule(cx, duration, move |this, cx| {
                if this
                    .panel_previous
                    .as_ref()
                    .is_some_and(|(_, current)| *current == generation)
                {
                    this.panel_previous = None;
                    cx.notify();
                }
            });
        } else {
            self.panel_previous = None;
        }
        self.panel = live;
    }

    fn render_browse(
        &self,
        theme: super::theme::Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let current = motion::fade(
            ("browse-to", self.browse_generation as usize),
            true,
            motion::crossfade(),
            div()
                .size_full()
                .child(self.paint_surface(self.browse_surface, theme, cx)),
        );
        if let Some((previous, generation)) = self.browse_previous {
            div()
                .relative()
                .size_full()
                .child(
                    div().absolute().inset_0().child(motion::fade(
                        ("browse-from", generation as usize),
                        false,
                        motion::crossfade(),
                        div()
                            .size_full()
                            .child(self.paint_surface(previous, theme, cx)),
                    )),
                )
                .child(current)
                .into_any_element()
        } else {
            current
        }
    }

    fn paint_surface(
        &self,
        surface: BrowseSurface,
        theme: super::theme::Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let state = self.state.read(cx);
        match surface {
            BrowseSurface::Inbox => {
                super::inbox_view::render(&self.state, theme, cx).into_any_element()
            }
            BrowseSurface::FirstUse => {
                startup::first_use(&self.state, theme, cx).into_any_element()
            }
            BrowseSurface::Grid | BrowseSurface::List => {
                let browse = if surface == BrowseSurface::List {
                    self.list.clone().into_any_element()
                } else {
                    self.grid.clone().into_any_element()
                };
                let mut row = div().size_full().flex().relative();
                if let Some((kind, generation)) = self.panel_previous {
                    row = row.child(div().absolute().left_0().top_0().bottom_0().child(
                        motion::fade(
                            ("panel-from", generation as usize),
                            false,
                            motion::crossfade(),
                            div().h_full().child(self.paint_panel(kind, state, theme)),
                        ),
                    ));
                }
                if let Some(kind) = self.panel {
                    row = row.child(motion::fade(
                        ("panel-to", self.panel_generation as usize),
                        true,
                        motion::crossfade(),
                        div().h_full().child(self.paint_panel(kind, state, theme)),
                    ));
                }
                row.child(
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
            }
        }
    }

    fn paint_panel(
        &self,
        kind: PanelKind,
        state: &AppState,
        theme: super::theme::Theme,
    ) -> gpui::AnyElement {
        match kind {
            PanelKind::Facets => {
                super::facet_view::panel(state, &self.state, theme).into_any_element()
            }
            PanelKind::Composers => {
                super::composer_view::panel(state, &self.state, theme).into_any_element()
            }
        }
    }
}

impl Focusable for VoonWindow {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
