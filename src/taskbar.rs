use std::{cell::RefCell, collections::HashMap, rc::Rc};

use waybar_cffi::gtk::{
    self as gtk, gdk,
    glib::translate::{IntoGlib, ToGlibPtr},
    prelude::AdjustmentExt,
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

thread_local! {
    static TASKBAR_ADJUSTMENT: RefCell<Option<gtk::Adjustment>> = const { RefCell::new(None) };
}

pub fn set_taskbar_adjustment(adj: gtk::Adjustment) {
    TASKBAR_ADJUSTMENT.with(|cell| {
        *cell.borrow_mut() = Some(adj);
    });
}

pub fn scroll_taskbar(delta: f64) {
    TASKBAR_ADJUSTMENT.with(|cell| {
        if let Some(ref adj) = *cell.borrow() {
            let step = adj.page_size() / 4.0;
            let max = adj.upper() - adj.page_size();
            let new_value = (adj.value() + delta * step).clamp(0.0, max);
            adj.set_value(new_value);
        }
    });
}
