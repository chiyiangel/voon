//! Shared motion tokens, reduced-motion policy and exit-state helpers.
//!
//! Views keep using these durations and the linger / fade helpers instead of
//! inventing one-off timings. When reduced motion is on, every token is zero
//! and helpers snap to the end state instead of calling `with_animation`.

use std::{cell::Cell, sync::OnceLock, time::Duration};

use gpui::{
    Animation, AnimationExt, AnyElement, Context, ElementId, IntoElement, ease_out_quint,
    prelude::*,
};

/// Dialog, sheet and modal open / close.
pub const OVERLAY: Duration = Duration::from_millis(220);
/// Popover and window-menu open / close.
pub const MENU: Duration = Duration::from_millis(180);
/// Scan, offline and error banners, including height.
pub const BANNER: Duration = Duration::from_millis(200);
/// Grid ↔ list, sidebar destinations and first-library transition.
pub const CROSSFADE: Duration = Duration::from_millis(200);
/// Album detail content swap and inspector tab change.
pub const DETAIL: Duration = Duration::from_millis(180);
/// Facet chips and collection-tree rows.
pub const CHIP: Duration = Duration::from_millis(150);
/// Sidebar and list selection colour.
pub const SELECTION: Duration = Duration::from_millis(140);
/// Cover-cache bar and similar determinate widths.
pub const PROGRESS: Duration = Duration::from_millis(250);
/// New Album cover entrance; matches the design-canvas pop.
pub const NEW_CARD: Duration = Duration::from_millis(600);

/// Quintic ease-out, 0..=1 → 0..=1.
pub fn eased(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(5)
}

pub fn overlay() -> Duration {
    token(OVERLAY)
}
pub fn menu() -> Duration {
    token(MENU)
}
pub fn banner() -> Duration {
    token(BANNER)
}
pub fn crossfade() -> Duration {
    token(CROSSFADE)
}
pub fn detail() -> Duration {
    token(DETAIL)
}
pub fn chip() -> Duration {
    token(CHIP)
}
pub fn selection() -> Duration {
    token(SELECTION)
}
pub fn progress() -> Duration {
    token(PROGRESS)
}
pub fn new_card() -> Duration {
    token(NEW_CARD)
}

fn token(duration: Duration) -> Duration {
    if reduce_motion() {
        Duration::ZERO
    } else {
        duration
    }
}

thread_local! {
    static OVERRIDE: Cell<Option<bool>> = const { Cell::new(None) };
}

/// Force reduced motion for the current thread. Drop the guard to clear it.
pub fn force_reduce_motion(value: bool) -> ReduceMotionGuard {
    OVERRIDE.with(|cell| cell.set(Some(value)));
    ReduceMotionGuard
}

pub struct ReduceMotionGuard;

impl Drop for ReduceMotionGuard {
    fn drop(&mut self) {
        OVERRIDE.with(|cell| cell.set(None));
    }
}

/// `VOON_REDUCE_MOTION` wins over the platform setting. Recognised values:
/// `1` / `true` / `yes` / `on` enable it; `0` / `false` / `no` / `off` disable
/// it; any other non-empty value enables it.
pub fn reduce_motion() -> bool {
    if let Some(value) = OVERRIDE.with(Cell::get) {
        return value;
    }
    if let Ok(value) = std::env::var("VOON_REDUCE_MOTION")
        && !value.is_empty()
    {
        return parse_env(&value);
    }
    platform_reduce_motion()
}

fn parse_env(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "0" | "false" | "no" | "off"
    )
}

fn platform_reduce_motion() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(platform_reduce_motion_uncached)
}

#[cfg(target_os = "macos")]
fn platform_reduce_motion_uncached() -> bool {
    macos_reduce_motion()
}

#[cfg(target_os = "linux")]
fn platform_reduce_motion_uncached() -> bool {
    gnome_animations_disabled()
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn platform_reduce_motion_uncached() -> bool {
    false
}

#[cfg(target_os = "macos")]
fn macos_reduce_motion() -> bool {
    use objc::{class, msg_send, sel, sel_impl};

    // NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceMotion
    unsafe {
        let workspace: *mut objc::runtime::Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace.is_null() {
            return false;
        }
        let reduced: bool = msg_send![workspace, accessibilityDisplayShouldReduceMotion];
        reduced
    }
}

/// Best-effort: GNOME `enable-animations` being false means the user asked
/// for reduced motion. Missing `gsettings` or a missing key leaves motion on.
#[cfg(target_os = "linux")]
fn gnome_animations_disabled() -> bool {
    let output = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "enable-animations"])
        .output();
    match output {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .trim()
            .eq_ignore_ascii_case("false"),
        _ => false,
    }
}

/// Run `update` after `duration`. A zero duration still hops through the
/// executor so callers can `run_until_parked` uniformly, except we invoke
/// it immediately when duration is zero so the UI can snap without a frame
/// of leftover content.
pub fn schedule<T: 'static>(
    cx: &mut Context<T>,
    duration: Duration,
    update: impl FnOnce(&mut T, &mut Context<T>) + 'static,
) {
    cx.spawn(async move |this, cx| {
        if !duration.is_zero() {
            cx.background_executor().timer(duration).await;
        }
        this.update(cx, update).ok();
    })
    .detach();
}

/// Holds a just-dismissed value so a view can keep rendering it until the
/// exit animation finishes. A later `begin` or `cancel` bumps the generation
/// and ignores the previous timer.
pub struct Linger<T> {
    closing: Option<(T, u64)>,
    generation: u64,
}

impl<T> Default for Linger<T> {
    fn default() -> Self {
        Self {
            closing: None,
            generation: 0,
        }
    }
}

impl<T> Linger<T> {
    pub fn is_closing(&self) -> bool {
        self.closing.is_some()
    }

    pub fn closing_value(&self) -> Option<&T> {
        self.closing.as_ref().map(|(value, _)| value)
    }

    /// Prefer the live value, otherwise the value that is still leaving.
    pub fn present<'a>(&'a self, live: Option<&'a T>) -> Option<(&'a T, bool)> {
        if let Some(value) = live {
            return Some((value, false));
        }
        self.closing.as_ref().map(|(value, _)| (value, true))
    }

    /// Start lingering `value`. Returns the generation and duration that the
    /// caller should schedule; `None` means reduced motion, so nothing lingers.
    pub fn begin(&mut self, value: T, duration: Duration) -> Option<(u64, Duration)> {
        self.generation = self.generation.wrapping_add(1);
        if duration.is_zero() {
            self.closing = None;
            return None;
        }
        let generation = self.generation;
        self.closing = Some((value, generation));
        Some((generation, duration))
    }

    pub fn finish(&mut self, generation: u64) -> bool {
        if self
            .closing
            .as_ref()
            .is_some_and(|(_, current)| *current == generation)
        {
            self.closing = None;
            true
        } else {
            false
        }
    }

    /// Drop any in-flight exit, e.g. when the same overlay opens again.
    pub fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.closing = None;
    }
}

/// Fade `element` in (`appearing`) or out. Zero-duration skips `with_animation`
/// so GPUI never divides by a zero interval.
pub fn fade(
    id: impl Into<ElementId>,
    appearing: bool,
    duration: Duration,
    element: impl Styled + IntoElement + 'static,
) -> AnyElement {
    if duration.is_zero() {
        return if appearing {
            element.into_any_element()
        } else {
            element.opacity(0.).into_any_element()
        };
    }
    element
        .with_animation(
            id,
            Animation::new(duration).with_easing(ease_out_quint()),
            move |element, delta| {
                let opacity = if appearing { delta } else { 1.0 - delta };
                element.opacity(opacity)
            },
        )
        .into_any_element()
}

/// Fade plus a height reveal, used by status banners so the grid does not jump.
pub fn reveal(
    id: impl Into<ElementId>,
    appearing: bool,
    duration: Duration,
    height: f32,
    element: impl Styled + IntoElement + 'static,
) -> AnyElement {
    let height = height.max(1.);
    if duration.is_zero() {
        return if appearing {
            element.into_any_element()
        } else {
            element
                .h(gpui::px(0.))
                .overflow_hidden()
                .opacity(0.)
                .into_any_element()
        };
    }
    element
        .overflow_hidden()
        .with_animation(
            id,
            Animation::new(duration).with_easing(ease_out_quint()),
            move |element, delta| {
                let t = if appearing { delta } else { 1.0 - delta };
                element.h(gpui::px(height * t)).opacity(t)
            },
        )
        .into_any_element()
}

/// A set of values that should keep rendering after they have been removed
/// from the live list, until their exit animation completes.
pub struct Leaving<T> {
    items: Vec<(T, u64)>,
    generation: u64,
}

impl<T> Default for Leaving<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            generation: 0,
        }
    }
}

impl<T> Leaving<T> {
    pub fn push(
        &mut self,
        items: impl IntoIterator<Item = T>,
        duration: Duration,
    ) -> Option<(u64, Duration)> {
        if duration.is_zero() {
            return None;
        }
        let items: Vec<T> = items.into_iter().collect();
        if items.is_empty() {
            return None;
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.items
            .extend(items.into_iter().map(|item| (item, generation)));
        Some((generation, duration))
    }

    pub fn finish(&mut self, generation: u64) -> bool {
        let before = self.items.len();
        self.items.retain(|(_, current)| *current != generation);
        before != self.items.len()
    }

    pub fn retain(&mut self, mut predicate: impl FnMut(&T) -> bool) {
        self.items.retain(|(item, _)| predicate(item));
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter().map(|(item, _)| item)
    }
}
