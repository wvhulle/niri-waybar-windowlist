use std::sync::Once;

use tracing_subscriber::{
    fmt::{format::FmtSpan, time::uptime},
    EnvFilter,
};
use waybar_cffi::{waybar_module, Module};

mod app_icon;
mod focus_urgent_indicator;
pub mod mpris_indicator;
mod niri;
mod notification_bubble;
mod right_click_menu;
mod settings;
mod waybar_module;
mod window_button;
mod window_list;
mod window_title;

pub(crate) use waybar_module::SharedState;

static LOGGING_INIT: Once = Once::new();

fn init_logging(level: &settings::LogLevel) {
    LOGGING_INIT.call_once(|| {
        let filter = format!("niri_waybar_windowlist={level}");
        if let Err(e) = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::new(&filter))
            .with_span_events(FmtSpan::CLOSE)
            .with_timer(uptime())
            .with_writer(std::io::stderr)
            .try_init()
        {
            eprintln!("tracing subscriber initialization failed: {e}");
        }
    });
}

struct WindowButtonsModule {
    /// Dropping this sender closes the shutdown channel, causing the spawned
    /// async event-loop task to exit. Without this, the task outlives the C++
    /// module object and dereferences a dangling `WaybarUpdater` pointer on the
    /// next niri event → SIGSEGV.
    _shutdown: async_channel::Sender<()>,
}

impl Module for WindowButtonsModule {
    type Config = settings::Settings;

    fn init(info: &waybar_cffi::InitInfo, settings: settings::Settings) -> Self {
        init_logging(&settings.log_level);

        let updater = waybar_module::WaybarUpdater::from_init_info(info);
        let shutdown = waybar_module::initialize_module(info, settings, updater);

        Self { _shutdown: shutdown }
    }
}

waybar_module!(WindowButtonsModule);
