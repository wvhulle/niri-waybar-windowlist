use waybar_cffi::gtk::{self as gtk, prelude::{LabelExt, WidgetExt}};

use super::PlaybackStatus;
use super::settings::AudioIndicatorConfig;

pub fn update_audio_state(
    audio_event_box: &gtk::EventBox,
    audio_label: &gtk::Label,
    config: &AudioIndicatorConfig,
    status: Option<PlaybackStatus>,
) {
    if !config.enabled {
        return;
    }

    match status {
        None | Some(PlaybackStatus::Stopped) => {
            audio_event_box.hide();
        }
        Some(PlaybackStatus::Playing) => {
            audio_label.set_text(config.playing_icon.as_str());
            audio_event_box.show();
        }
        Some(PlaybackStatus::Paused) => {
            audio_label.set_text(config.muted_icon.as_str());
            audio_event_box.show();
        }
    }
}
