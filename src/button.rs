use std::{cell::{Cell, RefCell}, fmt::Debug, path::PathBuf, rc::Rc, time::Duration};
use waybar_cffi::gtk::{
    self as gtk, EventBox, IconLookupFlags, IconSize, IconTheme, Orientation,
    gdk_pixbuf::Pixbuf,
    gdk,
    prelude::{BoxExt, Cast, ContainerExt, DragContextExtManual, EventBoxExt, GdkPixbufExt, IconThemeExt, LabelExt, WidgetExt, WidgetExtManual},
    DestDefaults, TargetEntry, TargetFlags,
};
use crate::taskbar::{set_background_color, SelectionState, FocusedWindow};
use crate::SharedState;

pub struct WindowButton {
    pub(crate) app_id: Option<String>,
    pub(crate) event_box: gtk::EventBox,
    pub(crate) layout_box: gtk::Box,
    pub(crate) title_label: gtk::Label,
    pub(crate) audio_event_box: EventBox,
    pub(crate) audio_label: gtk::Label,
    pub(crate) audio_sink_inputs: Rc<RefCell<Vec<(u32, bool)>>>,
    pub(crate) display_titles: bool,
    pub(crate) state: SharedState,
    pub(crate) window_id: u64,
    pub(crate) title: Rc<RefCell<Option<String>>>,
    pub(crate) selection: SelectionState,
    pub(crate) focused_window: FocusedWindow,
    pub(crate) tooltip_timeout: Rc<RefCell<Option<gtk::glib::SourceId>>>,
    pub(crate) skip_clicked: Rc<RefCell<bool>>,
    pub(crate) indicator_color: Rc<Cell<Option<gdk::RGBA>>>,
    pub(crate) is_urgent: Cell<bool>,
    pub(crate) process_info_enabled: bool,
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
        let process_info_enabled = state.settings().should_show_process_info(app_id.as_deref());
        let icon_location = app_id.as_deref().and_then(|id| state_clone.icon_resolver().resolve(id));

        let audio_label = gtk::Label::new(None);
        audio_label.show();
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
            process_info_enabled,
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

    pub fn process_info_enabled(&self) -> bool {
        self.process_info_enabled
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
            process_info_enabled: self.process_info_enabled,
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
        let audio_sink_inputs = self.audio_sink_inputs.clone();
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
                let audio_inputs = audio_sink_inputs.clone();
                let button_copy = button.clone();
                gtk::glib::source::idle_add_local_once(move || {
                    for child in container_copy.children() {
                        container_copy.remove(&child);
                    }

                    container_copy.pack_start(&icon_image, false, false, 0);

                    if show_titles {
                        container_copy.pack_start(&label_copy, true, true, 0);
                    }

                    container_copy.pack_start(&audio_copy, false, false, 0);

                    container_copy.show_all();
                    button_copy.show_all();

                    // Restore audio indicator visibility after re-packing,
                    // since no_show_all prevents show_all() from showing it.
                    if !audio_inputs.borrow().is_empty() {
                        audio_copy.show();
                    }
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
