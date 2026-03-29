use std::{cell::Cell, rc::Rc};

use waybar_cffi::gtk::{self as gtk, glib::Propagation, pango, prelude::WidgetExt};

use super::settings::BubbleColors;

/// Bubble radius as a fraction of widget height.
const RADIUS_RATIO: f64 = 0.2;
/// Minimum bubble radius so it stays visible on very small bars.
const MIN_RADIUS: f64 = 3.0;
/// Vertical inset from the top edge, as a fraction of widget height.
const TOP_INSET_RATIO: f64 = 0.0;
/// Default indicator character (a filled circle: ●).
const DEFAULT_INDICATOR: &str = "●";

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum NotificationUrgency {
    Low,
    #[default]
    Normal,
    Critical,
}

impl NotificationUrgency {
    pub fn from_hint(hint: Option<u8>) -> Self {
        match hint {
            Some(0) => Self::Low,
            Some(2) => Self::Critical,
            _ => Self::Normal,
        }
    }

    fn resolve(self, colors: &BubbleColors) -> (f64, f64, f64, f64) {
        match self {
            Self::Low => colors.low.rgba(),
            Self::Normal => colors.normal.rgba(),
            Self::Critical => colors.critical.rgba(),
        }
    }
}

/// Bubble state shared between the draw callback and the public API.
pub struct BubbleState {
    pub active: Cell<bool>,
    pub urgency: Cell<NotificationUrgency>,
}

/// Compute the font size from the widget height.
fn font_size(widget_height: f64) -> f64 {
    (widget_height * RADIUS_RATIO * 2.0).max(MIN_RADIUS * 2.0)
}

/// Compute the right margin needed to avoid clipping the indicator.
fn indicator_margin(widget_height: f64) -> i32 {
    font_size(widget_height) as i32 + 1
}

/// Draw a coloured character as the notification indicator.
fn draw_indicator(
    cr: &gtk::cairo::Context,
    widget: &gtk::EventBox,
    cx: f64,
    cy: f64,
    size: f64,
    indicator: &str,
    r: f64,
    g: f64,
    b: f64,
    a: f64,
) {
    let layout = widget.create_pango_layout(Some(indicator));
    let font = pango::FontDescription::from_string(&format!("{size}px"));
    layout.set_font_description(Some(&font));
    let (tw, th) = layout.pixel_size();

    cr.set_source_rgba(r, g, b, a);
    cr.move_to(cx - f64::from(tw) / 2.0, cy - f64::from(th) / 2.0);
    pangocairo::functions::show_layout(cr, &layout);
}

/// Connect a draw handler on the event_box that paints a notification
/// indicator in the top-right area when a notification is active.
pub fn setup_notification_bubble(
    state: &Rc<BubbleState>,
    event_box: &gtk::EventBox,
    colors: BubbleColors,
    indicator: Option<String>,
) {
    let state = state.clone();
    let indicator = indicator.unwrap_or_else(|| DEFAULT_INDICATOR.to_owned());
    event_box.connect_draw(move |widget, cr| {
        if !state.active.get() {
            return Propagation::Proceed;
        }
        let alloc = widget.allocation();
        let w = f64::from(alloc.width());
        let h = f64::from(alloc.height());
        let size = font_size(h);
        let cx = w - size / 2.0;
        let cy = (h * TOP_INSET_RATIO) + size / 2.0;
        let (r, g, b, a) = state.urgency.get().resolve(&colors);
        draw_indicator(cr, widget, cx, cy, size, &indicator, r, g, b, a);
        Propagation::Proceed
    });
}

pub fn mark_notification_urgent(
    event_box: &gtk::EventBox,
    state: &BubbleState,
    urgency: NotificationUrgency,
) {
    state.active.set(true);
    state.urgency.set(urgency);
    let h = f64::from(event_box.allocation().height());
    event_box.set_margin_end(indicator_margin(h));
    event_box.queue_draw();
}

pub fn clear_notification_urgent(event_box: &gtk::EventBox, state: &BubbleState) {
    if state.active.get() {
        state.active.set(false);
        event_box.set_margin_end(0);
        event_box.queue_draw();
    }
}
