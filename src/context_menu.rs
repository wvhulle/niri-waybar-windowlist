use std::process::Command;
use waybar_cffi::gtk::{
    Menu, MenuItem,
    gdk,
    prelude::{GtkMenuExt, GtkMenuItemExt, MenuShellExt, WidgetExt},
};
use crate::settings::ModifierKey;
use crate::taskbar::clear_selection;
use crate::button::WindowButton;

impl WindowButton {
    #[tracing::instrument(level = "TRACE", skip(self))]
    pub(crate) fn display_context_menu(&self, window_id: u64) {
        let menu = Menu::new();
        menu.set_reserve_toggle_size(false);

        let menu_items = self.state.settings().context_menu();

        for menu_item in menu_items {
            let item = MenuItem::with_label(&menu_item.label);
            menu.append(&item);

            let state = self.state.clone();
            let action = menu_item.action.clone();
            let command = menu_item.command.clone();
            let app_id = self.app_id.clone();
            let title = self.title.borrow().clone();
            item.connect_activate(move |_| {
                if let Some(ref cmd) = command {
                    Self::execute_command(cmd, window_id, app_id.as_deref(), title.as_deref());
                } else if let Some(ref act) = action {
                    Self::execute_action(&state, window_id, act, app_id.as_deref(), title.as_deref());
                }
            });
        }

        menu.show_all();
        menu.popup_at_pointer(None);
    }

    pub(crate) fn display_multi_select_menu(&self) {
        let menu = Menu::new();
        menu.set_reserve_toggle_size(false);

        let menu_items = self.state.settings().multi_select_menu();
        let selected_windows: Vec<u64> = self.selection.borrow().keys().copied().collect();

        for menu_item in menu_items {
            let item = MenuItem::with_label(&menu_item.label);
            menu.append(&item);

            let state = self.state.clone();
            let selection = self.selection.clone();
            let action = menu_item.action.clone();
            let command = menu_item.command.clone();
            let windows = selected_windows.clone();
            item.connect_activate(move |_| {
                if let Some(ref cmd) = command {
                    let windows_str = windows.iter().map(|w| w.to_string()).collect::<Vec<_>>().join(",");
                    let cmd = cmd.replace("{window_ids}", &windows_str);
                    std::thread::spawn(move || {
                        if let Err(e) = Command::new("sh").arg("-c").arg(&cmd).spawn() {
                            tracing::error!(%e, "failed to execute multi-select command");
                        }
                    });
                } else if let Some(ref act) = action {
                    Self::execute_multi_select_action(&state, &windows, act);
                }
                clear_selection(&selection);
            });
        }

        menu.show_all();
        menu.popup_at_pointer(None);
    }

    pub(crate) fn check_modifier_from_event(event: &gdk::EventButton, modifier: ModifierKey) -> bool {
        let state = event.state();
        match modifier {
            ModifierKey::Ctrl => state.contains(gdk::ModifierType::CONTROL_MASK),
            ModifierKey::Shift => state.contains(gdk::ModifierType::SHIFT_MASK),
            ModifierKey::Alt => state.contains(gdk::ModifierType::MOD1_MASK),
            ModifierKey::Super => state.contains(gdk::ModifierType::SUPER_MASK),
        }
    }

    pub(crate) fn check_modifier_static(modifier: ModifierKey) -> bool {
        let display = match gdk::Display::default() {
            Some(d) => d,
            None => return false,
        };
        let keymap = match gdk::Keymap::for_display(&display) {
            Some(k) => k,
            None => return false,
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
