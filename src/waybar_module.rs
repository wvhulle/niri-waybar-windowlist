use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    ptr,
    sync::{Arc, Mutex},
};

use futures::StreamExt;
use waybar_cffi::gtk::{
    self, gio, glib,
    glib::MainContext,
    traits::{BoxExt, ContainerExt, WidgetExt},
    Orientation,
};
use waybar_cffi::sys::{wbcffi_init_info, wbcffi_module};

use crate::{
    app_icon::image_resolve::IconResolver,
    mpris_indicator::{self, AudioState},
    niri::{
        border_colors::{load_border_colors, BorderColors},
        event_stream,
        output_matching::{DisplayFilter, OutputMatcher},
        CompositorClient, WindowSnapshot,
    },
    notification_bubble::{self, NotificationData},
    settings::Settings,
    window_button::WindowButton,
    window_list::{
        clear_selection, create_focused_window, create_selection_state, FocusedWindow,
        SelectionState,
    },
    window_title::guess_term_proc_info::{self as proc_info},
};

pub(crate) type SharedState = Arc<SharedStateInner>;

#[derive(Debug)]
pub(crate) struct SharedStateInner {
    pub settings: Settings,
    pub icon_resolver: IconResolver,
    pub compositor: CompositorClient,
    pub border_colors: Mutex<BorderColors>,
}

fn create_shared_state(settings: Settings) -> SharedState {
    let colors = load_border_colors();
    Arc::new(SharedStateInner {
        compositor: CompositorClient::create(settings.clone()),
        icon_resolver: IconResolver::new(),
        settings,
        border_colors: Mutex::new(colors),
    })
}

#[derive(Clone)]
pub(crate) struct WaybarUpdater {
    obj: *mut wbcffi_module,
    queue_update: unsafe extern "C" fn(*mut wbcffi_module),
}

unsafe impl Send for WaybarUpdater {}
unsafe impl Sync for WaybarUpdater {}

impl WaybarUpdater {
    pub(crate) fn from_init_info(info: &waybar_cffi::InitInfo) -> Self {
        let raw_info = unsafe {
            let ptr: *const *const wbcffi_init_info = ptr::from_ref(info).cast();
            &**ptr
        };
        Self {
            obj: raw_info.obj,
            queue_update: raw_info.queue_update.expect("queue_update is not null"),
        }
    }

    fn queue_update(&self) {
        unsafe { (self.queue_update)(self.obj) };
    }
}

pub(crate) fn initialize_module(
    info: &waybar_cffi::InitInfo,
    settings: Settings,
    updater: WaybarUpdater,
) {
    let state = create_shared_state(settings);
    let root = info.get_root_widget();

    let container = gtk::Box::new(Orientation::Horizontal, 0);
    container.set_vexpand(true);
    container.set_hexpand(true);

    root.add(&container);

    let context = MainContext::default();
    context.spawn_local(async move {
        ModuleInstance::create(state, container, updater)
            .run_event_loop()
            .await;
    });
}

// ── Event message ──

pub(crate) enum EventMessage {
    Notification(Box<NotificationData>),
    FullSnapshot(WindowSnapshot),
    FocusChanged { old: Option<u64>, new: Option<u64> },
    Workspaces(()),
    AudioUpdate(AudioState),
    ProcessInfoTick,
    ConfigReloaded,
}

fn create_event_stream(
    state: &SharedState,
) -> (
    impl futures::Stream<Item = EventMessage>,
    Option<mpris_indicator::AudioMonitor>,
) {
    let (tx, rx) = async_channel::unbounded();
    let mut audio_monitor = None;

    if state.settings.notifications_enabled() {
        glib::spawn_future_local(notification_bubble::forward_events(tx.clone()));
    }

    if state.settings.audio_indicator().enabled {
        let (monitor, stream) = mpris_indicator::start();
        glib::spawn_future_local(mpris_indicator::forward_events(tx.clone(), stream));
        audio_monitor = Some(monitor);
    }

    if let Some(interval_ms) = state.settings.proc_poll_interval() {
        tracing::info!(interval_ms, "starting proc poll timer");
        glib::spawn_future_local(proc_info::forward_poll_ticks(tx.clone(), interval_ms));
    } else {
        tracing::info!("proc polling disabled (no poll_proc rules)");
    }

    glib::spawn_future_local(event_stream::forward_events(
        tx,
        state.compositor.create_event_stream(),
    ));

    let stream = async_stream::stream! {
        while let Ok(event) = rx.recv().await {
            yield event;
        }
    };

    (stream, audio_monitor)
}


// ── Module instance ──

pub(crate) struct ModuleInstance {
    buttons: BTreeMap<u64, WindowButton>,
    container: gtk::Box,
    previous_snapshot: Option<WindowSnapshot>,
    current_output: Option<String>,
    previous_focused: Option<u64>,
    state: SharedState,
    selection: SelectionState,
    focused_window: FocusedWindow,
    audio_state: AudioState,
    audio_monitor_handle: Option<mpris_indicator::AudioMonitor>,
    window_pids: HashMap<u64, u32>,
    updater: WaybarUpdater,
}

impl ModuleInstance {
    pub(crate) fn create(state: SharedState, container: gtk::Box, updater: WaybarUpdater) -> Self {
        Self {
            buttons: BTreeMap::new(),
            container,
            previous_snapshot: None,
            current_output: None,
            previous_focused: None,
            state,
            selection: create_selection_state(),
            focused_window: create_focused_window(),
            audio_state: AudioState::default(),
            audio_monitor_handle: None,
            window_pids: HashMap::new(),
            updater,
        }
    }

    pub(crate) async fn run_event_loop(&mut self) {
        let display_filter = Arc::new(Mutex::new(self.determine_display_filter().await));

        let (stream, audio_monitor) = create_event_stream(&self.state);
        self.audio_monitor_handle = audio_monitor;
        let mut event_stream = Box::pin(stream);

        while let Some(event) = event_stream.next().await {
            match event {
                EventMessage::Notification(notif) => self.handle_notification(notif),
                EventMessage::AudioUpdate(state) => self.handle_audio_update(state),
                EventMessage::ProcessInfoTick => self.handle_process_info_tick(),
                EventMessage::FullSnapshot(snapshot) => {
                    self.handle_window_update(snapshot, display_filter.clone())
                        .await;
                }
                EventMessage::FocusChanged { old, new } => {
                    self.handle_focus_change(old, new);
                }
                EventMessage::ConfigReloaded => {
                    *self.state.border_colors.lock().unwrap() = load_border_colors();
                    self.buttons
                        .values()
                        .for_each(|button| button.get_widget().queue_draw());
                }
                EventMessage::Workspaces(()) => {
                    let updated_filter = self.determine_display_filter().await;
                    let filter_changed = {
                        let Ok(mut filter_lock) = display_filter.lock() else {
                            tracing::warn!("display filter lock poisoned");
                            continue;
                        };
                        let changed = *filter_lock != updated_filter;
                        *filter_lock = updated_filter;
                        changed
                    };

                    if filter_changed && self.update_output_and_resize() {
                        if let Some(snapshot) = self.previous_snapshot.clone() {
                            let filter = Arc::new(Mutex::new(DisplayFilter::ShowAll));
                            self.handle_window_update(snapshot, filter).await;
                        }
                    }
                }
            }
        }
    }

    fn update_output_and_resize(&mut self) -> bool {
        let new_output = self.get_current_output_name();

        if self.current_output.as_deref() != new_output.as_deref() {
            self.current_output = new_output;
            return true;
        }

        false
    }

    fn get_current_output_name(&self) -> Option<String> {
        let gdk_window = self.container.window()?;
        let display = gdk_window.display();
        let monitor = display.monitor_at_window(&gdk_window)?;

        let outputs = CompositorClient::query_outputs().ok()?;

        outputs.into_iter().find_map(|(output_name, output_info)| {
            (OutputMatcher::compare(&monitor, &output_info) == OutputMatcher::all())
                .then_some(output_name)
        })
    }

    #[tracing::instrument(level = "DEBUG", skip(self))]
    async fn determine_display_filter(&self) -> DisplayFilter {
        if self.state.settings.show_all_outputs() {
            return DisplayFilter::ShowAll;
        }

        let available_outputs = match gio::spawn_blocking(CompositorClient::query_outputs).await {
            Ok(Ok(outputs)) => outputs,
            Ok(Err(e)) => {
                tracing::warn!(%e, "failed to query compositor outputs");
                return DisplayFilter::ShowAll;
            }
            Err(_) => {
                tracing::error!("task spawning error");
                return DisplayFilter::ShowAll;
            }
        };

        if available_outputs.len() == 1 {
            return DisplayFilter::ShowAll;
        }

        let Some(gdk_window) = self.container.window() else {
            tracing::warn!("container has no GDK window");
            return DisplayFilter::ShowAll;
        };

        let display = gdk_window.display();
        let Some(monitor) = display.monitor_at_window(&gdk_window) else {
            tracing::warn!(display = ?gdk_window.display(), geometry = ?gdk_window.geometry(),
                "no monitor found for window");
            return DisplayFilter::ShowAll;
        };

        available_outputs
            .into_iter()
            .find_map(|(output_name, output_info)| {
                (OutputMatcher::compare(&monitor, &output_info) == OutputMatcher::all())
                    .then_some(DisplayFilter::Only(output_name))
            })
            .unwrap_or_else(|| {
                tracing::warn!(?monitor, "no matching compositor output found");
                DisplayFilter::ShowAll
            })
    }

    #[tracing::instrument(level = "TRACE", skip(self))]
    fn handle_notification(&mut self, notification: Box<NotificationData>) {
        let Some(windows) = &self.previous_snapshot else {
            return;
        };

        for (window_id, urgency) in notification_bubble::match_notification(&notification, windows)
        {
            if let Some(button) = self.buttons.get(&window_id) {
                button.mark_notification_urgent(urgency);
            }
        }
    }

    #[tracing::instrument(level = "DEBUG", skip(self))]
    async fn handle_window_update(
        &mut self,
        snapshot: WindowSnapshot,
        filter: Arc<Mutex<DisplayFilter>>,
    ) {
        self.update_output_and_resize();

        let current_focused = snapshot.iter().find(|w| w.is_focused).map(|w| w.id);
        if current_focused != self.previous_focused {
            clear_selection(&self.selection);
            self.previous_focused = current_focused;
        }

        let mut removed_windows = self.buttons.keys().copied().collect::<BTreeSet<_>>();
        let config = &self.state.settings;

        for window in snapshot.iter().filter(|w| {
            let should_display = filter.lock().map_or(true, |f| {
                f.should_display(w.get_output().unwrap_or_default())
            });
            if !should_display {
                return false;
            }
            if let Some(_app_id) = &w.app_id {
                if config.should_ignore(w.app_id.as_deref(), w.title.as_deref(), w.workspace_id) {
                    return false;
                }
            }
            true
        }) {
            if let Some(pid) = window.pid {
                self.window_pids.insert(window.id, pid.cast_unsigned());
            }

            let button = self.buttons.entry(window.id).or_insert_with(|| {
                let btn = WindowButton::create(
                    &self.state,
                    window,
                    self.selection.clone(),
                    self.focused_window.clone(),
                );
                self.container.pack_start(btn.get_widget(), true, true, 0);
                btn
            });

            button.update_focus(window.is_focused, window.is_urgent);
            button.update_title(window.title.as_deref());

            removed_windows.remove(&window.id);
            self.container.reorder_child(button.get_widget(), -1);
        }

        for window_id in removed_windows {
            if let Some(button) = self.buttons.remove(&window_id) {
                self.container.remove(button.get_widget());
            }
            self.selection.borrow_mut().remove(&window_id);
            self.window_pids.remove(&window_id);
        }

        self.container.show_all();
        self.updater.queue_update();
        self.previous_snapshot = Some(snapshot);
        self.update_button_audio_states();
    }

    #[tracing::instrument(level = "DEBUG", skip(self))]
    fn handle_focus_change(&mut self, old: Option<u64>, new: Option<u64>) {
        if old == new {
            return;
        }

        clear_selection(&self.selection);

        if let Some(old_id) = old {
            if let Some(button) = self.buttons.get(&old_id) {
                button.update_focus(false, false);
            }
        }

        if let Some(new_id) = new {
            if let Some(button) = self.buttons.get(&new_id) {
                button.update_focus(true, false);
            }
        }

        self.previous_focused = new;
        self.updater.queue_update();
        self.update_button_audio_states();
    }

    fn handle_audio_update(&mut self, state: AudioState) {
        self.audio_state = state;
        self.update_button_audio_states();
    }

    fn handle_process_info_tick(&mut self) {
        let mut any_changed = false;

        let pollable: Vec<_> = self
            .window_pids
            .iter()
            .filter(|(wid, _)| {
                self.buttons
                    .get(wid)
                    .is_some_and(|b| self.state.settings.should_poll_proc(b.app_id.as_deref()))
            })
            .map(|(&wid, &pid)| (wid, pid))
            .collect();

        // Count how many windows share each PID. When multiple windows
        // report the same PID (e.g. kitty), `/proc` polling returns
        // identical results for all of them, overwriting the correct
        // per-window titles that the compositor provides.
        let mut pid_refcount: HashMap<u32, usize> = HashMap::new();
        for &(_, pid) in &pollable {
            *pid_refcount.entry(pid).or_default() += 1;
        }

        let pids_to_query: Vec<_> = pollable
            .into_iter()
            .filter(|&(_, pid)| pid_refcount.get(&pid).copied().unwrap_or(0) == 1)
            .collect();

        tracing::debug!(count = pids_to_query.len(), "process info tick");

        for (wid, pid) in pids_to_query {
            match proc_info::query_foreground(pid) {
                Ok(info) => {
                    tracing::debug!(window_id = wid, pid, cwd = ?info.cwd, cmd = ?info.command, "proc query result");
                    if let Some(button) = self.buttons.get(&wid) {
                        button.update_process_info(info.cwd.as_deref(), info.command.as_deref());
                        any_changed = true;
                    }
                }
                Err(e) => {
                    tracing::debug!(window_id = wid, pid, %e, "process info query failed");
                }
            }
        }

        if any_changed {
            self.updater.queue_update();
        }
    }

    fn update_button_audio_states(&self) {
        for button in self.buttons.values() {
            let status = button.app_id.as_deref().and_then(|app_id| {
                let id_lower = app_id.to_lowercase();
                self.audio_state
                    .by_desktop_entry
                    .get(&id_lower)
                    .or_else(|| self.audio_state.by_bus_suffix.get(&id_lower))
                    .copied()
            });
            button.update_audio_state(status);
        }
    }
}
