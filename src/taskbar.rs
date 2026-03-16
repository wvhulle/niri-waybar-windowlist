use std::{cell::RefCell, collections::HashMap, rc::Rc};

use waybar_cffi::gtk::{
    self as gtk, gdk,
    glib::translate::{IntoGlib, ToGlibPtr},
    StateFlags,
};

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

pub type FocusedWindow = Rc<std::cell::Cell<Option<u64>>>;

pub fn create_focused_window() -> FocusedWindow {
    Rc::new(std::cell::Cell::new(None))
}
