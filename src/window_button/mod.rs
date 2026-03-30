pub(crate) mod context_click;
pub(crate) mod focus_click;
pub(crate) mod hover_mouse;
pub(crate) mod settings;

use std::{
    cell::{Cell, RefCell},
    fmt::{self, Debug},
    rc::Rc,
};


use waybar_cffi::gtk::{
    self as gtk, gdk,
    glib::SourceId,
    pango::{AttrInt, AttrList, EllipsizeMode, Weight},
    prelude::{ContainerExt, EventBoxExt, LabelExt, WidgetExt, WidgetExtManual},
    EventBox, Orientation,
};

use crate::{
    app_icon::style::{setup_icon_rendering, IconRenderingParams},
    focus_urgent_indicator::Indicator,
    mpris_indicator::{style::update_audio_state, PlaybackStatus},
    notification_bubble::style::{
        clear_notification_urgent, mark_notification_urgent, setup_notification_bubble,
        BubbleState, NotificationUrgency,
    },
    window_list::SelectionState,
    window_title::style::{update_process_info, update_title},
    SharedState,
};

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
    pub(crate) tooltip_timeout: Rc<RefCell<Option<SourceId>>>,
    pub(crate) skip_clicked: Rc<RefCell<bool>>,
    pub(crate) indicator: Indicator,
    pub(crate) bubble_state: Rc<BubbleState>,
}

impl Debug for WindowButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WindowButton")
            .field("app_id", &self.app_id)
            .field("display_titles", &self.display_titles)
            .field("window_id", &self.window_id)
            .finish_non_exhaustive()
    }
}

impl WindowButton {
    #[tracing::instrument(level = "TRACE", fields(app_id = &window.app_id))]
    pub fn create(
        state: &SharedState,
        window: &niri_ipc::Window,
        selection: SelectionState,
        focused_window: Rc<Cell<Option<u64>>>,
    ) -> Self {
        let state_clone = state.clone();
        let display_titles = state.settings.show_window_titles();

        let icon_gap = state.settings.icon_spacing();
        let layout_box = gtk::Box::new(Orientation::Horizontal, icon_gap);
        layout_box.set_vexpand(true);
        layout_box.set_margin_start(4);
        layout_box.set_margin_end(4);
        layout_box.set_margin_top(4);
        layout_box.set_margin_bottom(0);

        let title_label = gtk::Label::new(None);
        title_label.set_hexpand(true);
        let truncate_titles = state.settings.truncate_titles();
        if truncate_titles {
            title_label.set_ellipsize(EllipsizeMode::End);
        } else {
            title_label.set_ellipsize(EllipsizeMode::None);
        }
        title_label.set_xalign(0.0);

        let attrs = AttrList::new();
        attrs.insert(AttrInt::new_weight(Weight::Normal));
        title_label.set_attributes(Some(&attrs));

        let bubble_state = Rc::new(BubbleState {
            active: Cell::new(false),
            urgency: Cell::new(NotificationUrgency::default()),
        });

        let event_box = gtk::EventBox::new();
        event_box.set_visible_window(true);
        event_box.set_vexpand(true);
        event_box.add(&layout_box);

        let indicator = Indicator::new(&event_box, window.id, focused_window);
        setup_notification_bubble(&bubble_state, &event_box, Default::default(), None);
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
            .and_then(|id| state_clone.icon_resolver.resolve(id));

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
            tooltip_timeout: Rc::new(RefCell::new(None)),
            skip_clicked: Rc::new(RefCell::new(false)),
            indicator,
            bubble_state,
        };

        button.setup_click_handlers(window.id);
        button.setup_hover();
        button.setup_drag_reorder();
        setup_icon_rendering(
            &button.event_box,
            &button.layout_box,
            &button.title_label,
            &button.audio_event_box,
            &button.audio_visible,
            IconRenderingParams {
                display_titles: button.display_titles,
                icon_size: state.settings.icon_size(),
                icon_path: icon_location,
            },
        );
        button.setup_tooltip();

        button
    }

    #[tracing::instrument(level = "TRACE")]
    pub fn update_focus(&self, is_focused: bool, is_urgent: bool) {
        if is_focused {
            clear_notification_urgent(&self.event_box, &self.bubble_state);
        }
        let colors = *self.state.border_colors.lock().unwrap();
        self.indicator.update(&colors, is_focused, is_urgent);
    }

    #[tracing::instrument(level = "TRACE")]
    pub fn mark_notification_urgent(&self, urgency: NotificationUrgency) {
        mark_notification_urgent(&self.event_box, &self.bubble_state, urgency);
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
            tooltip_timeout: self.tooltip_timeout.clone(),
            skip_clicked: self.skip_clicked.clone(),
            indicator: self.indicator.clone(),
            bubble_state: self.bubble_state.clone(),
        }
    }

    pub fn update_audio_state(&self, status: Option<PlaybackStatus>) {
        update_audio_state(
            &self.audio_event_box,
            &self.audio_label,
            self.state.settings.audio_indicator(),
            status,
        );
    }

    #[tracing::instrument(level = "TRACE")]
    pub fn update_title(&self, title: Option<&str>) {
        update_title(
            &self.title_label,
            &self.title,
            self.window_id,
            self.app_id.as_deref(),
            &self.state.settings,
            self.display_titles,
            title,
        );
    }

    #[tracing::instrument(level = "TRACE")]
    pub fn update_process_info(&self, cwd: Option<&str>, command: Option<&str>) {
        update_process_info(
            &self.title_label,
            &self.title,
            self.app_id.as_deref(),
            &self.state.settings,
            self.display_titles,
            cwd,
            command,
        );
    }
}
