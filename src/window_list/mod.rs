pub(crate) mod settings;
pub(crate) mod window_drag_action;

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use waybar_cffi::gtk;

use crate::window_button::hover_mouse;

pub type SelectionState = Rc<RefCell<HashMap<u64, gtk::EventBox>>>;

pub fn create_selection_state() -> SelectionState {
    Rc::new(RefCell::new(HashMap::new()))
}

pub fn clear_selection(selection: &SelectionState) {
    let mut sel = selection.borrow_mut();
    for (_, event_box) in sel.drain() {
        hover_mouse::set_background_color(&event_box, None);
    }
}

pub type FocusedWindow = Rc<Cell<Option<u64>>>;

pub fn create_focused_window() -> FocusedWindow {
    Rc::new(Cell::new(None))
}
