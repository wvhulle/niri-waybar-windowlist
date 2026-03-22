pub(crate) mod click_handlers;
pub(crate) mod context_menu;

use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap},
    fmt::Debug,
    path::PathBuf,
    rc::Rc,
    time::Duration,
};

use waybar_cffi::gtk::{
    self as gtk, gdk,
    gdk_pixbuf::Pixbuf,
    prelude::{
        BoxExt, Cast, ContainerExt, DragContextExtManual, EventBoxExt, GdkPixbufExt, IconThemeExt,
        LabelExt, WidgetExt, WidgetExtManual,
    },
    DestDefaults, EventBox, IconLookupFlags, IconSize, IconTheme, Orientation, StateFlags,
    TargetEntry, TargetFlags,
};

use crate::{audio::PlaybackStatus, niri_border_colors::IndicatorColor, title_format, SharedState};

// ── Selection & Focus helpers (from taskbar.rs) ──

pub fn set_background_color(
    widget: &impl gtk::prelude::IsA<gtk::Widget>,
    color: Option<&gdk::RGBA>,
) {
    unsafe {
        gtk::ffi::gtk_widget_override_background_color(
            gtk::prelude::Cast::upcast_ref::<gtk::Widget>(widget.as_ref())
                .to_glib_none()
                .0,
            StateFlags::NORMAL.into_glib(),
            color.map_or(std::ptr::null(), |c| c.to_glib_none().0),
        );
    }
}

use waybar_cffi::gtk::glib::translate::{IntoGlib, ToGlibPtr};

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

// ── WindowButton ──

pub struct WindowButton {
    pub(crate) app_id: Option<String>,
    pub(crate) event_box: gtk::EventBox,
    pub(crate) layout_box: gtk::Box,
    pub(crate) title_label: gtk::Label,
    pub(crate) audio_event_box: EventBox,
    pub(crate) audio_label: gtk::Label,
    pub(crate) audio_visible: Rc<Cell<bool>>,
    pub(crate) display_titles: bool,
    pub(crate) state: SharedState,
    pub(crate) window_id: u64,
    pub(crate) title: Rc<RefCell<Option<String>>>,
    pub(crate) selection: SelectionState,
    pub(crate) focused_window: FocusedWindow,
    pub(crate) tooltip_timeout: Rc<RefCell<Option<gtk::glib::SourceId>>>,
    pub(crate) skip_clicked: Rc<RefCell<bool>>,
    pub(crate) indicator_color: Rc<Cell<Option<IndicatorColor>>>,
    pub(crate) is_urgent: Cell<bool>,
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
}

impl WindowButton {
    #[tracing::instrument(level = "TRACE", fields(app_id = &window.app_id))]
    pub fn create(
        state: &SharedState,
        window: &niri_ipc::Window,
        selection: SelectionState,
        focused_window: FocusedWindow,
    ) -> Self {
        let state_clone = state.clone();
        let display_titles = state.settings().show_window_titles();

        let icon_gap = state.settings().icon_spacing();
        let layout_box = gtk::Box::new(Orientation::Horizontal, icon_gap);
        layout_box.set_vexpand(true);
        layout_box.set_margin_start(4);
        layout_box.set_margin_end(4);
        layout_box.set_margin_top(4);
        layout_box.set_margin_bottom(0);

        let title_label = gtk::Label::new(None);
        title_label.set_hexpand(true);
        let truncate_titles = state.settings().truncate_titles();
        if truncate_titles {
            title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        } else {
            title_label.set_ellipsize(gtk::pango::EllipsizeMode::None);
        }
        title_label.set_xalign(0.0);

        let attrs = gtk::pango::AttrList::new();
        attrs.insert(gtk::pango::AttrInt::new_weight(gtk::pango::Weight::Normal));
        title_label.set_attributes(Some(&attrs));

        let indicator_color: Rc<Cell<Option<IndicatorColor>>> = Rc::new(Cell::new(None));

        let event_box = gtk::EventBox::new();
        event_box.set_visible_window(true);
        event_box.set_vexpand(true);
        event_box.add(&layout_box);

        Self::setup_border_indicator(&indicator_color, &event_box);
        event_box.add_events(
            gdk::EventMask::BUTTON_PRESS_MASK
                | gdk::EventMask::BUTTON_RELEASE_MASK
                | gdk::EventMask::SCROLL_MASK
                | gdk::EventMask::SMOOTH_SCROLL_MASK
                | gdk::EventMask::ENTER_NOTIFY_MASK
                | gdk::EventMask::LEAVE_NOTIFY_MASK,
        );

        event_box.set_margin_start(0);
        event_box.set_margin_end(0);
        event_box.set_margin_top(0);
        event_box.set_margin_bottom(0);

        let app_id = window.app_id.clone();
        let icon_location = app_id
            .as_deref()
            .and_then(|id| state_clone.icon_resolver().resolve(id));

        let audio_label = gtk::Label::new(None);
        audio_label.show();
        audio_label.set_attributes(Some(&attrs));
        let audio_event_box = EventBox::new();
        audio_event_box.add(&audio_label);
        audio_event_box.set_no_show_all(true);

        let audio_visible = Rc::new(Cell::new(false));

        let button = Self {
            app_id,
            event_box,
            layout_box,
            title_label,
            audio_event_box,
            audio_label,
            audio_visible,
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

    pub(crate) fn clone_for_menu(&self) -> Self {
        Self {
            app_id: self.app_id.clone(),
            event_box: self.event_box.clone(),
            layout_box: self.layout_box.clone(),
            title_label: self.title_label.clone(),
            audio_event_box: self.audio_event_box.clone(),
            audio_label: self.audio_label.clone(),
            audio_visible: self.audio_visible.clone(),
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

    fn setup_drag_reorder(&self) {
        tracing::info!("configuring drag-drop for window {}", self.window_id);

        let initial_position = Rc::new(RefCell::new(0));
        self.setup_drag_source(initial_position.clone());
        self.setup_drag_destination(initial_position);
    }

    fn setup_drag_source(&self, initial_position: Rc<RefCell<i32>>) {
        let internal_targets = vec![TargetEntry::new("text/plain", TargetFlags::SAME_APP, 0)];

        self.event_box.drag_source_set(
            gtk::gdk::ModifierType::BUTTON1_MASK,
            &internal_targets,
            gtk::gdk::DragAction::MOVE,
        );

        let pos_for_begin = initial_position;

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
        self.event_box
            .connect_drag_data_get(move |_, _, data, _, _| {
                data.set_text(&window_id.to_string());
            });

        let button_for_end = self.event_box.clone();
        let skip_clicked_drag = self.skip_clicked.clone();
        self.event_box.connect_drag_end(move |_, _| {
            tracing::info!("drag completed");
            set_background_color(&button_for_end, None);
            *skip_clicked_drag.borrow_mut() = false;
        });
    }

    fn setup_drag_destination(&self, initial_position: Rc<RefCell<i32>>) {
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

        let hover_timeout: Rc<RefCell<Option<gtk::glib::SourceId>>> = Rc::new(RefCell::new(None));
        let timeout_for_motion = hover_timeout.clone();
        let timeout_for_leave = hover_timeout.clone();
        let timeout_for_drop = hover_timeout.clone();

        let state_for_motion = self.state.clone();
        let window_id_for_motion = self.window_id;
        let button_for_motion = self.event_box.clone();
        self.event_box
            .connect_drag_motion(move |widget, ctx, _x, _y, _time| {
                let is_external = ctx.drag_get_source_widget().is_none();

                if is_external {
                    if state_for_motion.settings().drag_hover_focus()
                        && timeout_for_motion.borrow().is_none()
                    {
                        let drag_over_bg = gdk::RGBA::new(0.5, 0.7, 1.0, 0.3);
                        set_background_color(&button_for_motion, Some(&drag_over_bg));

                        let state = state_for_motion.clone();
                        let wid = window_id_for_motion;
                        let delay = state_for_motion.settings().drag_hover_focus_delay();
                        let timeout_ref = timeout_for_motion.clone();

                        let source_id = gtk::glib::timeout_add_local_once(
                            Duration::from_millis(u64::from(delay)),
                            move || {
                                tracing::debug!("drag hover focus triggered for window {}", wid);
                                if let Err(e) = state.compositor().focus_window(wid) {
                                    tracing::error!("failed to focus window on drag hover: {}", e);
                                }
                                timeout_ref.borrow_mut().take();
                            },
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
                                    tracing::trace!(
                                        "reordered from {} to {}",
                                        source_pos,
                                        target_pos
                                    );
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
        let pos_for_drop = initial_position;
        let settings_for_drop = self.state.settings().clone();
        self.event_box
            .connect_drag_drop(move |widget, ctx, _x, _y, time| {
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
        self.event_box
            .connect_drag_data_received(move |_widget, ctx, _, _, data, _, time| {
                tracing::info!("drop received");

                if let Some(text) = data.text() {
                    if let Ok(dragged_window_id) = text.parse::<u64>() {
                        if let Some(source) = ctx.drag_get_source_widget() {
                            if let Some(parent) = source.parent() {
                                if let Ok(container) = parent.downcast::<gtk::Box>() {
                                    let start_pos = *pos_for_drop.borrow();
                                    let end_pos = container.child_position(&source);
                                    let delta = end_pos - start_pos;

                                    let keep_stacked = Self::check_modifier_static(
                                        settings_for_drop.multi_select_modifier(),
                                    );
                                    tracing::info!(
                                        "position change: {} -> {} (delta: {}, keep_stacked: {})",
                                        start_pos,
                                        end_pos,
                                        delta,
                                        keep_stacked
                                    );

                                    match state.compositor().reposition_window(
                                        dragged_window_id,
                                        delta,
                                        keep_stacked,
                                    ) {
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
        let container = self.layout_box.clone();
        let label = self.title_label.clone();
        let audio_event_box = self.audio_event_box.clone();
        let audio_visible = self.audio_visible.clone();
        let show_titles = self.display_titles;
        let icon_dimension = self.state.settings().icon_size();

        // Pack label and audio immediately — they don't need the scale factor.
        if show_titles {
            container.pack_start(&label, true, true, 0);
        }
        container.pack_start(&audio_event_box, false, false, 0);

        // Load and insert the icon once the widget is realized (so scale_factor is available).
        let icon_inserted = Rc::new(Cell::new(false));
        self.event_box
            .connect_size_allocate(move |button, _allocation| {
                if icon_inserted.get() {
                    return;
                }
                icon_inserted.set(true);
                tracing::info!("icon insertion triggered for size_allocate (one-time)");

                let dimension = icon_dimension;

                let icon_image = Self::load_icon_image(icon_path.as_ref(), button, dimension)
                    .unwrap_or_else(|| {
                        static FALLBACK: &str = "application-x-executable";

                        ICON_THEME_INSTANCE
                            .with(|theme| {
                                theme.lookup_icon_for_scale(
                                    FALLBACK,
                                    dimension,
                                    button.scale_factor(),
                                    IconLookupFlags::empty(),
                                )
                            })
                            .and_then(|info| {
                                Self::load_icon_image(
                                    info.filename().as_ref(),
                                    button,
                                    dimension,
                                )
                            })
                            .unwrap_or_else(|| {
                                gtk::Image::from_icon_name(Some(FALLBACK), IconSize::Button)
                            })
                    });

                // Insert icon at the front, before the label.
                let container_copy = container.clone();
                let audio_copy = audio_event_box.clone();
                let audio_vis = audio_visible.clone();
                let button_copy = button.clone();
                gtk::glib::source::idle_add_local_once(move || {
                    container_copy.pack_start(&icon_image, false, false, 0);
                    container_copy.reorder_child(&icon_image, 0);

                    container_copy.show_all();
                    button_copy.show_all();

                    if audio_vis.get() {
                        audio_copy.show();
                    }
                });
            });
    }

    fn load_icon_image(
        path: Option<&PathBuf>,
        widget: &impl WidgetExt,
        size: i32,
    ) -> Option<gtk::Image> {
        let scaled_size = size * widget.scale_factor();

        path.and_then(
            |p| match Pixbuf::from_file_at_scale(p, scaled_size, scaled_size, true) {
                Ok(pixbuf) => Some(pixbuf),
                Err(e) => {
                    tracing::info!(%e, ?p, "icon load failed");
                    None
                }
            },
        )
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
                Duration::from_millis(u64::from(delay)),
                move || {
                    if let Some(ref text) = *title_clone.borrow() {
                        btn_clone.set_tooltip_text(Some(text));
                        btn_clone.trigger_tooltip_query();
                    }
                    timeout_ref.borrow_mut().take();
                },
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

    // ── Border indicator (from indicator.rs) ──

    pub(crate) fn setup_border_indicator(
        indicator_color: &Rc<Cell<Option<IndicatorColor>>>,
        event_box: &gtk::EventBox,
    ) {
        let indicator_for_draw = indicator_color.clone();
        event_box.connect_draw(move |widget, cr| {
            if let Some(color) = indicator_for_draw.get() {
                let w = f64::from(widget.allocation().width());
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

    // ── Title formatting ──

    #[tracing::instrument(level = "INFO")]
    pub fn update_title(&self, title: Option<&str>) {
        if let Some(t) = title {
            *self.title.borrow_mut() = Some(t.to_string());
        }

        if let Some(text) = title {
            let rule = self
                .app_id
                .as_deref()
                .and_then(|id| self.state.settings().title_format_rule(id));

            if let Some(rule) = rule {
                if let Some(caps) = rule.pattern.captures(text) {
                    let capture_names: BTreeMap<&str, &str> = rule
                        .pattern
                        .capture_names()
                        .flatten()
                        .filter_map(|name| caps.name(name).map(|m| (name, m.as_str())))
                        .collect();

                    if let Some(markup) = title_format::render_with_rule(rule, &capture_names) {
                        tracing::info!(
                            window_id = self.window_id,
                            markup = %markup,
                            has_parent = self.title_label.parent().is_some(),
                            "set_markup on title label"
                        );
                        self.title_label.set_markup(&markup);
                        self.title_label.show();
                        return;
                    }
                }
            }
        }

        if self.display_titles {
            if let Some(text) = title {
                let display_text = if self.state.settings().allow_title_linebreaks() {
                    text.to_string()
                } else {
                    text.replace(['\n', '\r'], " ")
                };
                tracing::info!(
                    window_id = self.window_id,
                    display_text = %display_text,
                    has_parent = self.title_label.parent().is_some(),
                    "set_text on title label"
                );
                self.title_label.set_text(&display_text);
                self.title_label.show();
            } else {
                tracing::info!(window_id = self.window_id, "clearing title label");
                self.title_label.set_text("");
                self.title_label.hide();
            }
        }
    }

    /// Update the title label using process info from `/proc` polling.
    ///
    /// Builds a capture map from `cwd` and `command`, then renders through
    /// the title format rule template. Falls back to the raw title if no
    /// rule matches or captures are empty.
    #[tracing::instrument(level = "TRACE")]
    pub fn update_process_info(&self, cwd: Option<&str>, command: Option<&str>) {
        if !self.display_titles {
            return;
        }

        let rule = self
            .app_id
            .as_deref()
            .and_then(|id| self.state.settings().title_format_rule(id));

        if let Some(rule) = rule {
            let mut captures = BTreeMap::new();
            if let Some(c) = cwd {
                captures.insert("cwd", c);
            }
            if let Some(c) = command {
                captures.insert("cmd", c);
            }

            if captures.is_empty() {
                let title = self.title.borrow();
                self.title_label.set_text(title.as_deref().unwrap_or(""));
                self.title_label.show();
                return;
            }

            if let Some(markup) = title_format::render_with_rule(rule, &captures) {
                self.title_label.set_markup(&markup);
                self.title_label.show();
                return;
            }
        }

        let title = self.title.borrow();
        self.title_label.set_text(title.as_deref().unwrap_or(""));
        self.title_label.show();
    }
}
