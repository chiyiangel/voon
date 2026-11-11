use gpui::{
    Context, Entity, FontWeight, MouseButton, Render, Subscription, Window, deferred, div,
    prelude::*, px, rgb,
};

use super::{
    icons::icon,
    sidebar::navigation_label,
    text_input::{TextChanged, TextInput},
    theme::{TOOLBAR_HEIGHT, Theme},
};
use crate::view_model::{
    AppState, SortOrder,
    browse::{SortDirection, ViewMode},
};

pub struct Toolbar {
    state: Entity<AppState>,
    search: Entity<TextInput>,
    sort_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl Toolbar {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| {
            let mut input = TextInput::new("专辑、艺术家、作曲家、唱片编号", cx);
            input.set_text(state.read(cx).search().to_string(), cx);
            input
        });
        let query_state = state.clone();
        let subscriptions = vec![
            cx.subscribe(&search, move |_, _, event: &TextChanged, cx| {
                query_state.update(cx, |state, cx| state.set_search(event.text.clone(), cx));
            }),
            cx.observe(&state, |this, state, cx| {
                let query = state.read(cx).search().to_string();
                this.search
                    .update(cx, |input, cx| input.set_text(query, cx));
                cx.notify();
            }),
        ];
        Self {
            state,
            search,
            sort_open: false,
            _subscriptions: subscriptions,
        }
    }
}

impl Render for Toolbar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let state = self.state.read(cx);
        let mode = state.view_mode();
        let sort = state.sort_order();
        let direction = state.sort_direction();
        let mut modes = div()
            .flex()
            .items_center()
            .p(px(2.))
            .rounded(px(7.))
            .bg(rgb(theme.well))
            .border_1()
            .border_color(rgb(theme.line));
        for (view, glyph, id) in [
            (ViewMode::Grid, "grid", "grid-view"),
            (ViewMode::List, "list", "list-view"),
        ] {
            let state = self.state.clone();
            modes = modes.child(
                div()
                    .id(id)
                    .w(px(30.))
                    .h(px(22.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(5.))
                    .bg(rgb(if mode == view {
                        theme.window
                    } else {
                        theme.well
                    }))
                    .cursor_pointer()
                    .child(icon(
                        glyph,
                        14.,
                        if mode == view {
                            theme.ink
                        } else {
                            theme.tertiary
                        },
                    ))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(move |_, _, cx| {
                        state.update(cx, |state, cx| state.set_view_mode(view, cx))
                    }),
            );
        }
        let mut toolbar = div()
            .relative()
            .h(px(TOOLBAR_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(10.))
            .pl(px(18.))
            .pr(px(14.))
            .bg(rgb(theme.toolbar))
            .border_b_1()
            .border_color(rgb(theme.line))
            .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
            .child(
                div()
                    .min_w_0()
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .truncate()
                    .child(navigation_label(state.navigation())),
            )
            .child(div().flex_1())
            .child(modes)
            .child(
                div()
                    .id("choose-album-sort")
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .cursor_pointer()
                    .text_size(px(12.))
                    .text_color(rgb(theme.secondary))
                    .child(icon("sort", 14., theme.secondary))
                    .child(format!(
                        "{} {}",
                        sort.label(),
                        if direction == SortDirection::Ascending {
                            "↑"
                        } else {
                            "↓"
                        }
                    ))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.sort_open = !this.sort_open;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .w(px(240.))
                    .min_w(px(120.))
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(8.))
                    .rounded(px(7.))
                    .bg(rgb(theme.well))
                    .border_1()
                    .border_color(rgb(theme.line))
                    .text_color(rgb(theme.ink))
                    .child(icon("search", 14., theme.tertiary))
                    .child(div().flex_1().min_w_0().child(self.search.clone())),
            );
        if self.sort_open {
            let mut menu = div()
                .id("album-sort-menu")
                .absolute()
                .top(px(46.))
                .right(px(256.))
                .w(px(180.))
                .p_1()
                .rounded_md()
                .bg(rgb(theme.window))
                .border_1()
                .border_color(rgb(theme.line))
                .shadow_lg()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.sort_open = false;
                    cx.notify();
                }));
            for order in SortOrder::ALL {
                if order == SortOrder::Manual && !(state.navigation() == crate::view_model::Navigation::ManualCollections && matches!(state.organization_scope(), Some(crate::view_model::organization::OrganizationScope::ManualCollection(_)))) { continue; }
                menu = menu.child(
                    div()
                        .id(("sort-albums-by", order as usize))
                        .px_3()
                        .py_1()
                        .rounded_sm()
                        .cursor_pointer()
                        .text_sm()
                        .bg(rgb(if sort == order {
                            theme.selected
                        } else {
                            theme.window
                        }))
                        .hover(move |style| style.bg(rgb(theme.well)))
                        .child(order.label())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.state
                                .update(cx, |state, cx| state.set_sort_order(order, cx));
                            this.sort_open = false;
                            cx.notify();
                        })),
                );
            }
            toolbar = toolbar.child(deferred(menu).with_priority(2));
        }
        toolbar
    }
}
