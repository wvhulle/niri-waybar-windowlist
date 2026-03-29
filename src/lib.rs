use std::{
    ptr,
    sync::{Arc, LazyLock, Mutex},
};

use settings::Settings;
use tracing_subscriber::{
    fmt::{format::FmtSpan, time::uptime},
    EnvFilter,
};
use waybar_cffi::{
    gtk::{
        self,
        glib::MainContext,
        traits::{ContainerExt, WidgetExt},
        Orientation,
    },
    sys::{wbcffi_init_info, wbcffi_module},
    waybar_module, Module,
};

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

use app_icon::image_resolve::IconResolver;
use niri::{
    border_colors::{load_border_colors, BorderColors},
    CompositorClient,
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

// ── Logging ──

static LOGGING: LazyLock<()> = LazyLock::new(|| {
    if let Err(e) = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("niri_waybar_windowlist=info")),
        )
        .with_span_events(FmtSpan::CLOSE)
        .with_timer(uptime())
        .with_writer(std::io::stderr)
        .try_init()
    {
        eprintln!("tracing subscriber initialization failed: {e}");
    }
});

#[derive(Clone)]
pub(crate) struct WaybarUpdater {
    obj: *mut wbcffi_module,
    queue_update: unsafe extern "C" fn(*mut wbcffi_module),
}

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

        let raw_info = unsafe {
            let ptr: *const *const wbcffi_init_info = ptr::from_ref(info).cast();
            &**ptr
        };
        let updater = WaybarUpdater {
            obj: raw_info.obj,
            queue_update: raw_info.queue_update.expect("queue_update is not null"),
        };

        let shared_state = create_shared_state(settings);

        initialize_module(info, shared_state, updater);

        Self
    }
}

waybar_module!(WindowButtonsModule);

fn initialize_module(info: &waybar_cffi::InitInfo, state: SharedState, updater: WaybarUpdater) {
    let root = info.get_root_widget();

    let container = gtk::Box::new(Orientation::Horizontal, 0);
    container.set_vexpand(true);
    container.set_hexpand(true);

    root.add(&container);

    let context = MainContext::default();
    context.spawn_local(async move {
        waybar_module::ModuleInstance::create(state, container, updater)
            .run_event_loop()
            .await;
    });
}
