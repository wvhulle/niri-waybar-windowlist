use std::{cell::Cell, rc::Rc};

use waybar_cffi::gtk::{
    self as gtk,
    prelude::{LabelExt, WidgetExt},
};

use crate::{audio::SinkInput, button::WindowButton, theme::IndicatorColor};

impl WindowButton {
    pub(crate) fn setup_border_indicator(
        indicator_color: &Rc<Cell<Option<IndicatorColor>>>,
        event_box: &gtk::EventBox,
    ) {
        let indicator_for_draw = indicator_color.clone();
        event_box.connect_draw(move |widget, cr| {
            if let Some(color) = indicator_for_draw.get() {
                let w = widget.allocation().width() as f64;
                let h = 3.0;
                match color {
                    IndicatorColor::Solid(rgba) => {
                        cr.set_source_rgba(
                            rgba.red(),
                            rgba.green(),
                            rgba.blue(),
                            rgba.alpha(),
                        );
                    }
                    IndicatorColor::Gradient { from, to } => {
                        let gradient = gtk::cairo::LinearGradient::new(0.0, 0.0, w, 0.0);
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
            gtk::glib::Propagation::Proceed
        });
    }

    pub fn update_audio_state(&self, sink_inputs: &[SinkInput]) {
        if !self.state.settings().audio_indicator().enabled {
            return;
        }

        if sink_inputs.is_empty() {
            self.audio_event_box.hide();
            self.audio_sink_inputs.borrow_mut().clear();
            return;
        }

        let all_muted = sink_inputs.iter().all(|s| s.muted);
        let config = self.state.settings().audio_indicator();
        let icon = if all_muted {
            config.muted_icon.as_str()
        } else {
            config.playing_icon.as_str()
        };

        self.audio_label.set_text(icon);
        self.audio_event_box.show();

        *self.audio_sink_inputs.borrow_mut() = sink_inputs.to_vec();
    }

    pub(crate) fn setup_audio_click_handler(&self) {
        let config = self.state.settings().audio_indicator();
        if !config.enabled || !config.clickable {
            return;
        }

        let sink_inputs_ref = self.audio_sink_inputs.clone();
        self.audio_event_box
            .connect_button_press_event(move |_, event| {
                if event.button() == 1 {
                    let inputs = sink_inputs_ref.borrow().clone();
                    if !inputs.is_empty() {
                        crate::audio::toggle_mute(&inputs);
                    }
                    gtk::glib::Propagation::Stop
                } else {
                    gtk::glib::Propagation::Proceed
                }
            });

        // Absorb release events so they don't propagate to the parent
        // event_box and trigger window focus/click actions.
        self.audio_event_box
            .connect_button_release_event(|_, event| {
                if event.button() == 1 {
                    gtk::glib::Propagation::Stop
                } else {
                    gtk::glib::Propagation::Proceed
                }
            });

        self.audio_event_box.set_tooltip_text(Some("Toggle mute"));
    }
}
