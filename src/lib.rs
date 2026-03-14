use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{Arc, LazyLock, Mutex},
};

use futures::StreamExt;
use settings::Settings;
use tracing_subscriber::{EnvFilter, fmt::format::FmtSpan};
use waybar_cffi::{
    Module,
    gtk::{self, Orientation, gio, glib::MainContext, traits::{BoxExt, ContainerExt, CssProviderExt, StyleContextExt, WidgetExt}},
    sys::wbcffi_module,
    waybar_module,
};

mod audio;
mod compositor;
mod errors;
mod global;
mod icons;
mod notifications;
mod screen;
mod settings;
mod system;
mod theme;
mod wayland;
mod widget;

use audio::AudioState;
use compositor::{WindowInfo, WindowSnapshot};
use errors::ModuleError;
use global::{EventMessage, SharedState};
use notifications::NotificationData;
use system::ProcessInfo;
use widget::{WindowButton, SelectionState, FocusedWindow, create_selection_state, create_focused_window, clear_selection};

static LOGGING: LazyLock<()> = LazyLock::new(|| {
    let log_path = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("window-list.log");

    let log_file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        Ok(f) => f,
        Err(e) => {
            eprintln!("failed to open log file {log_path:?}: {e}");
            return;
        }
    };

    if let Err(e) = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("niri_window_buttons=info"))
        )
        .with_span_events(FmtSpan::CLOSE)
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .with_writer(log_file)
        .with_ansi(false)
        .try_init()
    {
        eprintln!("tracing subscriber initialization failed: {e}");
    }
});

/// Handle to signal waybar that the module UI has changed and needs a redraw.
#[derive(Clone)]
struct WaybarUpdater {
    obj: *mut wbcffi_module,
    queue_update: unsafe extern "C" fn(*mut wbcffi_module),
}

// The wbcffi_module pointer is managed by waybar and lives for the module's lifetime.
unsafe impl Send for WaybarUpdater {}
unsafe impl Sync for WaybarUpdater {}

impl WaybarUpdater {
    fn queue_update(&self) {
        unsafe { (self.queue_update)(self.obj) };
    }
}

struct WindowButtonsModule;

impl Module for WindowButtonsModule {
    type Config = Settings;

    fn init(info: &waybar_cffi::InitInfo, settings: Settings) -> Self {
        *LOGGING;

        // Extract the raw queue_update callback from waybar's init_info.
        // The Rust wrapper doesn't expose this, so we access the raw pointer.
        let raw_info = unsafe {
            // InitInfo wraps a *const wbcffi_init_info as its first field
            let ptr: *const *const waybar_cffi::sys::wbcffi_init_info = std::ptr::from_ref(info).cast();
            &**ptr
        };
        let updater = WaybarUpdater {
            obj: raw_info.obj,
            queue_update: raw_info.queue_update.expect("queue_update is not null"),
        };

        let shared_state = SharedState::create(settings);
        let context = MainContext::default();

        if let Err(e) = context.block_on(initialize_module(info, shared_state, updater)) {
            tracing::error!(%e, "module initialization failed");
        }

        Self
    }
}

waybar_module!(WindowButtonsModule);

async fn initialize_module(info: &waybar_cffi::InitInfo, state: SharedState, updater: WaybarUpdater) -> Result<(), ModuleError> {
    let root = info.get_root_widget();

    // Remove rounded corners on button hover (GTK default theme adds border-radius)
    let css_provider = gtk::CssProvider::new();
    css_provider.load_from_data(b"
        .niri-window-buttons button {
            border-radius: 0;
        }
    ").expect("failed to load CSS");
    gtk::StyleContext::add_provider_for_screen(
        &gtk::gdk::Screen::default().expect("no default screen"),
        &css_provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let container = gtk::Box::new(Orientation::Horizontal, 0);
    container.style_context().add_class("niri-window-buttons");

    root.add(&container);

    let context = MainContext::default();
    context.spawn_local(async move {
        ModuleInstance::create(state, container, updater).run_event_loop().await
    });

    Ok(())
}

struct ModuleInstance {
    buttons: BTreeMap<u64, WindowButton>,
    container: gtk::Box,
    previous_snapshot: Option<WindowSnapshot>,
    current_output: Option<String>,
    previous_focused: Option<u64>,
    state: SharedState,
    selection: SelectionState,
    focused_window: FocusedWindow,
    audio_state: AudioState,
    window_pids: HashMap<u64, u32>,
    updater: WaybarUpdater,
}

impl ModuleInstance {
    fn create(state: SharedState, container: gtk::Box, updater: WaybarUpdater) -> Self {
        Self {
            buttons: BTreeMap::new(),
            container,
            previous_snapshot: None,
            current_output: None,
            previous_focused: None,
            state,
            selection: create_selection_state(),
            focused_window: create_focused_window(),
            audio_state: AudioState::new(),
            window_pids: HashMap::new(),
            updater,
        }
    }

    async fn run_event_loop(&mut self) {
        let display_filter = Arc::new(Mutex::new(self.determine_display_filter().await));

        let mut event_stream = Box::pin(self.state.create_event_stream());

        while let Some(event) = event_stream.next().await {
            match event {
                EventMessage::Notification(notif) => self.handle_notification(notif).await,
                EventMessage::AudioUpdate(state) => self.handle_audio_update(state),
                EventMessage::FullSnapshot(snapshot) => {
                    self.handle_window_update(snapshot, display_filter.clone()).await
                }
                EventMessage::FocusChanged { old, new } => {
                    self.handle_focus_change(old, new);
                }
                EventMessage::WindowTitleChanged { id, title } => {
                    self.handle_window_title_update(id, title.as_deref());
                }
                EventMessage::ConfigReloaded => {
                    tracing::info!("config reloaded, refreshing border colors");
                    self.state.reload_border_colors();
                    for button in self.buttons.values() {
                        button.get_widget().queue_draw();
                    }
                }
                EventMessage::Workspaces(_) => {
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

                    if filter_changed && self.update_output_and_resize().await {
                        if let Some(snapshot) = self.previous_snapshot.clone() {
                            let filter = Arc::new(Mutex::new(screen::DisplayFilter::ShowAll));
                            self.handle_window_update(snapshot, filter).await;
                        }
                    }
                }
            }
        }
    }

    async fn update_output_and_resize(&mut self) -> bool {
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
        
        let compositor = self.state.compositor().clone();
        let outputs = compositor.query_outputs().ok()?;
        
        for (output_name, output_info) in outputs.into_iter() {
            let match_result = screen::OutputMatcher::compare(&monitor, &output_info);
            if match_result == screen::OutputMatcher::all() {
                return Some(output_name);
            }
        }
        
        None
    }

    #[tracing::instrument(level = "DEBUG", skip(self))]
    async fn determine_display_filter(&self) -> screen::DisplayFilter {
        if self.state.settings().show_all_outputs() {
            return screen::DisplayFilter::ShowAll;
        }

        let compositor = self.state.compositor().clone();
        let available_outputs = match gio::spawn_blocking(move || compositor.query_outputs()).await {
            Ok(Ok(outputs)) => outputs,
            Ok(Err(e)) => {
                tracing::warn!(%e, "failed to query compositor outputs");
                return screen::DisplayFilter::ShowAll;
            }
            Err(_) => {
                tracing::error!("task spawning error");
                return screen::DisplayFilter::ShowAll;
            }
        };

        if available_outputs.len() == 1 {
            return screen::DisplayFilter::ShowAll;
        }

        let Some(gdk_window) = self.container.window() else {
            tracing::warn!("container has no GDK window");
            return screen::DisplayFilter::ShowAll;
        };

        let display = gdk_window.display();
        let Some(monitor) = display.monitor_at_window(&gdk_window) else {
            tracing::warn!(display = ?gdk_window.display(), geometry = ?gdk_window.geometry(), 
                "no monitor found for window");
            return screen::DisplayFilter::ShowAll;
        };

        for (output_name, output_info) in available_outputs.into_iter() {
            let match_result = screen::OutputMatcher::compare(&monitor, &output_info);
            if match_result == screen::OutputMatcher::all() {
                return screen::DisplayFilter::Only(output_name);
            }
        }

        tracing::warn!(?monitor, "no matching compositor output found");
        screen::DisplayFilter::ShowAll
    }

    #[tracing::instrument(level = "TRACE", skip(self))]
    async fn handle_notification(&mut self, notification: Box<NotificationData>) {
        let Some(windows) = &self.previous_snapshot else {
            return;
        };

        if let Some(mut process_id) = notification.get_process_id() {
            tracing::trace!(process_id, "attempting PID-based notification matching");

            let process_map = ProcessWindowMap::build(windows.iter());
            let mut matched = false;

            loop {
                if let Some(window) = process_map.lookup(process_id) {
                    if !window.is_focused {
                        if let Some(button) = self.buttons.get(&window.id) {
                            tracing::trace!(?button, ?window, process_id, 
                                "marking window as urgent via PID match");
                            button.mark_urgent();
                            matched = true;
                        }
                    }
                }

                match ProcessInfo::query(process_id).await {
                    Ok(ProcessInfo { parent_id }) => {
                        if let Some(parent) = parent_id {
                            process_id = parent;
                        } else {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::info!(process_id, %e, "process tree traversal ended");
                        break;
                    }
                }
            }

            if matched {
                return;
            }
        }

        tracing::trace!("no PID match found for notification");

        if !self.state.settings().notifications_use_desktop_entry() {
            tracing::trace!("desktop entry matching disabled");
            return;
        }

        let Some(desktop_entry) = &notification.get_notification().hints.desktop_entry else {
            tracing::trace!("no desktop entry in notification");
            return;
        };

        let fuzzy_enabled = self.state.settings().notifications_use_fuzzy_matching();
        let mut fuzzy_matches = Vec::new();

        let mapped_entry = self.state.settings()
            .notifications_app_map(desktop_entry)
            .unwrap_or(desktop_entry);
        let entry_lower = mapped_entry.to_lowercase();
        let entry_suffix = mapped_entry.split('.').next_back().unwrap_or_default().to_lowercase();

        let mut exact_match = false;
        for window in windows.iter() {
            let Some(app_identifier) = window.app_id.as_deref() else {
                continue;
            };

            if app_identifier == mapped_entry {
                if let Some(button) = self.buttons.get(&window.id) {
                    tracing::trace!(app_identifier, ?button, ?window, 
                        "exact app ID match for notification");
                    button.mark_urgent();
                    exact_match = true;
                }
            } else if fuzzy_enabled {
                if app_identifier.to_lowercase() == entry_lower {
                    tracing::trace!(app_identifier, ?window, 
                        "case-insensitive app ID match");
                    fuzzy_matches.push(window.id);
                } else if app_identifier.contains('.') {
                    if let Some(suffix) = app_identifier.split('.').next_back() {
                        if suffix.to_lowercase() == entry_suffix {
                            tracing::trace!(app_identifier, ?window, 
                                "suffix-based app ID match");
                            fuzzy_matches.push(window.id);
                        }
                    }
                }
            }
        }

        if !exact_match {
            for window_id in fuzzy_matches {
                if let Some(button) = self.buttons.get(&window_id) {
                    button.mark_urgent();
                }
            }
        }
    }

    #[tracing::instrument(level = "DEBUG", skip(self))]
    async fn handle_window_update(
        &mut self,
        snapshot: WindowSnapshot,
        filter: Arc<Mutex<screen::DisplayFilter>>,
    ) {
        self.update_output_and_resize().await;

        let current_focused = snapshot.iter().find(|w| w.is_focused).map(|w| w.id);
        if current_focused != self.previous_focused {
            clear_selection(&self.selection);
            self.previous_focused = current_focused;
        }

        let mut removed_windows = self.buttons.keys().copied().collect::<BTreeSet<_>>();
        let config = self.state.settings();

        for window in snapshot.iter().filter(|w| {
            let should_display = filter.lock()
                .map(|f| f.should_display(w.get_output().unwrap_or_default()))
                .unwrap_or(true);
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
                self.window_pids.insert(window.id, pid as u32);
            }

            let button = self.buttons.entry(window.id).or_insert_with(|| {
                let btn = WindowButton::create(&self.state, window, self.selection.clone(), self.focused_window.clone());
                self.container.pack_start(btn.get_widget(), false, false, 0);
                btn
            });

            button.update_focus(window.is_focused);
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

    #[tracing::instrument(level = "INFO", skip(self))]
    fn handle_focus_change(&mut self, old: Option<u64>, new: Option<u64>) {
        if old == new {
            return;
        }

        clear_selection(&self.selection);

        if let Some(old_id) = old {
            if let Some(button) = self.buttons.get(&old_id) {
                button.update_focus(false);
            }
        }

        if let Some(new_id) = new {
            if let Some(button) = self.buttons.get(&new_id) {
                button.update_focus(true);
            }
        }

        self.previous_focused = new;
        self.updater.queue_update();
        self.update_button_audio_states();
    }

    #[tracing::instrument(level = "INFO", skip(self))]
    fn handle_window_title_update(&mut self, id: u64, title: Option<&str>) {
        if let Some(button) = self.buttons.get(&id) {
            button.update_title(title);
            self.updater.queue_update();
        }
    }

    fn handle_audio_update(&mut self, state: AudioState) {
        self.audio_state = state;
        self.update_button_audio_states();
    }

    fn update_button_audio_states(&self) {
        let mut pid_window_count: HashMap<u32, usize> = HashMap::new();
        for &pid in self.window_pids.values() {
            *pid_window_count.entry(pid).or_insert(0) += 1;
        }

        for (window_id, button) in &self.buttons {
            let Some(&pid) = self.window_pids.get(window_id) else {
                button.update_audio_state(&[]);
                continue;
            };
            let Some(inputs) = self.audio_state.get(&pid) else {
                button.update_audio_state(&[]);
                continue;
            };
            if pid_window_count.get(&pid).copied().unwrap_or(1) > 1 {
                let focused_has_pid = self.previous_focused
                    .and_then(|fid| self.window_pids.get(&fid))
                    .map(|&fpid| fpid == pid)
                    .unwrap_or(false);
                if focused_has_pid && Some(*window_id) != self.previous_focused {
                    button.update_audio_state(&[]);
                    continue;
                }
            }
            button.update_audio_state(inputs);
        }
    }
}

struct ProcessWindowMap<'a>(HashMap<i64, &'a WindowInfo>);

impl<'a> ProcessWindowMap<'a> {
    fn build(windows: impl Iterator<Item = &'a WindowInfo>) -> Self {
        Self(
            windows
                .filter_map(|w| w.pid.map(|pid| (i64::from(pid), w)))
                .collect()
        )
    }

    fn lookup(&self, pid: i64) -> Option<&'a WindowInfo> {
        self.0.get(&pid).copied()
    }
}