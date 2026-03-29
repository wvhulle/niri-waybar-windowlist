use std::process::Command;
use std::thread;

use waybar_cffi::gtk::prelude::{GtkMenuExt, GtkMenuItemExt, MenuShellExt, WidgetExt};
use waybar_cffi::gtk::{Menu, MenuItem};

use crate::window_button::focus_click::{
    execute_action, execute_command, execute_multi_select_action,
};
use crate::window_list::{clear_selection, SelectionState};
use crate::SharedState;

pub(crate) fn display_context_menu(
    state: &SharedState,
    window_id: u64,
    app_id: Option<&str>,
    title: Option<&str>,
) {
    let menu = Menu::new();
    menu.set_reserve_toggle_size(false);

    let menu_items = state.settings().context_menu();

    for menu_item in menu_items {
        let item = MenuItem::with_label(&menu_item.label);
        menu.append(&item);

        let state = state.clone();
        let action = menu_item.action.clone();
        let command = menu_item.command.clone();
        let app_id = app_id.map(String::from);
        let title = title.map(String::from);
        item.connect_activate(move |_| {
            if let Some(ref cmd) = command {
                execute_command(cmd, window_id, app_id.as_deref(), title.as_deref());
            } else if let Some(ref act) = action {
                execute_action(&state, window_id, act, app_id.as_deref(), title.as_deref());
            }
        });
    }

    menu.show_all();
    menu.popup_at_pointer(None);
}

pub(crate) fn display_multi_select_menu(
    state: &SharedState,
    selection: &SelectionState,
) {
    let menu = Menu::new();
    menu.set_reserve_toggle_size(false);

    let menu_items = state.settings().multi_select_menu();
    let selected_windows: Vec<u64> = selection.borrow().keys().copied().collect();

    for menu_item in menu_items {
        let item = MenuItem::with_label(&menu_item.label);
        menu.append(&item);

        let state = state.clone();
        let selection = selection.clone();
        let action = menu_item.action.clone();
        let command = menu_item.command.clone();
        let windows = selected_windows.clone();
        item.connect_activate(move |_| {
            if let Some(ref cmd) = command {
                let windows_str = windows
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                let cmd = cmd.replace("{window_ids}", &windows_str);
                thread::spawn(move || {
                    if let Err(e) = Command::new("sh").arg("-c").arg(&cmd).spawn() {
                        tracing::error!(%e, "failed to execute multi-select command");
                    }
                });
            } else if let Some(ref act) = action {
                execute_multi_select_action(&state, &windows, act);
            }
            clear_selection(&selection);
        });
    }

    menu.show_all();
    menu.popup_at_pointer(None);
}
