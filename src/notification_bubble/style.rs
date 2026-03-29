use std::{cell::Cell, f64::consts::TAU, rc::Rc};

use waybar_cffi::gtk::{self as gtk, cairo, glib::Propagation, prelude::WidgetExt};

const BUBBLE_SIZE: i32 = 8;

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

    fn rgba(self) -> (f64, f64, f64, f64) {
        match self {
            Self::Low => (0.6, 0.6, 0.6, 0.7),
            Self::Normal => (0.3, 0.6, 1.0, 1.0),
            Self::Critical => (1.0, 0.3, 0.2, 1.0),
        }
    }
}

pub fn create_notification_bubble(urgency: Rc<Cell<NotificationUrgency>>) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_size_request(BUBBLE_SIZE, BUBBLE_SIZE);
    area.set_valign(gtk::Align::Center);
    area.set_app_paintable(true);
    area.set_no_show_all(true);
    area.connect_draw(move |widget, cr| {
        // Clear to transparent so only the circle is visible, not a background
        // rectangle.
        cr.set_operator(cairo::Operator::Clear);
        cr.paint().ok();
        cr.set_operator(cairo::Operator::Over);

        let w = f64::from(widget.allocation().width());
        let h = f64::from(widget.allocation().height());
        let radius = w.min(h) / 2.0;
        let (r, g, b, a) = urgency.get().rgba();
        cr.arc(w / 2.0, h / 2.0, radius, 0.0, TAU);
        cr.set_source_rgba(r, g, b, a);
        cr.fill().ok();
        Propagation::Proceed
    });
    area
}

pub fn mark_notification_urgent(
    bubble: &gtk::DrawingArea,
    notification_urgency: &Cell<bool>,
    urgency_level: &Rc<Cell<NotificationUrgency>>,
    urgency: NotificationUrgency,
) {
    notification_urgency.set(true);
    urgency_level.set(urgency);
    bubble.show();
    bubble.queue_draw();
}

pub fn clear_notification_urgent(bubble: &gtk::DrawingArea, notification_urgency: &Cell<bool>) {
    notification_urgency.set(false);
    bubble.hide();
}
