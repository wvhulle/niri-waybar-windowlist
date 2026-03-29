use waybar_cffi::gtk::gdk;

use super::{settings::ModifierKey, WindowButton};
use crate::right_click_menu::entry_generation;

impl WindowButton {
    #[tracing::instrument(level = "TRACE", skip(self))]
    pub(crate) fn display_context_menu(&self, window_id: u64) {
        entry_generation::display_context_menu(
            &self.state,
            window_id,
            self.app_id.as_deref(),
            self.title.borrow().as_deref(),
        );
    }

    pub(crate) fn display_multi_select_menu(&self) {
        entry_generation::display_multi_select_menu(&self.state, &self.selection);
    }

    pub(crate) fn check_modifier_from_event(
        event: &gdk::EventButton,
        modifier: ModifierKey,
    ) -> bool {
        let state = event.state();
        match modifier {
            ModifierKey::Ctrl => state.contains(gdk::ModifierType::CONTROL_MASK),
            ModifierKey::Shift => state.contains(gdk::ModifierType::SHIFT_MASK),
            ModifierKey::Alt => state.contains(gdk::ModifierType::MOD1_MASK),
            ModifierKey::Super => state.contains(gdk::ModifierType::SUPER_MASK),
        }
    }

    pub(crate) fn check_modifier_static(modifier: ModifierKey) -> bool {
        let Some(display) = gdk::Display::default() else {
            return false;
        };
        let Some(keymap) = gdk::Keymap::for_display(&display) else {
            return false;
        };
        let state = gdk::ModifierType::from_bits_truncate(keymap.modifier_state());
        match modifier {
            ModifierKey::Ctrl => state.contains(gdk::ModifierType::CONTROL_MASK),
            ModifierKey::Shift => state.contains(gdk::ModifierType::SHIFT_MASK),
            ModifierKey::Alt => state.contains(gdk::ModifierType::MOD1_MASK),
            ModifierKey::Super => state.contains(gdk::ModifierType::SUPER_MASK),
        }
    }
}
