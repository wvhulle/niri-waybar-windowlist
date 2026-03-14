use std::{cell::{Cell, RefCell}, collections::HashMap, fmt::Debug, path::PathBuf, process::Command, rc::Rc, time::{Duration, Instant}};
use waybar_cffi::gtk::{
    self as gtk, EventBox, IconLookupFlags, IconSize, IconTheme, Menu, MenuItem, Orientation, StateFlags,
    gdk_pixbuf::Pixbuf,
    gdk,
    glib::translate::{IntoGlib, ToGlibPtr},
    prelude::{AdjustmentExt, BoxExt, Cast, ContainerExt, DragContextExtManual, EventBoxExt, GdkPixbufExt, GtkMenuExt, GtkMenuItemExt, IconThemeExt, LabelExt, MenuShellExt, WidgetExt, WidgetExtManual},
    DestDefaults, TargetEntry, TargetFlags,
};
use crate::audio::SinkInput;
use crate::global::SharedState;
use crate::settings::{ModifierKey, MultiSelectAction};

/// Set background color on a widget using the deprecated but functional GTK3 API.
/// The safe gtk-rs bindings omit this method, so we call through FFI.
fn set_background_color(widget: &impl gtk::prelude::IsA<gtk::Widget>, color: Option<&gdk::RGBA>) {
    unsafe {
        gtk::ffi::gtk_widget_override_background_color(
            gtk::prelude::Cast::upcast_ref::<gtk::Widget>(widget.as_ref()).to_glib_none().0,
            StateFlags::NORMAL.into_glib(),
            color.map_or(std::ptr::null(), |c| c.to_glib_none().0),
        );
    }
}

pub type SelectionState = Rc<RefCell<HashMap<u64, gtk::EventBox>>>;

pub fn create_selection_state() -> SelectionState {
    Rc::new(RefCell::new(HashMap::new()))
}

pub fn clear_selection(selection: &SelectionState) {
    let mut sel = selection.borrow_mut();
    for (_, event_box) in sel.drain() {
        set_background_color(&event_box, None);
    }
}

pub type FocusedWindow = Rc<Cell<Option<u64>>>;

pub fn create_focused_window() -> FocusedWindow {
    Rc::new(Cell::new(None))
}

pub struct WindowButton {
    app_id: Option<String>,
    event_box: gtk::EventBox,
    layout_box: gtk::Box,
    title_label: gtk::Label,
    audio_event_box: EventBox,
    audio_label: gtk::Label,
    audio_sink_inputs: Rc<RefCell<Vec<(u32, bool)>>>,
    display_titles: bool,
    state: SharedState,
    window_id: u64,
    title: Rc<RefCell<Option<String>>>,
    selection: SelectionState,
    focused_window: FocusedWindow,
    tooltip_timeout: Rc<RefCell<Option<gtk::glib::SourceId>>>,
    skip_clicked: Rc<RefCell<bool>>,
    indicator_color: Rc<Cell<Option<gdk::RGBA>>>,
    is_urgent: Cell<bool>,
}

impl Debug for WindowButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowButton")
            .field("app_id", &self.app_id)
            .field("display_titles", &self.display_titles)
            .field("window_id", &self.window_id)
            .finish_non_exhaustive()
    }
}

thread_local! {
    static ICON_THEME_INSTANCE: IconTheme = IconTheme::default().unwrap_or_default();

    static TASKBAR_ADJUSTMENT: std::cell::RefCell<Option<gtk::Adjustment>> = const { std::cell::RefCell::new(None) };
}

pub fn set_taskbar_adjustment(adj: gtk::Adjustment) {
    TASKBAR_ADJUSTMENT.with(|cell| {
        *cell.borrow_mut() = Some(adj);
    });
}

fn scroll_taskbar(delta: f64) {
    TASKBAR_ADJUSTMENT.with(|cell| {
        if let Some(ref adj) = *cell.borrow() {
            let step = adj.page_size() / 4.0;
            let max = adj.upper() - adj.page_size();
            let new_value = (adj.value() + delta * step).clamp(0.0, max);
            adj.set_value(new_value);
        }
    });
}

impl WindowButton {
    #[tracing::instrument(level = "TRACE", fields(app_id = &window.app_id))]
    pub fn create(state: &SharedState, window: &niri_ipc::Window, selection: SelectionState, focused_window: FocusedWindow) -> Self {
        let state_clone = state.clone();
        let display_titles = state.settings().show_window_titles();

        let icon_gap = state.settings().icon_spacing();
        let layout_box = gtk::Box::new(Orientation::Horizontal, icon_gap);
        layout_box.set_vexpand(true);
        layout_box.set_margin_start(4);
        layout_box.set_margin_end(4);
        layout_box.set_margin_top(2);
        layout_box.set_margin_bottom(2);

        let title_label = gtk::Label::new(None);
        title_label.set_hexpand(true);
        let truncate_titles = state.settings().truncate_titles();
        if truncate_titles {
            title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        } else {
            title_label.set_ellipsize(gtk::pango::EllipsizeMode::None);
        }
        title_label.set_xalign(0.0);

        // Set normal font weight via Pango attributes
        let attrs = gtk::pango::AttrList::new();
        attrs.insert(gtk::pango::AttrInt::new_weight(gtk::pango::Weight::Normal));
        title_label.set_attributes(Some(&attrs));

        let indicator_color: Rc<Cell<Option<gdk::RGBA>>> = Rc::new(Cell::new(None));
        let indicator_for_draw = indicator_color.clone();

        let event_box = gtk::EventBox::new();
        event_box.set_visible_window(true);
        event_box.set_vexpand(true);
        event_box.add(&layout_box);

        event_box.connect_draw(move |widget, cr| {
            if let Some(rgba) = indicator_for_draw.get() {
                let w = widget.allocation().width() as f64;
                cr.set_source_rgba(rgba.red(), rgba.green(), rgba.blue(), rgba.alpha());
                cr.rectangle(0.0, 0.0, w, 3.0);
                cr.fill().ok();
            }
            gtk::glib::Propagation::Proceed
        });
        event_box.add_events(
            gdk::EventMask::BUTTON_PRESS_MASK |
            gdk::EventMask::BUTTON_RELEASE_MASK |
            gdk::EventMask::SCROLL_MASK |
            gdk::EventMask::SMOOTH_SCROLL_MASK |
            gdk::EventMask::ENTER_NOTIFY_MASK |
            gdk::EventMask::LEAVE_NOTIFY_MASK
        );

        event_box.set_margin_start(0);
        event_box.set_margin_end(0);
        event_box.set_margin_top(0);
        event_box.set_margin_bottom(0);

        if let Some(max_width) = state.settings().max_button_width(None) {
            event_box.set_size_request(max_width, -1);
            if display_titles && truncate_titles {
                let icon_dim = state.settings().icon_size();
                let max_chars = (max_width - icon_dim - icon_gap - 16) / 8;
                title_label.set_max_width_chars(max_chars);
            }
        }

        let app_id = window.app_id.clone();
        let icon_location = app_id.as_deref().and_then(|id| state_clone.icon_resolver().resolve(id));

        let audio_label = gtk::Label::new(None);
        audio_label.show();
        // Set normal font weight on audio label too
        audio_label.set_attributes(Some(&attrs));
        let audio_event_box = EventBox::new();
        audio_event_box.add(&audio_label);
        audio_event_box.set_no_show_all(true);

        let audio_sink_inputs = Rc::new(RefCell::new(Vec::<(u32, bool)>::new()));

        let button = Self {
            app_id,
            event_box,
            layout_box,
            title_label,
            audio_event_box,
            audio_label,
            audio_sink_inputs,
            display_titles,
            state: state_clone,
            window_id: window.id,
            title: Rc::new(RefCell::new(window.title.clone())),
            selection,
            focused_window,
            tooltip_timeout: Rc::new(RefCell::new(None)),
            skip_clicked: Rc::new(RefCell::new(false)),
            indicator_color,
            is_urgent: Cell::new(false),
        };

        button.setup_click_handlers(window.id);
        button.setup_audio_click_handler();
        button.setup_hover();
        button.setup_drag_reorder();
        button.setup_icon_rendering(icon_location);
        button.setup_tooltip();

        button
    }

    #[tracing::instrument(level = "TRACE")]
    pub fn update_focus(&self, is_focused: bool) {
        let colors = self.state.border_colors();
        if is_focused {
            self.indicator_color.set(Some(colors.active));
            self.focused_window.set(Some(self.window_id));
        } else if self.is_urgent.get() {
            self.indicator_color.set(Some(colors.urgent));
        } else {
            self.indicator_color.set(None);
        }
        self.event_box.queue_draw();
    }

    #[tracing::instrument(level = "TRACE")]
    pub fn update_title(&self, title: Option<&str>) {
        if let Some(t) = title {
            *self.title.borrow_mut() = Some(t.to_string());
        }

        if self.display_titles {
            if let Some(text) = title {
                let display_text = if self.state.settings().allow_title_linebreaks() {
                    text.to_string()
                } else {
                    text.replace('\n', " ").replace('\r', " ")
                };
                self.title_label.set_text(&display_text);
                self.title_label.show();
            } else {
                self.title_label.set_text("");
                self.title_label.hide();
            }
        }

    }

    #[tracing::instrument(level = "TRACE")]
    pub fn mark_urgent(&self) {
        self.is_urgent.set(true);
        let colors = self.state.border_colors();
        self.indicator_color.set(Some(colors.urgent));
        self.event_box.queue_draw();
    }

    #[tracing::instrument(level = "TRACE")]
    pub fn update_urgent(&self, urgent: bool) {
        self.is_urgent.set(urgent);
        if urgent {
            self.mark_urgent();
        }
    }

    pub fn get_widget(&self) -> &gtk::EventBox {
        &self.event_box
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
        let icon = if all_muted { config.muted_icon.as_str() } else { config.playing_icon.as_str() };

        self.audio_label.set_text(icon);
        self.audio_event_box.show();

        *self.audio_sink_inputs.borrow_mut() = sink_inputs.iter().map(|s| (s.index, s.muted)).collect();
    }

    fn setup_audio_click_handler(&self) {
        let config = self.state.settings().audio_indicator();
        if !config.enabled || !config.clickable {
            return;
        }

        let sink_inputs_ref = self.audio_sink_inputs.clone();
        self.audio_event_box.connect_button_press_event(move |_, event| {
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

        self.audio_event_box.set_tooltip_text(Some("Toggle mute"));
    }

    fn setup_hover(&self) {
        let hover_bg = gdk::RGBA::new(0.5, 0.5, 0.5, 0.15);
        let focused = self.focused_window.clone();
        let window_id = self.window_id;

        self.event_box.connect_enter_notify_event(move |widget, _| {
            if focused.get() != Some(window_id) {
                set_background_color(widget, Some(&hover_bg));
            }
            gtk::glib::Propagation::Proceed
        });

        let focused_leave = self.focused_window.clone();

        self.event_box.connect_leave_notify_event(move |widget, _| {
            if focused_leave.get() != Some(window_id) {
                set_background_color(widget, None);
            }
            gtk::glib::Propagation::Proceed
        });
    }

	fn setup_click_handlers(&self, window_id: u64) {
		let title = self.title.clone();

		// Left-click release: handles focused left-click and double-click
		let skip_release = self.skip_clicked.clone();
		let state_release = self.state.clone();
		let app_id_release = self.app_id.clone();
		let title_release = self.title.clone();
		let selection_release = self.selection.clone();
		let focused_release = self.focused_window.clone();
		let indicator_color_release = self.indicator_color.clone();
		let last_click_release = Rc::new(RefCell::new(Instant::now() - Duration::from_secs(1)));

		self.event_box.connect_button_release_event(move |btn, event| {
		    if event.button() == 1 {
		        if *skip_release.borrow() {
		            *skip_release.borrow_mut() = false;
		            return gtk::glib::Propagation::Stop;
		        }

		        let is_currently_focused = focused_release.get() == Some(window_id);
		        let app_id_ref = app_id_release.as_deref();
		        let title_ref = title_release.borrow();
		        let title_str = title_ref.as_deref();
		        let actions = state_release.settings().get_click_actions(app_id_ref, title_str);

		        if is_currently_focused {
		            let mut last_click = last_click_release.borrow_mut();
		            let now = Instant::now();
		            let time_since_last = now.duration_since(*last_click);

		            if time_since_last < Duration::from_millis(300) {
		                clear_selection(&selection_release);
		                Self::execute_click_action(&state_release, window_id, &actions.double_click, app_id_ref, title_str);
		                *last_click = Instant::now() - Duration::from_secs(1);
		            } else {
		                clear_selection(&selection_release);
		                Self::execute_click_action(&state_release, window_id, &actions.left_click_focused, app_id_ref, title_str);
		                *last_click = now;
		            }
		        } else {
		            clear_selection(&selection_release);
		            Self::optimistic_focus(btn, window_id, &focused_release, &indicator_color_release, &state_release);
		            Self::execute_click_action(&state_release, window_id, &actions.left_click_unfocused, app_id_ref, title_str);
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

		self.event_box.connect_button_press_event(move |btn, event| {
		    if event.button() == 1 {
		        let modifier_held = Self::check_modifier_from_event(event, state_press.settings().multi_select_modifier());
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
		                Self::optimistic_focus(btn, window_id, &focused_press, &indicator_color_press, &state_press);
		                let app_id_ref = app_id_press.as_deref();
		                let title_ref = title_press.borrow();
		                let title_str = title_ref.as_deref();
		                let actions = state_press.settings().get_click_actions(app_id_ref, title_str);
		                Self::execute_click_action(&state_press, window_id, &actions.left_click_unfocused, app_id_ref, title_str);
		            }
		        }
		        gtk::glib::Propagation::Proceed
		    } else if event.button() == 2 {
		        let is_currently_focused = focused_press.get() == Some(window_id);
		        let app_id_ref = app_id_press.as_deref();
		        let title_ref = title_press.borrow();
		        let title_str = title_ref.as_deref();
		        let actions = state_press.settings().get_click_actions(app_id_ref, title_str);
		        let action = if is_currently_focused {
		            &actions.middle_click_focused
		        } else {
		            &actions.middle_click_unfocused
		        };
		        if action.is_menu() {
		            menu_self.display_context_menu(window_id);
		        } else {
		            Self::execute_click_action(&state_press, window_id, action, app_id_ref, title_str);
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
		            let actions = state_press.settings().get_click_actions(app_id_ref, title_str);
		            let action = if is_currently_focused {
		                &actions.right_click_focused
		            } else {
		                &actions.right_click_unfocused
		            };
		            if action.is_menu() {
		                menu_self.display_context_menu(window_id);
		            } else {
		                Self::execute_click_action(&state_press, window_id, action, app_id_ref, title_str);
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
		    let actions = state_scroll.settings().get_click_actions(app_id_ref, title_str);

		    let (action, scroll_delta) = match event.direction() {
		        ScrollDirection::Up => (&actions.scroll_up, -1.0),
		        ScrollDirection::Down => (&actions.scroll_down, 1.0),
		        ScrollDirection::Smooth => {
		            let (delta_x, delta_y) = event.delta();
		            let delta = if delta_x.abs() > delta_y.abs() { delta_x } else { delta_y };
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

    fn optimistic_focus(
        btn: &gtk::EventBox,
        window_id: u64,
        focused_window: &FocusedWindow,
        indicator_color: &Rc<Cell<Option<gdk::RGBA>>>,
        state: &SharedState,
    ) {
        // Redraw all siblings to clear their indicators
        if let Some(parent) = btn.parent() {
            if let Ok(container) = parent.downcast::<gtk::Box>() {
                for child in container.children() {
                    if let Ok(child_eb) = child.downcast::<gtk::EventBox>() {
                        child_eb.queue_draw();
                    }
                }
            }
        }

        // Mark this one as focused
        let colors = state.border_colors();
        indicator_color.set(Some(colors.active));
        btn.queue_draw();
        focused_window.set(Some(window_id));
    }

    fn execute_click_action(
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

    fn execute_action(state: &SharedState, window_id: u64, action: &crate::settings::WindowAction, app_id: Option<&str>, title: Option<&str>) {
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
                if let Err(e) = state.compositor().expand_column_to_available_width(window_id) {
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
                if let Err(e) = state.compositor().move_window_down_or_to_workspace_down(window_id) {
                    tracing::warn!(%e, id = window_id, "move window down or to workspace down failed");
                }
            }
            WindowAction::MoveWindowUpOrToWorkspaceUp => {
                if let Err(e) = state.compositor().move_window_up_or_to_workspace_up(window_id) {
                    tracing::warn!(%e, id = window_id, "move window up or to workspace up failed");
                }
            }
            WindowAction::MoveColumnLeftOrToMonitorLeft => {
                if let Err(e) = state.compositor().move_column_left_or_to_monitor_left(window_id) {
                    tracing::warn!(%e, id = window_id, "move column left or to monitor left failed");
                }
            }
            WindowAction::MoveColumnRightOrToMonitorRight => {
                if let Err(e) = state.compositor().move_column_right_or_to_monitor_right(window_id) {
                    tracing::warn!(%e, id = window_id, "move column right or to monitor right failed");
                }
            }
            WindowAction::Menu => {}
        }
    }

	#[tracing::instrument(level = "TRACE", skip(self))]
	fn display_context_menu(&self, window_id: u64) {
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

	fn execute_command(command: &str, window_id: u64, app_id: Option<&str>, title: Option<&str>) {
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

	fn check_modifier_from_event(event: &gdk::EventButton, modifier: ModifierKey) -> bool {
		let state = event.state();
		match modifier {
		    ModifierKey::Ctrl => state.contains(gdk::ModifierType::CONTROL_MASK),
		    ModifierKey::Shift => state.contains(gdk::ModifierType::SHIFT_MASK),
		    ModifierKey::Alt => state.contains(gdk::ModifierType::MOD1_MASK),
		    ModifierKey::Super => state.contains(gdk::ModifierType::SUPER_MASK),
		}
	}

	fn check_modifier_static(modifier: ModifierKey) -> bool {
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

	fn display_multi_select_menu(&self) {
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

	fn execute_multi_select_action(state: &SharedState, window_ids: &[u64], action: &MultiSelectAction) {
		for &window_id in window_ids {
		    let result = match action {
		        MultiSelectAction::CloseWindows => state.compositor().close_window(window_id),
		        MultiSelectAction::MoveToWorkspaceUp => state.compositor().move_window_to_workspace_up(window_id),
		        MultiSelectAction::MoveToWorkspaceDown => state.compositor().move_window_to_workspace_down(window_id),
		        MultiSelectAction::MoveToMonitorLeft => state.compositor().move_window_to_monitor_left(window_id),
		        MultiSelectAction::MoveToMonitorRight => state.compositor().move_window_to_monitor_right(window_id),
		        MultiSelectAction::MoveToMonitorUp => state.compositor().move_window_to_monitor_up(window_id),
		        MultiSelectAction::MoveToMonitorDown => state.compositor().move_window_to_monitor_down(window_id),
		        MultiSelectAction::MoveColumnLeft => state.compositor().move_column_left(window_id),
		        MultiSelectAction::MoveColumnRight => state.compositor().move_column_right(window_id),
		        MultiSelectAction::ToggleFloating => state.compositor().toggle_floating(window_id),
		        MultiSelectAction::FullscreenWindows => state.compositor().fullscreen_window(window_id),
		        MultiSelectAction::MaximizeColumns => state.compositor().maximize_window_column(window_id),
		        MultiSelectAction::CenterColumns => state.compositor().center_column(window_id),
		        MultiSelectAction::ConsumeIntoColumn => state.compositor().consume_window_into_column(window_id),
		        MultiSelectAction::ToggleTabbedDisplay => state.compositor().toggle_column_tabbed_display(window_id),
		    };
		    if let Err(e) = result {
		        tracing::warn!(%e, id = window_id, "multi-select action failed");
		    }
		}
	}

	fn clone_for_menu(&self) -> Self {
		Self {
		    app_id: self.app_id.clone(),
		    event_box: self.event_box.clone(),
		    layout_box: self.layout_box.clone(),
		    title_label: self.title_label.clone(),
		    audio_event_box: self.audio_event_box.clone(),
		    audio_label: self.audio_label.clone(),
		    audio_sink_inputs: self.audio_sink_inputs.clone(),
		    display_titles: self.display_titles,
		    state: self.state.clone(),
		    window_id: self.window_id,
		    title: self.title.clone(),
		    selection: self.selection.clone(),
		    focused_window: self.focused_window.clone(),
		    tooltip_timeout: self.tooltip_timeout.clone(),
		    skip_clicked: self.skip_clicked.clone(),
		    indicator_color: self.indicator_color.clone(),
		    is_urgent: Cell::new(self.is_urgent.get()),
		}
	}

    fn setup_drag_reorder(&self) {
        tracing::info!("configuring drag-drop for window {}", self.window_id);

        let internal_targets = vec![TargetEntry::new("text/plain", TargetFlags::SAME_APP, 0)];

        self.event_box.drag_source_set(
            gtk::gdk::ModifierType::BUTTON1_MASK,
            &internal_targets,
            gtk::gdk::DragAction::MOVE,
        );

        let dest_targets = vec![
            TargetEntry::new("text/plain", TargetFlags::SAME_APP, 0),
            TargetEntry::new("text/uri-list", TargetFlags::OTHER_APP, 1),
            TargetEntry::new("text/plain", TargetFlags::OTHER_APP, 2),
        ];

        self.event_box.drag_dest_set(
            DestDefaults::MOTION | DestDefaults::HIGHLIGHT,
            &dest_targets,
            gtk::gdk::DragAction::MOVE | gtk::gdk::DragAction::COPY,
        );

        let initial_position = Rc::new(RefCell::new(0));
        let pos_for_begin = initial_position.clone();

        self.event_box.connect_drag_begin(move |widget, _| {
            tracing::info!("drag initiated");

            if let Some(parent) = widget.parent() {
                if let Ok(container) = parent.downcast::<gtk::Box>() {
                    let position = container.child_position(widget);
                    *pos_for_begin.borrow_mut() = position;
                    tracing::info!("stored initial position: {}", position);
                }
            }

            let drag_bg = gdk::RGBA::new(0.4, 0.4, 0.4, 0.2);
            set_background_color(widget, Some(&drag_bg));
        });

        let window_id = self.window_id;
        self.event_box.connect_drag_data_get(move |_, _, data, _, _| {
            data.set_text(&window_id.to_string());
        });

        let button_for_end = self.event_box.clone();
        let skip_clicked_drag = self.skip_clicked.clone();
        self.event_box.connect_drag_end(move |_, _| {
            tracing::info!("drag completed");
            set_background_color(&button_for_end, None);
            *skip_clicked_drag.borrow_mut() = false;
        });

        let hover_timeout: Rc<RefCell<Option<gtk::glib::SourceId>>> = Rc::new(RefCell::new(None));
        let timeout_for_motion = hover_timeout.clone();
        let timeout_for_leave = hover_timeout.clone();
        let timeout_for_drop = hover_timeout.clone();

        let state_for_motion = self.state.clone();
        let window_id_for_motion = self.window_id;
        let button_for_motion = self.event_box.clone();
        self.event_box.connect_drag_motion(move |widget, ctx, _x, _y, _time| {
            let is_external = ctx.drag_get_source_widget().is_none();

            if is_external {
                if state_for_motion.settings().drag_hover_focus() && timeout_for_motion.borrow().is_none() {
                    let drag_over_bg = gdk::RGBA::new(0.5, 0.7, 1.0, 0.3);
                    set_background_color(&button_for_motion, Some(&drag_over_bg));

                    let state = state_for_motion.clone();
                    let wid = window_id_for_motion;
                    let delay = state_for_motion.settings().drag_hover_focus_delay();
                    let timeout_ref = timeout_for_motion.clone();

                    let source_id = gtk::glib::timeout_add_local_once(
                        Duration::from_millis(delay as u64),
                        move || {
                            tracing::debug!("drag hover focus triggered for window {}", wid);
                            if let Err(e) = state.compositor().focus_window(wid) {
                                tracing::error!("failed to focus window on drag hover: {}", e);
                            }
                            timeout_ref.borrow_mut().take();
                        }
                    );

                    *timeout_for_motion.borrow_mut() = Some(source_id);
                }
                return true;
            }

            if let Some(source) = ctx.drag_get_source_widget() {
                if source != *widget {
                    if let Some(parent) = widget.parent() {
                        if let Ok(container) = parent.downcast::<gtk::Box>() {
                            let source_pos = container.child_position(&source);
                            let target_pos = container.child_position(widget);

                            if source_pos != target_pos {
                                container.reorder_child(&source, target_pos);
                                tracing::trace!("reordered from {} to {}", source_pos, target_pos);
                            }
                        }
                    }
                }
            }
            true
        });

        let button_for_leave = self.event_box.clone();
        self.event_box.connect_drag_leave(move |_, _, _| {
            set_background_color(&button_for_leave, None);

            if let Some(timeout_id) = timeout_for_leave.borrow_mut().take() {
                timeout_id.remove();
            }
        });

        let state_for_drop = self.state.clone();
        let pos_for_drop = initial_position.clone();
        let settings_for_drop = self.state.settings().clone();
        self.event_box.connect_drag_drop(move |widget, ctx, _x, _y, time| {
            if let Some(timeout_id) = timeout_for_drop.borrow_mut().take() {
                timeout_id.remove();
            }

            let is_internal = ctx.drag_get_source_widget().is_some();

            if is_internal {
                let target = widget.drag_dest_find_target(ctx, None);
                if let Some(target) = target {
                    widget.drag_get_data(ctx, &target, time);
                    return true;
                }
            }

            false
        });

        let state = state_for_drop;
        self.event_box.connect_drag_data_received(move |_widget, ctx, _, _, data, _, time| {
            tracing::info!("drop received");

            if let Some(text) = data.text() {
                if let Ok(dragged_window_id) = text.parse::<u64>() {
                    if let Some(source) = ctx.drag_get_source_widget() {
                        if let Some(parent) = source.parent() {
                            if let Ok(container) = parent.downcast::<gtk::Box>() {
                                let start_pos = *pos_for_drop.borrow();
                                let end_pos = container.child_position(&source);
                                let delta = end_pos - start_pos;

                                let keep_stacked = Self::check_modifier_static(settings_for_drop.multi_select_modifier());
                                tracing::info!("position change: {} -> {} (delta: {}, keep_stacked: {})", start_pos, end_pos, delta, keep_stacked);

                                match state.compositor().reposition_window(dragged_window_id, delta, keep_stacked) {
                                    Ok(()) => {
                                        tracing::info!("reposition successful");
                                        ctx.drag_finish(true, false, time);
                                        return;
                                    }
                                    Err(e) => {
                                        tracing::error!("reposition failed: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            ctx.drag_finish(false, false, time);
        });
    }

    #[tracing::instrument(level = "TRACE")]
    fn setup_icon_rendering(&self, icon_path: Option<PathBuf>) {
        let last_allocation = RefCell::new(None);
        let container = self.layout_box.clone();
        let label = self.title_label.clone();
        let audio_event_box = self.audio_event_box.clone();
        let show_titles = self.display_titles;
        let icon_dimension = self.state.settings().icon_size();

        self.event_box.connect_size_allocate(move |button, allocation| {
            let mut needs_render = container.children().is_empty();

            if !needs_render {
                if let Some(prev_alloc) = last_allocation.take() {
                    if &prev_alloc != allocation {
                        needs_render = true;
                    }
                } else {
                    needs_render = true;
                }

                last_allocation.replace(Some(*allocation));
            }

            if needs_render {
                let dimension = icon_dimension;

                let icon_image = Self::load_icon_image(icon_path.as_ref(), button, dimension)
                    .unwrap_or_else(|| {
                        static FALLBACK: &str = "application-x-executable";

                        ICON_THEME_INSTANCE.with(|theme| {
                            theme.lookup_icon_for_scale(
                                FALLBACK,
                                dimension,
                                button.scale_factor(),
                                IconLookupFlags::empty(),
                            )
                        })
                        .and_then(|info| Self::load_icon_image(info.filename().as_ref(), button, dimension))
                        .unwrap_or_else(|| gtk::Image::from_icon_name(Some(FALLBACK), IconSize::Button))
                    });

                let container_copy = container.clone();
                let label_copy = label.clone();
                let audio_copy = audio_event_box.clone();
                let button_copy = button.clone();
                gtk::glib::source::idle_add_local_once(move || {
                    for child in container_copy.children() {
                        container_copy.remove(&child);
                    }

                    container_copy.pack_start(&icon_image, false, false, 0);
                    container_copy.pack_start(&audio_copy, false, false, 0);

                    if show_titles {
                        container_copy.pack_start(&label_copy, true, true, 0);
                    }

                    container_copy.show_all();
                    button_copy.show_all();
                });
            }
        });
    }

    fn load_icon_image(
        path: Option<&PathBuf>,
        widget: &impl WidgetExt,
        size: i32,
    ) -> Option<gtk::Image> {
        let scaled_size = size * widget.scale_factor();

        path.and_then(|p| match Pixbuf::from_file_at_scale(p, scaled_size, scaled_size, true) {
            Ok(pixbuf) => Some(pixbuf),
            Err(e) => {
                tracing::info!(%e, ?p, "icon load failed");
                None
            }
        })
        .and_then(|pixbuf| pixbuf.create_surface(0, widget.window().as_ref()))
        .map(|surface| gtk::Image::from_surface(Some(&surface)))
    }

    fn setup_tooltip(&self) {
        if !self.state.settings().show_tooltip() {
            return;
        }

        let delay = self.state.settings().tooltip_delay();
        let title = self.title.clone();
        let tooltip_timeout = self.tooltip_timeout.clone();

        self.event_box.connect_enter_notify_event(move |btn, _| {
            let title_clone = title.clone();
            let btn_clone = btn.clone();
            let timeout_ref = tooltip_timeout.clone();

            let source_id = gtk::glib::timeout_add_local_once(
                Duration::from_millis(delay as u64),
                move || {
                    if let Some(ref text) = *title_clone.borrow() {
                        btn_clone.set_tooltip_text(Some(text));
                        btn_clone.trigger_tooltip_query();
                    }
                    timeout_ref.borrow_mut().take();
                }
            );

            *tooltip_timeout.borrow_mut() = Some(source_id);
            gtk::glib::Propagation::Proceed
        });

        let tooltip_timeout_leave = self.tooltip_timeout.clone();
        let button_leave = self.event_box.clone();
        self.event_box.connect_leave_notify_event(move |_, _| {
            if let Some(timeout_id) = tooltip_timeout_leave.borrow_mut().take() {
                timeout_id.remove();
            }
            button_leave.set_tooltip_text(None);
            gtk::glib::Propagation::Proceed
        });
    }

	pub fn resize_for_width(&self, width: i32) {
		if self.state.settings().max_button_width(None).is_none() {
		    return;
		}
		if self.display_titles && self.state.settings().truncate_titles() {
		    let icon_dim = self.state.settings().icon_size();
		    let icon_gap = self.state.settings().icon_spacing();
		    let max_chars = ((width - icon_dim - icon_gap - 16) / 8).max(0);
		    self.title_label.set_max_width_chars(max_chars);

		    if max_chars == 0 {
		        self.title_label.hide();
		    }
		}
	}
}