use std::{process::Command, thread};

use niri_ipc::WorkspaceReferenceArg;
use waybar_cffi::gtk::{
    glib,
    prelude::{BoxExt, ContainerExt, GtkMenuExt, GtkMenuItemExt, LabelExt, MenuShellExt, WidgetExt},
    IconSize, Image, Label, Menu, MenuItem, Orientation, SeparatorMenuItem,
};

use crate::{
    niri::CompositorClient,
    window_button::focus_click::{execute_action, execute_command, execute_multi_select_action},
    window_list::{clear_selection, SelectionState},
    SharedState,
};

const ICON_LABEL_SPACING: i32 = 6;

fn create_menu_item(icon_name: Option<&str>, label: &str) -> MenuItem {
    let item = MenuItem::new();
    let hbox = waybar_cffi::gtk::Box::new(Orientation::Horizontal, ICON_LABEL_SPACING);

    let image = match icon_name {
        Some(name) => Image::from_icon_name(Some(name), IconSize::Menu),
        None => Image::new(),
    };
    hbox.pack_start(&image, false, false, 0);

    let text_label = Label::new(Some(label));
    hbox.pack_start(&text_label, false, false, 0);

    item.add(&hbox);
    item
}

pub(crate) fn display_context_menu(
    state: &SharedState,
    window_id: u64,
    app_id: Option<&str>,
    title: Option<&str>,
) {
    let menu = Menu::new();
    menu.set_reserve_toggle_size(false);

    let groups = state.settings.context_menu();

    for (group_idx, group) in groups.iter().enumerate() {
        if group_idx > 0 {
            menu.append(&SeparatorMenuItem::new());
        }

        for menu_item in group {
            let item = create_menu_item(menu_item.icon.as_deref(), &menu_item.label);
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
    }

    append_workspace_submenu(&menu, state, window_id);

    menu.show_all();
    menu.popup_at_pointer(None);
}

fn append_workspace_submenu(menu: &Menu, state: &SharedState, window_id: u64) {
    let workspaces = match CompositorClient::query_workspaces() {
        Ok(ws) => ws,
        Err(e) => {
            tracing::warn!(%e, "failed to query workspaces for context menu");
            return;
        }
    };

    if workspaces.len() <= 1 {
        return;
    }

    menu.append(&SeparatorMenuItem::new());

    let submenu_item = create_menu_item(Some("go-next-symbolic"), "Move to Workspace");
    let submenu = Menu::new();
    submenu.set_reserve_toggle_size(false);

    let mut sorted_workspaces: Vec<_> = workspaces.into_iter().collect();
    sorted_workspaces.sort_by_key(|ws| (ws.output.clone(), ws.idx));

    for ws in sorted_workspaces {
        let markup = match ws.name.as_deref() {
            Some(name) => format!("{}. {}", ws.idx, glib::markup_escape_text(name)),
            None => format!(
                "<span alpha=\"50%\">{}. <i>unnamed</i></span>",
                ws.idx
            ),
        };

        let item = MenuItem::new();
        let label = Label::new(None);
        label.set_use_markup(true);
        label.set_markup(&markup);
        label.set_xalign(0.0);
        item.add(&label);
        submenu.append(&item);

        let state = state.clone();
        let ws_id = ws.id;
        item.connect_activate(move |_| {
            if let Err(e) = state
                .compositor
                .move_window_to_workspace(window_id, WorkspaceReferenceArg::Id(ws_id))
            {
                tracing::warn!(%e, window_id, ws_id, "move to workspace failed");
            }
        });
    }

    submenu_item.set_submenu(Some(&submenu));
    menu.append(&submenu_item);
}

pub(crate) fn display_multi_select_menu(state: &SharedState, selection: &SelectionState) {
    let menu = Menu::new();
    menu.set_reserve_toggle_size(false);

    let menu_items = state.settings.multi_select_menu();
    let selected_windows: Vec<u64> = selection.borrow().keys().copied().collect();

    for menu_item in menu_items {
        let item = create_menu_item(menu_item.icon.as_deref(), &menu_item.label);
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
