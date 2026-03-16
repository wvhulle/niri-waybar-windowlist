use std::{cell::Cell, rc::Rc};

use waybar_cffi::gtk::{
    self as gtk,
    prelude::{LabelExt, WidgetExt},
};

use crate::{audio::PlaybackStatus, button::WindowButton, theme::IndicatorColor};

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
                        cr.set_source_rgba(rgba.red(), rgba.green(), rgba.blue(), rgba.alpha());
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

    pub fn update_audio_state(&self, status: Option<PlaybackStatus>) {
        if !self.state.settings().audio_indicator().enabled {
            return;
        }

        match status {
            None | Some(PlaybackStatus::Stopped) => {
                self.audio_event_box.hide();
            }
            Some(PlaybackStatus::Playing) => {
                let config = self.state.settings().audio_indicator();
                self.audio_label.set_text(config.playing_icon.as_str());
                self.audio_event_box.show();
            }
            Some(PlaybackStatus::Paused) => {
                let config = self.state.settings().audio_indicator();
                self.audio_label.set_text(config.muted_icon.as_str());
                self.audio_event_box.show();
            }
        }
    }
}
