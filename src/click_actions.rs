use std::{
    cell::{Cell, RefCell},
    process::Command,
    rc::Rc,
    time::{Duration, Instant},
};

use waybar_cffi::gtk::{
    self as gtk, gdk,
    prelude::{Cast, ContainerExt, WidgetExt},
};

use crate::{
    button::WindowButton,
    settings::MultiSelectAction,
    taskbar::{clear_selection, scroll_taskbar, set_background_color, FocusedWindow},
    SharedState,
};

impl WindowButton {
    pub(crate) fn setup_click_handlers(&self, window_id: u64) {
        let title = self.title.clone();

        let skip_release = self.skip_clicked.clone();
        let state_release = self.state.clone();
        let app_id_release = self.app_id.clone();
        let title_release = self.title.clone();
        let selection_release = self.selection.clone();
        let focused_release = self.focused_window.clone();
        let indicator_color_release = self.indicator_color.clone();
        let last_click_release = Rc::new(RefCell::new(Instant::now() - Duration::from_secs(1)));

        self.event_box
            .connect_button_release_event(move |btn, event| {
                if event.button() == 1 {
                    if *skip_release.borrow() {
                        *skip_release.borrow_mut() = false;
                        return gtk::glib::Propagation::Stop;
                    }

                    let is_currently_focused = focused_release.get() == Some(window_id);
                    let app_id_ref = app_id_release.as_deref();
                    let title_ref = title_release.borrow();
                    let title_str = title_ref.as_deref();
                    let actions = state_release
                        .settings()
                        .get_click_actions(app_id_ref, title_str);

                    if is_currently_focused {
                        let mut last_click = last_click_release.borrow_mut();
                        let now = Instant::now();
                        let time_since_last = now.duration_since(*last_click);

                        if time_since_last < Duration::from_millis(300) {
                            clear_selection(&selection_release);
                            Self::execute_click_action(
                                &state_release,
                                window_id,
                                &actions.double_click,
                                app_id_ref,
                                title_str,
                            );
                            *last_click = Instant::now() - Duration::from_secs(1);
                        } else {
                            clear_selection(&selection_release);
                            Self::execute_click_action(
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
                        Self::execute_click_action(
                            &state_release,
                            window_id,
                            &actions.left_click_unfocused,
                            app_id_ref,
                            title_str,
                        );
                    }
                    gtk::glib::Propagation::Stop
                } else {
                    gtk::glib::Propagation::Proceed
                }
            });

        let state_press = self.state.clone();
        let event_box_press = self.event_box.clone();
        let app_id_press = self.app_id.clone();
        let title_press = title.clone();
        let selection_press = self.selection.clone();
        let menu_self = self.clone_for_menu();
        let focused_press = self.focused_window.clone();
        let indicator_color_press = self.indicator_color.clone();
        let skip_press = self.skip_clicked.clone();
        let selected_bg = gdk::RGBA::new(0.5, 0.5, 0.5, 0.3);

        self.event_box
            .connect_button_press_event(move |btn, event| {
                if event.button() == 1 {
                    let modifier_held = Self::check_modifier_from_event(
                        event,
                        state_press.settings().multi_select_modifier(),
                    );
                    if modifier_held {
                        *skip_press.borrow_mut() = true;
                        let mut sel = selection_press.borrow_mut();
                        if sel.contains_key(&window_id) {
                            sel.remove(&window_id);
                            set_background_color(&event_box_press, None);
                        } else {
                            sel.insert(window_id, event_box_press.clone());
                            set_background_color(&event_box_press, Some(&selected_bg));
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
                                .settings()
                                .get_click_actions(app_id_ref, title_str);
                            Self::execute_click_action(
                                &state_press,
                                window_id,
                                &actions.left_click_unfocused,
                                app_id_ref,
                                title_str,
                            );
                        }
                    }
                    gtk::glib::Propagation::Proceed
                } else if event.button() == 2 {
                    let is_currently_focused = focused_press.get() == Some(window_id);
                    let app_id_ref = app_id_press.as_deref();
                    let title_ref = title_press.borrow();
                    let title_str = title_ref.as_deref();
                    let actions = state_press
                        .settings()
                        .get_click_actions(app_id_ref, title_str);
                    let action = if is_currently_focused {
                        &actions.middle_click_focused
                    } else {
                        &actions.middle_click_unfocused
                    };
                    if action.is_menu() {
                        menu_self.display_context_menu(window_id);
                    } else {
                        Self::execute_click_action(
                            &state_press,
                            window_id,
                            action,
                            app_id_ref,
                            title_str,
                        );
                    }
                    gtk::glib::Propagation::Stop
                } else if event.button() == 3 {
                    let selection_count = selection_press.borrow().len();
                    if selection_count > 0 {
                        menu_self.display_multi_select_menu();
                    } else {
                        let is_currently_focused = focused_press.get() == Some(window_id);
                        let app_id_ref = app_id_press.as_deref();
                        let title_ref = title_press.borrow();
                        let title_str = title_ref.as_deref();
                        let actions = state_press
                            .settings()
                            .get_click_actions(app_id_ref, title_str);
                        let action = if is_currently_focused {
                            &actions.right_click_focused
                        } else {
                            &actions.right_click_unfocused
                        };
                        if action.is_menu() {
                            menu_self.display_context_menu(window_id);
                        } else {
                            Self::execute_click_action(
                                &state_press,
                                window_id,
                                action,
                                app_id_ref,
                                title_str,
                            );
                        }
                    }
                    gtk::glib::Propagation::Stop
                } else {
                    gtk::glib::Propagation::Proceed
                }
            });

        let state_scroll = self.state.clone();
        let app_id_scroll = self.app_id.clone();
        let title_scroll = title.clone();
        self.event_box.connect_scroll_event(move |_, event| {
            use waybar_cffi::gtk::gdk::ScrollDirection;

            let app_id_ref = app_id_scroll.as_deref();
            let title_ref = title_scroll.borrow();
            let title_str = title_ref.as_deref();
            let actions = state_scroll
                .settings()
                .get_click_actions(app_id_ref, title_str);

            let (action, scroll_delta) = match event.direction() {
                ScrollDirection::Up => (&actions.scroll_up, -1.0),
                ScrollDirection::Down => (&actions.scroll_down, 1.0),
                ScrollDirection::Smooth => {
                    let (delta_x, delta_y) = event.delta();
                    let delta = if delta_x.abs() > delta_y.abs() {
                        delta_x
                    } else {
                        delta_y
                    };
                    if delta < -0.01 {
                        (&actions.scroll_up, delta)
                    } else if delta > 0.01 {
                        (&actions.scroll_down, delta)
                    } else {
                        return gtk::glib::Propagation::Stop;
                    }
                }
                _ => return gtk::glib::Propagation::Stop,
            };

            if !action.is_none() {
                Self::execute_click_action(&state_scroll, window_id, action, app_id_ref, title_str);
            } else {
                scroll_taskbar(scroll_delta);
            }
            gtk::glib::Propagation::Stop
        });
    }

    pub(crate) fn optimistic_focus(
        btn: &gtk::EventBox,
        window_id: u64,
        focused_window: &FocusedWindow,
        indicator_color: &Rc<Cell<Option<gdk::RGBA>>>,
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

        let colors = state.border_colors();
        indicator_color.set(Some(colors.active));
        btn.queue_draw();
        focused_window.set(Some(window_id));
    }

    pub(crate) fn execute_click_action(
        state: &SharedState,
        window_id: u64,
        action: &crate::settings::ClickAction,
        app_id: Option<&str>,
        title: Option<&str>,
    ) {
        use crate::settings::ClickAction;
        match action {
            ClickAction::Action(window_action) => {
                Self::execute_action(state, window_id, window_action, app_id, title);
            }
            ClickAction::Command { command } => {
                Self::execute_command(command, window_id, app_id, title);
            }
        }
    }

    pub(crate) fn execute_action(
        state: &SharedState,
        window_id: u64,
        action: &crate::settings::WindowAction,
        app_id: Option<&str>,
        title: Option<&str>,
    ) {
        use crate::settings::WindowAction;
        match action {
            WindowAction::None => {}
            WindowAction::FocusWindow => {
                if let Some(activator) = state.wayland_activator() {
                    activator.activate(app_id, title);
                } else {
                    tracing::warn!(id = window_id, "no Wayland activator available");
                }
            }
            WindowAction::CloseWindow => {
                if let Err(e) = state.compositor().close_window(window_id) {
                    tracing::warn!(%e, id = window_id, "close failed");
                }
            }
            WindowAction::MaximizeColumn => {
                if let Err(e) = state.compositor().maximize_window_column(window_id) {
                    tracing::warn!(%e, id = window_id, "maximize column failed");
                }
            }
            WindowAction::MaximizeWindowToEdges => {
                if let Err(e) = state.compositor().maximize_window_to_edges(window_id) {
                    tracing::warn!(%e, id = window_id, "maximize to edges failed");
                }
            }
            WindowAction::CenterColumn => {
                if let Err(e) = state.compositor().center_column(window_id) {
                    tracing::warn!(%e, id = window_id, "center column failed");
                }
            }
            WindowAction::CenterWindow => {
                if let Err(e) = state.compositor().center_window(window_id) {
                    tracing::warn!(%e, id = window_id, "center window failed");
                }
            }
            WindowAction::CenterVisibleColumns => {
                if let Err(e) = state.compositor().center_visible_columns(window_id) {
                    tracing::warn!(%e, id = window_id, "center visible columns failed");
                }
            }
            WindowAction::ExpandColumnToAvailableWidth => {
                if let Err(e) = state
                    .compositor()
                    .expand_column_to_available_width(window_id)
                {
                    tracing::warn!(%e, id = window_id, "expand column failed");
                }
            }
            WindowAction::FullscreenWindow => {
                if let Err(e) = state.compositor().fullscreen_window(window_id) {
                    tracing::warn!(%e, id = window_id, "fullscreen failed");
                }
            }
            WindowAction::ToggleWindowedFullscreen => {
                if let Err(e) = state.compositor().toggle_windowed_fullscreen(window_id) {
                    tracing::warn!(%e, id = window_id, "toggle windowed fullscreen failed");
                }
            }
            WindowAction::ToggleWindowFloating => {
                if let Err(e) = state.compositor().toggle_floating(window_id) {
                    tracing::warn!(%e, id = window_id, "toggle floating failed");
                }
            }
            WindowAction::ConsumeWindowIntoColumn => {
                if let Err(e) = state.compositor().consume_window_into_column(window_id) {
                    tracing::warn!(%e, id = window_id, "consume window into column failed");
                }
            }
            WindowAction::ExpelWindowFromColumn => {
                if let Err(e) = state.compositor().expel_window_from_column(window_id) {
                    tracing::warn!(%e, id = window_id, "expel window from column failed");
                }
            }
            WindowAction::ResetWindowHeight => {
                if let Err(e) = state.compositor().reset_window_height(window_id) {
                    tracing::warn!(%e, id = window_id, "reset window height failed");
                }
            }
            WindowAction::SwitchPresetColumnWidth => {
                if let Err(e) = state.compositor().switch_preset_column_width(window_id) {
                    tracing::warn!(%e, id = window_id, "switch preset column width failed");
                }
            }
            WindowAction::SwitchPresetWindowHeight => {
                if let Err(e) = state.compositor().switch_preset_window_height(window_id) {
                    tracing::warn!(%e, id = window_id, "switch preset window height failed");
                }
            }
            WindowAction::MoveWindowToWorkspaceDown => {
                if let Err(e) = state.compositor().move_window_to_workspace_down(window_id) {
                    tracing::warn!(%e, id = window_id, "move window to workspace down failed");
                }
            }
            WindowAction::MoveWindowToWorkspaceUp => {
                if let Err(e) = state.compositor().move_window_to_workspace_up(window_id) {
                    tracing::warn!(%e, id = window_id, "move window to workspace up failed");
                }
            }
            WindowAction::MoveWindowToMonitorLeft => {
                if let Err(e) = state.compositor().move_window_to_monitor_left(window_id) {
                    tracing::warn!(%e, id = window_id, "move window to monitor left failed");
                }
            }
            WindowAction::MoveWindowToMonitorRight => {
                if let Err(e) = state.compositor().move_window_to_monitor_right(window_id) {
                    tracing::warn!(%e, id = window_id, "move window to monitor right failed");
                }
            }
            WindowAction::ToggleColumnTabbedDisplay => {
                if let Err(e) = state.compositor().toggle_column_tabbed_display(window_id) {
                    tracing::warn!(%e, id = window_id, "toggle column tabbed display failed");
                }
            }
            WindowAction::FocusWorkspacePrevious => {
                if let Err(e) = state.compositor().focus_workspace_previous(window_id) {
                    tracing::warn!(%e, id = window_id, "focus workspace previous failed");
                }
            }
            WindowAction::MoveColumnLeft => {
                if let Err(e) = state.compositor().move_column_left(window_id) {
                    tracing::warn!(%e, id = window_id, "move column left failed");
                }
            }
            WindowAction::MoveColumnRight => {
                if let Err(e) = state.compositor().move_column_right(window_id) {
                    tracing::warn!(%e, id = window_id, "move column right failed");
                }
            }
            WindowAction::MoveColumnToFirst => {
                if let Err(e) = state.compositor().move_column_to_first(window_id) {
                    tracing::warn!(%e, id = window_id, "move column to first failed");
                }
            }
            WindowAction::MoveColumnToLast => {
                if let Err(e) = state.compositor().move_column_to_last(window_id) {
                    tracing::warn!(%e, id = window_id, "move column to last failed");
                }
            }
            WindowAction::MoveWindowDown => {
                if let Err(e) = state.compositor().move_window_down(window_id) {
                    tracing::warn!(%e, id = window_id, "move window down failed");
                }
            }
            WindowAction::MoveWindowUp => {
                if let Err(e) = state.compositor().move_window_up(window_id) {
                    tracing::warn!(%e, id = window_id, "move window up failed");
                }
            }
            WindowAction::MoveWindowDownOrToWorkspaceDown => {
                if let Err(e) = state
                    .compositor()
                    .move_window_down_or_to_workspace_down(window_id)
                {
                    tracing::warn!(%e, id = window_id, "move window down or to workspace down failed");
                }
            }
            WindowAction::MoveWindowUpOrToWorkspaceUp => {
                if let Err(e) = state
                    .compositor()
                    .move_window_up_or_to_workspace_up(window_id)
                {
                    tracing::warn!(%e, id = window_id, "move window up or to workspace up failed");
                }
            }
            WindowAction::MoveColumnLeftOrToMonitorLeft => {
                if let Err(e) = state
                    .compositor()
                    .move_column_left_or_to_monitor_left(window_id)
                {
                    tracing::warn!(%e, id = window_id, "move column left or to monitor left failed");
                }
            }
            WindowAction::MoveColumnRightOrToMonitorRight => {
                if let Err(e) = state
                    .compositor()
                    .move_column_right_or_to_monitor_right(window_id)
                {
                    tracing::warn!(%e, id = window_id, "move column right or to monitor right failed");
                }
            }
            WindowAction::Menu => {}
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

        std::thread::spawn(move || {
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
                MultiSelectAction::CloseWindows => state.compositor().close_window(window_id),
                MultiSelectAction::MoveToWorkspaceUp => {
                    state.compositor().move_window_to_workspace_up(window_id)
                }
                MultiSelectAction::MoveToWorkspaceDown => {
                    state.compositor().move_window_to_workspace_down(window_id)
                }
                MultiSelectAction::MoveToMonitorLeft => {
                    state.compositor().move_window_to_monitor_left(window_id)
                }
                MultiSelectAction::MoveToMonitorRight => {
                    state.compositor().move_window_to_monitor_right(window_id)
                }
                MultiSelectAction::MoveToMonitorUp => {
                    state.compositor().move_window_to_monitor_up(window_id)
                }
                MultiSelectAction::MoveToMonitorDown => {
                    state.compositor().move_window_to_monitor_down(window_id)
                }
                MultiSelectAction::MoveColumnLeft => state.compositor().move_column_left(window_id),
                MultiSelectAction::MoveColumnRight => {
                    state.compositor().move_column_right(window_id)
                }
                MultiSelectAction::ToggleFloating => state.compositor().toggle_floating(window_id),
                MultiSelectAction::FullscreenWindows => {
                    state.compositor().fullscreen_window(window_id)
                }
                MultiSelectAction::MaximizeColumns => {
                    state.compositor().maximize_window_column(window_id)
                }
                MultiSelectAction::CenterColumns => state.compositor().center_column(window_id),
                MultiSelectAction::ConsumeIntoColumn => {
                    state.compositor().consume_window_into_column(window_id)
                }
                MultiSelectAction::ToggleTabbedDisplay => {
                    state.compositor().toggle_column_tabbed_display(window_id)
                }
            };
            if let Err(e) = result {
                tracing::warn!(%e, id = window_id, "multi-select action failed");
            }
        }
    }
}
