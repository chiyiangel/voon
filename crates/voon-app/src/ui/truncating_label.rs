//! Single-line labels that fill their final bounds and ellipsize with real shaping.
//!
//! GPUI 0.2.2 `truncate()` / `text_ellipsis()` cut text during measure with
//! per-character `width_for_char` widths. Painting then uses `shape_line`.
//! When those two disagree, a flex row can still give the label the remaining
//! width while only a few glyphs are kept, leaving a gap before a trailing
//! count. This element waits for the laid-out bounds and truncates with the
//! same `shape_line` path used to paint.

use std::cell::{Cell, RefCell};

use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, ShapedLine, SharedString, Style, Window, px, relative,
};

const ELLIPSIS: &str = "…";

#[derive(Clone, Debug, PartialEq)]
pub struct LabelSnapshot {
    pub source: String,
    pub displayed: String,
    pub available: f32,
    pub text_width: f32,
}

thread_local! {
    static CAPTURE: Cell<bool> = const { Cell::new(false) };
    static SNAPSHOTS: RefCell<Vec<LabelSnapshot>> = const { RefCell::new(Vec::new()) };
}

/// Begin recording laid-out label text on this thread until the guard drops.
pub fn capture_labels() -> LabelCapture {
    SNAPSHOTS.with(|slots| slots.borrow_mut().clear());
    CAPTURE.with(|flag| flag.set(true));
    LabelCapture
}

pub struct LabelCapture;

impl Drop for LabelCapture {
    fn drop(&mut self) {
        CAPTURE.with(|flag| flag.set(false));
    }
}

pub fn snapshots() -> Vec<LabelSnapshot> {
    SNAPSHOTS.with(|slots| slots.borrow().clone())
}

fn record(snapshot: LabelSnapshot) {
    if CAPTURE.with(|flag| flag.get()) {
        SNAPSHOTS.with(|slots| slots.borrow_mut().push(snapshot));
    }
}

/// Choose a prefix of `text` plus an ellipsis that fits in `max_width`.
///
/// `measure` must use the same typographic path that will paint the result.
pub fn ellipsize_to_width(
    text: &str,
    max_width: Pixels,
    mut measure: impl FnMut(&str) -> Pixels,
) -> String {
    if max_width <= px(0.) {
        return String::new();
    }
    if measure(text) <= max_width {
        return text.to_string();
    }
    let ellipsis_width = measure(ELLIPSIS);
    if ellipsis_width >= max_width {
        return ELLIPSIS.to_string();
    }
    let budget = max_width - ellipsis_width;
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return ELLIPSIS.to_string();
    }
    let mut low = 0;
    let mut high = chars.len();
    while low < high {
        let mid = (low + high).div_ceil(2);
        let prefix: String = chars[..mid].iter().collect();
        if measure(&prefix) <= budget {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    // Combined shaping can be wider than prefix + ellipsis (kerning), so
    // shrink until the string that will actually be painted fits.
    while low > 0 {
        let mut displayed: String = chars[..low].iter().collect();
        displayed.push_str(ELLIPSIS);
        if measure(&displayed) <= max_width {
            return displayed;
        }
        low -= 1;
    }
    ELLIPSIS.to_string()
}

pub struct TruncatingLabel {
    text: SharedString,
}

pub fn truncating_label(text: impl Into<SharedString>) -> TruncatingLabel {
    TruncatingLabel { text: text.into() }
}

impl IntoElement for TruncatingLabel {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TruncatingLabel {
    type RequestLayoutState = ();
    type PrepaintState = Option<ShapedLine>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        // Fill the already-allocated parent width instead of reporting an
        // intrinsic text size that flex would have to fight.
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        if bounds.size.width <= px(0.) || bounds.size.height <= px(0.) {
            return None;
        }
        let source = self.text.to_string();
        let displayed = ellipsize_to_width(&source, bounds.size.width, |text| {
            shape_line(window, text).width
        });
        let line = shape_line(window, &displayed);
        record(LabelSnapshot {
            source,
            displayed,
            available: f32::from(bounds.size.width),
            text_width: f32::from(line.width),
        });
        Some(line)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(line) = prepaint.take() {
            line.paint(bounds.origin, window.line_height(), window, cx)
                .ok();
        }
    }
}

fn shape_line(window: &mut Window, text: &str) -> ShapedLine {
    let style = window.text_style();
    let font_size = style.font_size.to_pixels(window.rem_size());
    let text = SharedString::from(text.to_string());
    let run = style.to_run(text.len());
    window
        .text_system()
        .shape_line(text, font_size, &[run], None)
}
