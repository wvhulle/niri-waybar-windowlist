use std::{
    cell::{Cell, RefCell},
    collections::hash_map::Entry,
    process::Command,
    rc::Rc,
    thread,
    time::{Duration, Instant},
};

use gtk::glib::Propagation;
use waybar_cffi::gtk::{
    self as gtk,
    gdk::{self, ScrollDirection},
    prelude::{Cast, ContainerExt, WidgetExt},
};

use super::{
    hover_mouse::set_background_color,
    settings::{ClickAction, ClickActions, MultiSelectAction, WindowAction},
    WindowButton,
};
use crate::{
    niri::border_colors::IndicatorColor,
    window_list::{clear_selection, FocusedWindow},
    SharedState,
};

impl WindowButton {
    pub(crate) fn setup_click_handlers(&self, window_id: u64) {
        self.setup_release_handler(window_id);
        self.setup_press_handler(window_id);
        self.setup_scroll_handler(window_id);
    }

    fn setup_release_handler(&self, window_id: u64) {
        let skip_release = self.skip_clicked.clone();
        let state_release = self.state.clone();
        let app_id_release = self.app_id.clone();
        let title_release = self.title.clone();
        let selection_release = self.selection.clone();
        let focused_release = self.focused_window.clone();
        let indicator_color_release = self.indicator_color.clone();
        let last_click_release = Rc::new(RefCell::new(
            Instant::now().checked_sub(Duration::from_secs(1)).unwrap(),
        ));

        self.event_box
            .connect_button_release_event(move |btn, event| {
                if event.button() == 1 {
                    if *skip_release.borrow() {
                        *skip_release.borrow_mut() = false;
                        return Propagation::Stop;
                    }

                    let is_currently_focused = focused_release.get() == Some(window_id);
                    let app_id_ref = app_id_release.as_deref();
                    let title_ref = title_release.borrow();
                    let title_str = title_ref.as_deref();
                    let actions = state_release
                        .settings
                        .get_click_actions(app_id_ref, title_str);

                    if is_currently_focused {
                        let mut last_click = last_click_release.borrow_mut();
                        let now = Instant::now();
                        let time_since_last = now.duration_since(*last_click);

                        if time_since_last < Duration::from_millis(300) {
                            clear_selection(&selection_release);
                            execute_click_action(
                                &state_release,
                                window_id,
                                &actions.double_click,
                                app_id_ref,
                                title_str,
                            );
                            *last_click =
                                Instant::now().checked_sub(Duration::from_secs(1)).unwrap();
                        } else {
                            clear_selection(&selection_release);
                            execute_click_action(
                                &state_release,
                                window_id,
                                &actions.left_click_focused,
                                app_id_ref,
                                title_str,
                            );
                            *last_click = now;
                        }
                    } else {
                        clear_selection(&selection_release);
                        Self::optimistic_focus(
                            btn,
                            window_id,
                            &focused_release,
                            &indicator_color_release,
                            &state_release,
                        );
                        execute_click_action(
                            &state_release,
                            window_id,
                            &actions.left_click_unfocused,
                            app_id_ref,
                            title_str,
                        );
                    }
                    Propagation::Stop
                } else {
                    Propagation::Proceed
                }
            });
    }

    fn setup_press_handler(&self, window_id: u64) {
        let state_press = self.state.clone();
        let event_box_press = self.event_box.clone();
        let app_id_press = self.app_id.clone();
        let title_press = self.title.clone();
        let selection_press = self.selection.clone();
        let menu_self = self.clone_for_menu();
        let focused_press = self.focused_window.clone();
        let indicator_color_press = self.indicator_color.clone();
        let skip_press = self.skip_clicked.clone();
        let selected_bg = gdk::RGBA::new(0.5, 0.5, 0.5, 0.3);

        self.event_box
            .connect_button_press_event(move |btn, event| match event.button() {
                1 => {
                    let modifier_held = Self::check_modifier_from_event(
                        event,
                        state_press.settings.multi_select_modifier(),
                    );
                    if modifier_held {
                        *skip_press.borrow_mut() = true;
                        let mut sel = selection_press.borrow_mut();
                        if let Entry::Vacant(e) = sel.entry(window_id) {
                            e.insert(event_box_press.clone());
                            set_background_color(&event_box_press, Some(&selected_bg));
                        } else {
                            sel.remove(&window_id);
                            set_background_color(&event_box_press, None);
                        }
                    } else {
                        let is_currently_focused = focused_press.get() == Some(window_id);
                        if !is_currently_focused {
                            *skip_press.borrow_mut() = true;
                            clear_selection(&selection_press);
                            Self::optimistic_focus(
                                btn,
                                window_id,
                                &focused_press,
                                &indicator_color_press,
                                &state_press,
                            );
                            let app_id_ref = app_id_press.as_deref();
                            let title_ref = title_press.borrow();
                            let title_str = title_ref.as_deref();
                            let actions = state_press
                                .settings
                                .get_click_actions(app_id_ref, title_str);
                            execute_click_action(
                                &state_press,
                                window_id,
                                &actions.left_click_unfocused,
                                app_id_ref,
                                title_str,
                            );
                        }
                    }
                    Propagation::Proceed
                }
                2 => {
                    let is_focused = focused_press.get() == Some(window_id);
                    execute_or_show_menu(
                        &menu_self,
                        &state_press,
                        window_id,
                        app_id_press.as_deref(),
                        &title_press,
                        |a| {
                            if is_focused {
                                &a.middle_click_focused
                            } else {
                                &a.middle_click_unfocused
                            }
                        },
                    );
                    Propagation::Stop
                }
                3 => {
                    if selection_press.borrow().is_empty() {
                        let is_focused = focused_press.get() == Some(window_id);
                        execute_or_show_menu(
                            &menu_self,
                            &state_press,
                            window_id,
                            app_id_press.as_deref(),
                            &title_press,
                            |a| {
                                if is_focused {
                                    &a.right_click_focused
                                } else {
                                    &a.right_click_unfocused
                                }
                            },
                        );
                    } else {
                        menu_self.display_multi_select_menu();
                    }
                    Propagation::Stop
                }
                _ => Propagation::Proceed,
            });
    }

    fn setup_scroll_handler(&self, window_id: u64) {
        let state_scroll = self.state.clone();
        let app_id_scroll = self.app_id.clone();
        let title_scroll = self.title.clone();
        self.event_box.connect_scroll_event(move |_, event| {
            let app_id_ref = app_id_scroll.as_deref();
            let title_ref = title_scroll.borrow();
            let title_str = title_ref.as_deref();
            let actions = state_scroll
                .settings
                .get_click_actions(app_id_ref, title_str);

            let action = match event.direction() {
                ScrollDirection::Up => &actions.scroll_up,
                ScrollDirection::Down => &actions.scroll_down,
                ScrollDirection::Smooth => {
                    let (delta_x, delta_y) = event.delta();
                    let delta = if delta_x.abs() > delta_y.abs() {
                        delta_x
                    } else {
                        delta_y
                    };
                    if delta < -0.01 {
                        &actions.scroll_up
                    } else if delta > 0.01 {
                        &actions.scroll_down
                    } else {
                        return Propagation::Stop;
                    }
                }
                _ => return Propagation::Stop,
            };

            if !action.is_none() {
                execute_click_action(&state_scroll, window_id, action, app_id_ref, title_str);
            }
            Propagation::Stop
        });
    }

    pub(crate) fn optimistic_focus(
        btn: &gtk::EventBox,
        window_id: u64,
        focused_window: &FocusedWindow,
        indicator_color: &Rc<Cell<Option<IndicatorColor>>>,
        state: &SharedState,
    ) {
        if let Some(parent) = btn.parent() {
            if let Ok(container) = parent.downcast::<gtk::Box>() {
                for child in container.children() {
                    if let Ok(child_eb) = child.downcast::<gtk::EventBox>() {
                        child_eb.queue_draw();
                    }
                }
            }
        }

        let colors = *state.border_colors.lock().unwrap();
        indicator_color.set(Some(colors.active));
        btn.queue_draw();
        focused_window.set(Some(window_id));
    }
}

fn execute_or_show_menu(
    button: &WindowButton,
    state: &SharedState,
    window_id: u64,
    app_id: Option<&str>,
    title: &Rc<RefCell<Option<String>>>,
    pick: impl FnOnce(&ClickActions) -> &ClickAction,
) {
    let title_ref = title.borrow();
    let title_str = title_ref.as_deref();
    let actions = state.settings.get_click_actions(app_id, title_str);
    let action = pick(&actions);
    if action.is_menu() {
        button.display_context_menu(window_id);
    } else {
        execute_click_action(state, window_id, action, app_id, title_str);
    }
}

pub(crate) fn execute_click_action(
    state: &SharedState,
    window_id: u64,
    action: &ClickAction,
    app_id: Option<&str>,
    title: Option<&str>,
) {
    match action {
        ClickAction::Action(window_action) => {
            execute_action(state, window_id, window_action, app_id, title);
        }
        ClickAction::Command { command } => {
            execute_command(command, window_id, app_id, title);
        }
    }
}

pub(crate) fn execute_action(
    state: &SharedState,
    window_id: u64,
    action: &WindowAction,
    _app_id: Option<&str>,
    _title: Option<&str>,
) {
    let result = match action {
        WindowAction::FocusWindow => state.compositor.focus_window(window_id),
        WindowAction::CloseWindow => state.compositor.close_window(window_id),
        WindowAction::MaximizeColumn => state.compositor.maximize_window_column(window_id),
        WindowAction::MaximizeWindowToEdges => state.compositor.maximize_window_to_edges(window_id),
        WindowAction::CenterColumn => state.compositor.center_column(window_id),
        WindowAction::CenterWindow => state.compositor.center_window(window_id),
        WindowAction::CenterVisibleColumns => state.compositor.center_visible_columns(window_id),
        WindowAction::ExpandColumnToAvailableWidth => {
            state.compositor.expand_column_to_available_width(window_id)
        }
        WindowAction::FullscreenWindow => state.compositor.fullscreen_window(window_id),
        WindowAction::ToggleWindowedFullscreen => {
            state.compositor.toggle_windowed_fullscreen(window_id)
        }
        WindowAction::ToggleWindowFloating => state.compositor.toggle_floating(window_id),
        WindowAction::ConsumeWindowIntoColumn => {
            state.compositor.consume_window_into_column(window_id)
        }
        WindowAction::ExpelWindowFromColumn => state.compositor.expel_window_from_column(window_id),
        WindowAction::ResetWindowHeight => state.compositor.reset_window_height(window_id),
        WindowAction::SwitchPresetColumnWidth => {
            state.compositor.switch_preset_column_width(window_id)
        }
        WindowAction::SwitchPresetWindowHeight => {
            state.compositor.switch_preset_window_height(window_id)
        }
        WindowAction::MoveWindowToWorkspaceDown => {
            state.compositor.move_window_to_workspace_down(window_id)
        }
        WindowAction::MoveWindowToWorkspaceUp => {
            state.compositor.move_window_to_workspace_up(window_id)
        }
        WindowAction::MoveWindowToMonitorLeft => {
            state.compositor.move_window_to_monitor_left(window_id)
        }
        WindowAction::MoveWindowToMonitorRight => {
            state.compositor.move_window_to_monitor_right(window_id)
        }
        WindowAction::ToggleColumnTabbedDisplay => {
            state.compositor.toggle_column_tabbed_display(window_id)
        }
        WindowAction::FocusWorkspacePrevious => {
            state.compositor.focus_workspace_previous(window_id)
        }
        WindowAction::MoveColumnLeft => state.compositor.move_column_left(window_id),
        WindowAction::MoveColumnRight => state.compositor.move_column_right(window_id),
        WindowAction::MoveColumnToFirst => state.compositor.move_column_to_first(window_id),
        WindowAction::MoveColumnToLast => state.compositor.move_column_to_last(window_id),
        WindowAction::MoveWindowDown => state.compositor.move_window_down(window_id),
        WindowAction::MoveWindowUp => state.compositor.move_window_up(window_id),
        WindowAction::MoveWindowDownOrToWorkspaceDown => state
            .compositor
            .move_window_down_or_to_workspace_down(window_id),
        WindowAction::MoveWindowUpOrToWorkspaceUp => state
            .compositor
            .move_window_up_or_to_workspace_up(window_id),
        WindowAction::MoveColumnLeftOrToMonitorLeft => state
            .compositor
            .move_column_left_or_to_monitor_left(window_id),
        WindowAction::MoveColumnRightOrToMonitorRight => state
            .compositor
            .move_column_right_or_to_monitor_right(window_id),
        WindowAction::None | WindowAction::Menu => return,
    };

    if let Err(e) = result {
        tracing::warn!(%e, id = window_id, action = ?action, "action failed");
    }
}

pub(crate) fn execute_command(
    command: &str,
    window_id: u64,
    app_id: Option<&str>,
    title: Option<&str>,
) {
    let cmd = command
        .replace("{window_id}", &window_id.to_string())
        .replace("{app_id}", app_id.unwrap_or(""))
        .replace("{title}", title.unwrap_or(""));

    thread::spawn(move || {
        if let Err(e) = Command::new("sh").arg("-c").arg(&cmd).spawn() {
            tracing::error!(%e, "failed to execute command: {}", cmd);
        }
    });
}

pub(crate) fn execute_multi_select_action(
    state: &SharedState,
    window_ids: &[u64],
    action: &MultiSelectAction,
) {
    for &window_id in window_ids {
        let result = match action {
            MultiSelectAction::CloseWindows => state.compositor.close_window(window_id),
            MultiSelectAction::MoveToWorkspaceUp => {
                state.compositor.move_window_to_workspace_up(window_id)
            }
            MultiSelectAction::MoveToWorkspaceDown => {
                state.compositor.move_window_to_workspace_down(window_id)
            }
            MultiSelectAction::MoveToMonitorLeft => {
                state.compositor.move_window_to_monitor_left(window_id)
            }
            MultiSelectAction::MoveToMonitorRight => {
                state.compositor.move_window_to_monitor_right(window_id)
            }
            MultiSelectAction::MoveToMonitorUp => {
                state.compositor.move_window_to_monitor_up(window_id)
            }
            MultiSelectAction::MoveToMonitorDown => {
                state.compositor.move_window_to_monitor_down(window_id)
            }
            MultiSelectAction::MoveColumnLeft => state.compositor.move_column_left(window_id),
            MultiSelectAction::MoveColumnRight => state.compositor.move_column_right(window_id),
            MultiSelectAction::ToggleFloating => state.compositor.toggle_floating(window_id),
            MultiSelectAction::FullscreenWindows => state.compositor.fullscreen_window(window_id),
            MultiSelectAction::MaximizeColumns => {
                state.compositor.maximize_window_column(window_id)
            }
            MultiSelectAction::CenterColumns => state.compositor.center_column(window_id),
            MultiSelectAction::ConsumeIntoColumn => {
                state.compositor.consume_window_into_column(window_id)
            }
            MultiSelectAction::ToggleTabbedDisplay => {
                state.compositor.toggle_column_tabbed_display(window_id)
            }
        };
        if let Err(e) = result {
            tracing::warn!(%e, id = window_id, "multi-select action failed");
        }
    }
}
