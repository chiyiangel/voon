use super::theme::Theme;
use crate::{
    ui::SettingsPage,
    view_model::{AppState, ScanState},
};
use gpui::{Entity, IntoElement, div, prelude::*, rgb};

pub(super) fn scan_status(snapshot: &AppState) -> Option<String> {
    let operation = snapshot.scan_operation();
    match snapshot.scan_state() {
        ScanState::Idle => None,
        ScanState::Scanning {
            current_directory,
            discovered,
            ..
        } => Some(format!(
            "正在{} · 已发现 {discovered} 张专辑\n{}",
            operation.label(),
            current_directory.display()
        )),
        ScanState::Done {
            root,
            discovered,
            reads,
        } => Some(
            if operation != crate::view_model::scanning::ScanOperation::Incremental {
                format!(
                    "本轮{}结束 · 实际读取 {reads} 个音频文件；读取状态见专辑详情和待处理",
                    operation.label()
                )
            } else if snapshot
                .attention_items()
                .iter()
                .any(|item| item.stage.is_some() && item.path.starts_with(root))
            {
                format!(
                    "本次读取不完整 · 已保留完整资料 · {discovered} 张专辑 · 读取 {reads} 个音频文件；请查看待处理"
                )
            } else {
                format!("扫描完成 · {discovered} 张专辑 · 读取 {reads} 个音频文件")
            },
        ),
        ScanState::Cancelled { discovered, .. } => Some(format!(
            "{}已取消 · 本次发现 {discovered} 张专辑 · 已接受的完整资料已保留",
            operation.label()
        )),
        ScanState::Missing { root } => Some(format!("目录未连接：{}", root.display())),
        ScanState::Empty { root } => Some(format!(
            "本次未发现专辑，已保留上次索引。请确认音乐库已挂载。\n{}",
            root.display()
        )),
    }
}

use crate::motion::{self, Linger};
use gpui::{Context, Render, Subscription, Window};

const OFFLINE_HEIGHT: f32 = 44.;
const STATUS_HEIGHT: f32 = 72.;
const ERROR_HEIGHT: f32 = 40.;

struct BannerSlot<T> {
    linger: Linger<T>,
    entering: bool,
}

impl<T> Default for BannerSlot<T> {
    fn default() -> Self {
        Self {
            linger: Linger::default(),
            entering: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BannerPhase {
    Entering,
    Shown,
    Leaving,
}

pub struct ScanBanner {
    state: Entity<AppState>,
    offline: BannerSlot<()>,
    status: BannerSlot<String>,
    error: BannerSlot<String>,
    shown_offline: Option<()>,
    shown_status: Option<String>,
    shown_error: Option<String>,
    _subscription: Subscription,
}

impl ScanBanner {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&state, |_, _, cx| cx.notify());
        Self {
            state,
            offline: BannerSlot::default(),
            status: BannerSlot::default(),
            error: BannerSlot::default(),
            shown_offline: None,
            shown_status: None,
            shown_error: None,
            _subscription: subscription,
        }
    }

    fn finish_offline(&mut self, cx: &mut Context<Self>) {
        self.offline.entering = false;
        cx.notify();
    }

    fn finish_status(&mut self, cx: &mut Context<Self>) {
        self.status.entering = false;
        cx.notify();
    }

    fn finish_error(&mut self, cx: &mut Context<Self>) {
        self.error.entering = false;
        cx.notify();
    }
}

impl Render for ScanBanner {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::for_window(window);
        let (live_offline, live_status, live_error) = {
            let snapshot = self.state.read(cx);
            (
                (!snapshot.offline_roots().is_empty()).then_some(()),
                scan_status(snapshot),
                snapshot.error().map(str::to_owned),
            )
        };

        sync_unit(
            &mut self.shown_offline,
            &mut self.offline,
            live_offline,
            cx,
            |this, cx| this.finish_offline(cx),
            |this, generation, cx| {
                this.offline.linger.finish(generation);
                cx.notify();
            },
        );
        sync_value(
            &mut self.shown_status,
            &mut self.status,
            live_status,
            cx,
            |this, cx| this.finish_status(cx),
            |this, generation, cx| {
                this.status.linger.finish(generation);
                cx.notify();
            },
        );
        sync_value(
            &mut self.shown_error,
            &mut self.error,
            live_error,
            cx,
            |this, cx| this.finish_error(cx),
            |this, generation, cx| {
                this.error.linger.finish(generation);
                cx.notify();
            },
        );

        let mut banner = div().flex().flex_col().text_sm();
        if let Some(phase) = unit_phase(&self.shown_offline, &self.offline) {
            let settings = self.state.clone();
            let row = div()
                .px_4()
                .py_2()
                .flex()
                .items_center()
                .gap_3()
                .bg(rgb(theme.well))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child("音乐目录未连接 · 正在使用本地索引"),
                )
                .child(
                    super::settings::button(
                        "offline-library-settings",
                        "管理音乐目录…",
                        true,
                        theme,
                    )
                    .on_click(move |_, _, cx| {
                        crate::application::open_settings_page(
                            settings.clone(),
                            SettingsPage::Directories,
                            cx,
                        )
                    }),
                );
            banner = banner.child(paint_row("offline-banner", phase, OFFLINE_HEIGHT, row));
        }
        if let Some((status, phase)) = value_phase(&self.shown_status, &self.status) {
            let settings = self.state.clone();
            let scanning = self.state.read(cx).scan_state().is_scanning();
            let mut row = div()
                .px_4()
                .py_2()
                .flex()
                .items_center()
                .gap_3()
                .bg(rgb(theme.well))
                .child(div().flex_1().min_w_0().child(status))
                .child(
                    super::settings::button("scan-settings", "扫描与读取…", true, theme).on_click(
                        move |_, _, cx| {
                            crate::application::open_settings_page(
                                settings.clone(),
                                SettingsPage::Scanning,
                                cx,
                            )
                        },
                    ),
                );
            if scanning {
                row = row.child(indeterminate_pulse(theme));
            }
            banner = banner.child(paint_row("scan-status-banner", phase, STATUS_HEIGHT, row));
        }
        if let Some((error, phase)) = value_phase(&self.shown_error, &self.error) {
            let row = div()
                .px_4()
                .py_2()
                .text_color(rgb(theme.accent))
                .child(error);
            banner = banner.child(paint_row("scan-error-banner", phase, ERROR_HEIGHT, row));
        }
        banner
    }
}

trait ScanLike {
    fn is_scanning(&self) -> bool;
}

impl ScanLike for ScanState {
    fn is_scanning(&self) -> bool {
        matches!(self, ScanState::Scanning { .. })
    }
}

fn sync_unit<V: 'static>(
    shown: &mut Option<()>,
    slot: &mut BannerSlot<()>,
    live: Option<()>,
    cx: &mut Context<V>,
    finish_entering: impl FnOnce(&mut V, &mut Context<V>) + 'static,
    finish_leaving: impl FnOnce(&mut V, u64, &mut Context<V>) + 'static,
) {
    match (*shown, slot.linger.is_closing(), live) {
        (None, false, Some(())) => {
            *shown = Some(());
            slot.entering = !motion::banner().is_zero();
            if slot.entering {
                motion::schedule(cx, motion::banner(), finish_entering);
            }
        }
        (Some(()), _, None) => {
            *shown = None;
            slot.entering = false;
            if let Some((generation, duration)) = slot.linger.begin((), motion::banner()) {
                motion::schedule(cx, duration, move |this, cx| {
                    finish_leaving(this, generation, cx)
                });
            }
        }
        _ => {}
    }
}

fn sync_value<V: 'static>(
    shown: &mut Option<String>,
    slot: &mut BannerSlot<String>,
    live: Option<String>,
    cx: &mut Context<V>,
    finish_entering: impl FnOnce(&mut V, &mut Context<V>) + 'static,
    finish_leaving: impl FnOnce(&mut V, u64, &mut Context<V>) + 'static,
) {
    match (shown.as_ref(), slot.linger.is_closing(), live) {
        (None, false, Some(value)) => {
            *shown = Some(value);
            slot.entering = !motion::banner().is_zero();
            if slot.entering {
                motion::schedule(cx, motion::banner(), finish_entering);
            }
        }
        (Some(_), _, None) => {
            let leaving = shown.take().unwrap();
            slot.entering = false;
            if let Some((generation, duration)) = slot.linger.begin(leaving, motion::banner()) {
                motion::schedule(cx, duration, move |this, cx| {
                    finish_leaving(this, generation, cx)
                });
            }
        }
        (Some(current), false, Some(value)) if current != &value => {
            *shown = Some(value);
        }
        _ => {}
    }
}

fn unit_phase(shown: &Option<()>, slot: &BannerSlot<()>) -> Option<BannerPhase> {
    if slot.linger.is_closing() {
        Some(BannerPhase::Leaving)
    } else if shown.is_some() {
        Some(if slot.entering {
            BannerPhase::Entering
        } else {
            BannerPhase::Shown
        })
    } else {
        None
    }
}

fn value_phase(shown: &Option<String>, slot: &BannerSlot<String>) -> Option<(String, BannerPhase)> {
    if let Some(value) = slot.linger.closing_value() {
        Some((value.clone(), BannerPhase::Leaving))
    } else {
        shown.as_ref().map(|value| {
            (
                value.clone(),
                if slot.entering {
                    BannerPhase::Entering
                } else {
                    BannerPhase::Shown
                },
            )
        })
    }
}

fn paint_row(
    id: &'static str,
    phase: BannerPhase,
    height: f32,
    row: impl gpui::Styled + gpui::IntoElement + 'static,
) -> gpui::AnyElement {
    match phase {
        BannerPhase::Shown => row.into_any_element(),
        BannerPhase::Entering => motion::reveal(
            gpui::SharedString::from(format!("{id}-enter")),
            true,
            motion::banner(),
            height,
            row,
        ),
        BannerPhase::Leaving => motion::reveal(
            gpui::SharedString::from(format!("{id}-exit")),
            false,
            motion::banner(),
            height,
            row,
        ),
    }
}

fn indeterminate_pulse(theme: Theme) -> impl gpui::IntoElement {
    use gpui::{Animation, AnimationExt, pulsating_between};
    let color = theme.accent;
    if motion::banner().is_zero() {
        return div()
            .w(gpui::px(48.))
            .h(gpui::px(3.))
            .rounded_full()
            .bg(rgb(color))
            .into_any_element();
    }
    div()
        .id("scan-indeterminate")
        .w(gpui::px(48.))
        .h(gpui::px(3.))
        .rounded_full()
        .bg(rgb(color))
        .with_animation(
            "scan-indeterminate-pulse",
            Animation::new(std::time::Duration::from_millis(1400))
                .repeat()
                .with_easing(pulsating_between(0.35, 1.0)),
            |bar, delta| bar.opacity(delta),
        )
        .into_any_element()
}
