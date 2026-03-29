use std::{cell::Cell, f64::consts::TAU, rc::Rc};

use waybar_cffi::gtk::{self as gtk, glib::Propagation, prelude::WidgetExt};

use super::settings::{BubbleColors};

const BUBBLE_RADIUS: f64 = 4.0;

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


/// Connect a draw handler on the event_box that paints a small circle at the
/// top-right corner when a notification is active.
pub fn setup_notification_bubble(
    state: &Rc<BubbleState>,
    event_box: &gtk::EventBox,
    colors: BubbleColors,
) {
    let state = state.clone();
    event_box.connect_draw(move |widget, cr| {
        if !state.active.get() {
            return Propagation::Proceed;
        }
        let w = f64::from(widget.allocation().width());
        let (r, g, b, a) = state.urgency.get().resolve(&colors);
        let cx = w - BUBBLE_RADIUS - 1.0;
        let cy = BUBBLE_RADIUS + 1.0;
        cr.arc(cx, cy, BUBBLE_RADIUS, 0.0, TAU);
        cr.set_source_rgba(r, g, b, a);
        cr.fill().ok();
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
    event_box.queue_draw();
}

pub fn clear_notification_urgent(event_box: &gtk::EventBox, state: &BubbleState) {
    if state.active.get() {
        state.active.set(false);
        event_box.queue_draw();
    }
}
