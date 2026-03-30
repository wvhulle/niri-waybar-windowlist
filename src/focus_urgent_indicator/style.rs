use std::{cell::Cell, rc::Rc};

use waybar_cffi::gtk::{self as gtk, cairo::LinearGradient, glib::Propagation, prelude::WidgetExt};

use crate::niri::border_colors::{BorderColors, IndicatorColor};

#[derive(Clone)]
pub struct Indicator {
    color: Rc<Cell<Option<IndicatorColor>>>,
    focused_window: Rc<Cell<Option<u64>>>,
    event_box: gtk::EventBox,
    window_id: u64,
}

impl Indicator {
    pub fn new(event_box: &gtk::EventBox, window_id: u64, focused_window: Rc<Cell<Option<u64>>>) -> Self {
        let color: Rc<Cell<Option<IndicatorColor>>> = Rc::new(Cell::new(None));
        let color_for_draw = color.clone();
        event_box.connect_draw(move |widget, cr| {
            if let Some(c) = color_for_draw.get() {
                let w = f64::from(widget.allocation().width());
                let h = 3.0;
                match c {
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
        Self {
            color,
            focused_window,
            event_box: event_box.clone(),
            window_id,
        }
    }

    pub fn is_focused(&self) -> bool {
        self.focused_window.get() == Some(self.window_id)
    }

    pub fn update(&self, border_colors: &BorderColors, is_focused: bool, is_urgent: bool) {
        if is_focused {
            self.focused_window.set(Some(self.window_id));
        }
        let color = if is_focused {
            Some(border_colors.active)
        } else if is_urgent {
            Some(border_colors.urgent)
        } else {
            None
        };
        self.color.set(color);
        self.event_box.queue_draw();
    }
}
