use std::ptr::null;
use std::time::Duration;

use waybar_cffi::gtk::ffi::gtk_widget_override_background_color;
use waybar_cffi::gtk::glib::timeout_add_local_once;
use waybar_cffi::gtk::glib::translate::{IntoGlib, ToGlibPtr};
use waybar_cffi::gtk::glib::Propagation;
use waybar_cffi::gtk::prelude::{Cast, IsA, WidgetExt};
use waybar_cffi::gtk::{self as gtk, gdk, StateFlags};

use super::WindowButton;

pub fn set_background_color(
    widget: &impl IsA<gtk::Widget>,
    color: Option<&gdk::RGBA>,
) {
    unsafe {
        gtk_widget_override_background_color(
            Cast::upcast_ref::<gtk::Widget>(widget.as_ref())
                .to_glib_none()
                .0,
            StateFlags::NORMAL.into_glib(),
            color.map_or(null(), |c| c.to_glib_none().0),
        );
    }
}

impl WindowButton {
    pub(crate) fn setup_hover(&self) {
        let hover_bg = gdk::RGBA::new(0.5, 0.5, 0.5, 0.15);

        self.event_box.connect_enter_notify_event(move |widget, _| {
            set_background_color(widget, Some(&hover_bg));
            Propagation::Proceed
        });

        self.event_box.connect_leave_notify_event(move |widget, _| {
            set_background_color(widget, None);
            Propagation::Proceed
        });
    }

    pub(crate) fn setup_tooltip(&self) {
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

            let source_id = timeout_add_local_once(
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
            Propagation::Proceed
        });

        let tooltip_timeout_leave = self.tooltip_timeout.clone();
        let button_leave = self.event_box.clone();
        self.event_box.connect_leave_notify_event(move |_, _| {
            if let Some(timeout_id) = tooltip_timeout_leave.borrow_mut().take() {
                timeout_id.remove();
            }
            button_leave.set_tooltip_text(None);
            Propagation::Proceed
        });
    }
}
