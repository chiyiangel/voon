use gpui::{
    Context, Div, Entity, FocusHandle, FontWeight, Render, Subscription, Window, div, prelude::*,
    px, rgb,
};

use super::{
    application_menu_bar::ApplicationMenuBar,
    icons::icon,
    settings::{button, caption},
    theme::Theme,
};
use crate::view_model::AppState;

mod directories;
mod ignore;
mod local_data;
mod scanning;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsPage {
    #[default]
    Directories,
    Scanning,
    Ignore,
    LocalData,
}

impl SettingsPage {
    fn label(self) -> &'static str {
        match self {
            Self::Directories => "音乐目录",
            Self::Scanning => "扫描与读取",
            Self::Ignore => "忽略规则",
            Self::LocalData => "本地数据",
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Directories => "settings-directories",
            Self::Scanning => "settings-scanning",
            Self::Ignore => "settings-ignore",
            Self::LocalData => "settings-local-data",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Directories => "管理 Voon 从哪里读取你的音乐。",
            Self::Scanning => "管理扫描任务，处理未能读取的音乐资料。",
            Self::Ignore => "跳过不需要纳入音乐库的子文件夹。",
            Self::LocalData => "管理本机资料库和封面缓存。",
        }
    }
}

pub struct SettingsWindow {
    state: Entity<AppState>,
    menu_bar: Entity<ApplicationMenuBar>,
    focus: FocusHandle,
    page: SettingsPage,
    confirm_new_index: bool,
    built_in_rules_open: bool,
    _subscription: Subscription,
}

impl SettingsWindow {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window);
        let subscription = cx.observe(&state, |_, _, cx| cx.notify());
        let menu_bar = cx.new(ApplicationMenuBar::new);
        let page = if state.read(cx).index_open_problem().is_some() {
            SettingsPage::LocalData
        } else {
            SettingsPage::Directories
        };
        Self {
            state,
            menu_bar,
            focus,
            page,
            confirm_new_index: false,
            built_in_rules_open: false,
            _subscription: subscription,
        }
    }

    pub fn set_page(&mut self, page: SettingsPage, cx: &mut Context<Self>) {
        self.page = page;
        self.confirm_new_index = false;
        cx.notify();
    }

    fn page_link(
        &self,
        id: &'static str,
        label: &'static str,
        page: SettingsPage,
        theme: Theme,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        button(id, label, true, theme)
            .on_click(cx.listener(move |this, _, _, cx| this.set_page(page, cx)))
    }

    fn sidebar(&self, theme: Theme, cx: &Context<Self>) -> Div {
        let mut sidebar = div()
            .w(px(176.))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .p_3()
            .gap_1()
            .bg(rgb(theme.sidebar))
            .border_r_1()
            .border_color(rgb(theme.line))
            .child(caption("偏好设置", theme).px_2().pt_3().pb_2());
        for (page, glyph) in [
            (SettingsPage::Directories, "folder"),
            (SettingsPage::Scanning, "refresh"),
            (SettingsPage::Ignore, "facet"),
            (SettingsPage::LocalData, "database"),
        ] {
            sidebar = sidebar.child(
                div()
                    .id(page.id())
                    .debug_selector(|| page.id().into())
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(move |row| row.bg(rgb(theme.selected)))
                    .when(self.page == page, |row| {
                        row.bg(rgb(theme.selected)).font_weight(FontWeight::MEDIUM)
                    })
                    .child(icon(
                        glyph,
                        16.,
                        if self.page == page {
                            theme.accent
                        } else {
                            theme.secondary
                        },
                    ))
                    .child(page.label())
                    .on_click(cx.listener(move |this, _, _, cx| this.set_page(page, cx))),
            );
        }
        sidebar
            .child(div().flex_1())
            .child(caption("VOON · 本地音乐资料库", theme).px_2().pb_2())
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let page = match self.page {
            SettingsPage::Directories => self.directories(theme, cx),
            SettingsPage::Scanning => self.scanning(theme, cx),
            SettingsPage::Ignore => self.ignore(theme, cx),
            SettingsPage::LocalData => self.local_data(theme, cx),
        };
        let state = self.state.read(cx);
        let content = div()
            .id(("settings-page", self.page as usize))
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_4()
            .p_6()
            .child(
                div()
                    .flex_none()
                    .flex()
                    .justify_between()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(22.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(self.page.label()),
                    )
                    .child(
                        button("close-settings", "完成", true, theme)
                            .on_click(|_, window, _| window.remove_window()),
                    ),
            )
            .child(caption(self.page.description(), theme).flex_none())
            .children(state.settings_error().map(|error| {
                caption(error.to_owned(), theme)
                    .text_color(rgb(theme.accent))
                    .flex_none()
            }))
            .children(
                state
                    .index_notice()
                    .map(|notice| caption(notice.to_owned(), theme).flex_none()),
            )
            .child(page)
            .child(div().h(px(8.)).flex_none());
        div()
            .size_full()
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .bg(rgb(theme.well))
            .text_color(rgb(theme.ink))
            .font_family(".SystemUIFont")
            .text_size(px(13.))
            .when(cfg!(target_os = "linux"), |root| {
                root.child(self.menu_bar.clone())
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.sidebar(theme, cx))
                    .child(div().flex_1().min_w_0().h_full().child(content)),
            )
    }
}
