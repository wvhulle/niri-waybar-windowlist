use std::{cell::RefCell, rc::Rc, time::Duration};

use waybar_cffi::gtk::{
    self as gtk,
    gdk::{DragAction, ModifierType, RGBA},
    glib::{timeout_add_local_once, SourceId},
    prelude::{BoxExt, Cast, DragContextExtManual, WidgetExt, WidgetExtManual},
    DestDefaults, TargetEntry, TargetFlags,
};

use crate::window_button::{hover_mouse::set_background_color, WindowButton};

impl WindowButton {
    pub(crate) fn setup_drag_reorder(&self) {
        tracing::debug!(window_id = self.window_id, "configuring drag-drop");

        let initial_position = Rc::new(RefCell::new(0));
        self.setup_drag_source(initial_position.clone());
        self.setup_drag_destination(initial_position);
    }

    fn setup_drag_source(&self, initial_position: Rc<RefCell<i32>>) {
        let internal_targets = vec![TargetEntry::new("text/plain", TargetFlags::SAME_APP, 0)];

        self.event_box.drag_source_set(
            ModifierType::BUTTON1_MASK,
            &internal_targets,
            DragAction::MOVE,
        );

        let pos_for_begin = initial_position;

        self.event_box.connect_drag_begin(move |widget, _| {
            if let Some(parent) = widget.parent() {
                if let Ok(container) = parent.downcast::<gtk::Box>() {
                    let position = container.child_position(widget);
                    *pos_for_begin.borrow_mut() = position;
                }
            }

            let drag_bg = RGBA::new(0.4, 0.4, 0.4, 0.2);
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
            DragAction::MOVE | DragAction::COPY,
        );

        let hover_timeout: Rc<RefCell<Option<SourceId>>> = Rc::new(RefCell::new(None));

        self.setup_drag_motion_and_leave(hover_timeout.clone());
        self.setup_drag_drop(initial_position, hover_timeout);
    }

    fn setup_drag_motion_and_leave(&self, hover_timeout: Rc<RefCell<Option<SourceId>>>) {
        let timeout_for_motion = hover_timeout.clone();
        let timeout_for_leave = hover_timeout;

        let state_for_motion = self.state.clone();
        let window_id_for_motion = self.window_id;
        let button_for_motion = self.event_box.clone();
        self.event_box
            .connect_drag_motion(move |widget, ctx, _x, _y, _time| {
                let is_external = ctx.drag_get_source_widget().is_none();

                if is_external {
                    if state_for_motion.settings.drag_hover_focus()
                        && timeout_for_motion.borrow().is_none()
                    {
                        let drag_over_bg = RGBA::new(0.5, 0.7, 1.0, 0.3);
                        set_background_color(&button_for_motion, Some(&drag_over_bg));

                        let state = state_for_motion.clone();
                        let wid = window_id_for_motion;
                        let delay = state_for_motion.settings.drag_hover_focus_delay();
                        let timeout_ref = timeout_for_motion.clone();

                        let source_id = timeout_add_local_once(
                            Duration::from_millis(u64::from(delay)),
                            move || {
                                tracing::debug!("drag hover focus triggered for window {}", wid);
                                if let Err(e) = state.compositor.focus_window(wid) {
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
    }

    fn setup_drag_drop(
        &self,
        initial_position: Rc<RefCell<i32>>,
        hover_timeout: Rc<RefCell<Option<SourceId>>>,
    ) {
        let timeout_for_drop = hover_timeout;

        let state_for_drop = self.state.clone();
        let pos_for_drop = initial_position;
        let settings_for_drop = self.state.settings.clone();
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
                                    tracing::debug!(
                                        start_pos,
                                        end_pos,
                                        delta,
                                        keep_stacked,
                                        "drag reposition"
                                    );

                                    match state.compositor.reposition_window(
                                        dragged_window_id,
                                        delta,
                                        keep_stacked,
                                    ) {
                                        Ok(()) => {
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
}
