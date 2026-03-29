use std::cell::Cell;
use std::rc::Rc;

use waybar_cffi::gtk::{self as gtk, cairo::LinearGradient, glib::Propagation, prelude::WidgetExt};

use crate::niri::border_colors::{BorderColors, IndicatorColor};

pub fn setup_border_indicator(
    indicator_color: &Rc<Cell<Option<IndicatorColor>>>,
    event_box: &gtk::EventBox,
) {
    let indicator_for_draw = indicator_color.clone();
    event_box.connect_draw(move |widget, cr| {
        if let Some(color) = indicator_for_draw.get() {
            let w = f64::from(widget.allocation().width());
            let h = 3.0;
            match color {
                IndicatorColor::Solid(rgba) => {
                    cr.set_source_rgba(rgba.red(), rgba.green(), rgba.blue(), rgba.alpha());
                }
                IndicatorColor::Gradient { from, to } => {
                    let gradient = LinearGradient::new(0.0, 0.0, w, 0.0);
                    gradient.add_color_stop_rgba(
                        0.0,
                        to.red(),
                        to.green(),
                        to.blue(),
                        to.alpha(),
                    );
                    gradient.add_color_stop_rgba(
                        0.5,
                        from.red(),
                        from.green(),
                        from.blue(),
                        from.alpha(),
                    );
                    gradient.add_color_stop_rgba(
                        1.0,
                        to.red(),
                        to.green(),
                        to.blue(),
                        to.alpha(),
                    );
                    cr.set_source(&gradient).ok();
                }
            }
            cr.rectangle(0.0, 0.0, w, h);
            cr.fill().ok();
        }
        Propagation::Proceed
    });
}

pub fn update_focus(
    indicator_color: &Rc<Cell<Option<IndicatorColor>>>,
    event_box: &gtk::EventBox,
    border_colors: &BorderColors,
    is_urgent: bool,
    focused_window: &Rc<Cell<Option<u64>>>,
    window_id: u64,
    is_focused: bool,
) {
    if is_focused {
        indicator_color.set(Some(border_colors.active));
        focused_window.set(Some(window_id));
    } else if is_urgent {
        indicator_color.set(Some(border_colors.urgent));
    } else {
        indicator_color.set(None);
    }
    event_box.queue_draw();
}

pub fn mark_urgent(
    indicator_color: &Rc<Cell<Option<IndicatorColor>>>,
    event_box: &gtk::EventBox,
    border_colors: &BorderColors,
    is_urgent: &Cell<bool>,
) {
    is_urgent.set(true);
    indicator_color.set(Some(border_colors.urgent));
    event_box.queue_draw();
}

pub fn update_urgent(
    indicator_color: &Rc<Cell<Option<IndicatorColor>>>,
    event_box: &gtk::EventBox,
    border_colors: &BorderColors,
    is_urgent: &Cell<bool>,
    urgent: bool,
) {
    is_urgent.set(urgent);
    if urgent {
        mark_urgent(indicator_color, event_box, border_colors, is_urgent);
    }
}
