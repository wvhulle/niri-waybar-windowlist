use std::cell::Cell;
use std::sync::Arc;
use async_channel::Sender;
use futures::{Stream, StreamExt};
use waybar_cffi::gtk::glib;
use crate::{
    audio,
    compositor::{CompositorClient, CompositorEvent, NiriEventStream, WindowSnapshot},
    icons::IconResolver,
    notifications::{self, NotificationData},
    settings::{ProcessInfoSource, Settings},
    theme::BorderColors,
    wayland::WaylandActivator,
};

#[derive(Debug, Clone)]
pub struct SharedState(Arc<StateInner>);

#[derive(Debug)]
struct StateInner {
    settings: Settings,
    icon_resolver: IconResolver,
    compositor: CompositorClient,
    wayland_activator: Option<WaylandActivator>,
}

thread_local! {
    static BORDER_COLORS: Cell<BorderColors> = Cell::new(crate::theme::load_border_colors());
}

impl SharedState {
    pub fn create(settings: Settings) -> Self {
        let colors = crate::theme::load_border_colors();
        BORDER_COLORS.with(|cell| cell.set(colors));
        let wayland_activator = WaylandActivator::connect();
        Self(Arc::new(StateInner {
            compositor: CompositorClient::create(settings.clone()),
            icon_resolver: IconResolver::new(),
            settings,
            wayland_activator,
        }))
    }

    pub fn settings(&self) -> &Settings {
        &self.0.settings
    }

    pub fn icon_resolver(&self) -> &IconResolver {
        &self.0.icon_resolver
    }

    pub fn compositor(&self) -> &CompositorClient {
        &self.0.compositor
    }

    pub fn wayland_activator(&self) -> Option<&WaylandActivator> {
        self.0.wayland_activator.as_ref()
    }

    pub fn border_colors(&self) -> BorderColors {
        BORDER_COLORS.with(|cell| cell.get())
    }

    pub fn reload_border_colors(&self) {
        let colors = crate::theme::load_border_colors();
        BORDER_COLORS.with(|cell| cell.set(colors));
        tracing::info!("border colors reloaded");
    }

    pub fn create_event_stream(&self) -> impl Stream<Item = EventMessage> {
        let (tx, rx) = async_channel::unbounded();

        if self.settings().notifications_enabled() {
            glib::spawn_future_local(forward_notifications(tx.clone()));
        }

        if self.settings().audio_indicator().enabled {
            glib::spawn_future_local(forward_audio_updates(tx.clone()));
        }

        let pi = self.settings().process_info();
        if pi.enabled && pi.source == ProcessInfoSource::Proc {
            let interval = pi.poll_interval_ms;
            glib::spawn_future_local(forward_process_info_ticks(tx.clone(), interval));
        }

        glib::spawn_future_local(forward_compositor_events(tx, self.compositor().create_event_stream()));

        async_stream::stream! {
            while let Ok(event) = rx.recv().await {
                yield event;
            }
        }
    }
}

pub enum EventMessage {
    Notification(Box<NotificationData>),
    FullSnapshot(WindowSnapshot),
    FocusChanged { old: Option<u64>, new: Option<u64> },
    WindowTitleChanged { id: u64, title: Option<String> },
    Workspaces(()),
    AudioUpdate(audio::AudioState),
    ProcessInfoTick,
    ConfigReloaded,
}

async fn forward_audio_updates(tx: Sender<EventMessage>) {
    let mut stream = Box::pin(audio::create_stream());
    while let Some(state) = stream.next().await {
        if let Err(e) = tx.send(EventMessage::AudioUpdate(state)).await {
            tracing::error!(%e, "failed to forward audio update");
        }
    }
}

async fn forward_notifications(tx: Sender<EventMessage>) {
    let mut notification_stream = Box::pin(notifications::create_stream());
    while let Some(notification) = notification_stream.next().await {
        if let Err(e) = tx.send(EventMessage::Notification(Box::new(notification))).await {
            tracing::error!(%e, "failed to forward notification");
        }
    }
}

async fn forward_process_info_ticks(tx: Sender<EventMessage>, interval_ms: u64) {
    loop {
        glib::timeout_future(std::time::Duration::from_millis(interval_ms)).await;
        if let Err(e) = tx.send(EventMessage::ProcessInfoTick).await {
            tracing::error!(%e, "failed to forward process info tick");
            break;
        }
    }
}

async fn forward_compositor_events(tx: Sender<EventMessage>, stream: NiriEventStream) {
    while let Some(event) = stream.next().await {
        let msg = match event {
            CompositorEvent::FullSnapshot(snapshot) => EventMessage::FullSnapshot(snapshot),
            CompositorEvent::FocusChanged { old, new } => EventMessage::FocusChanged { old, new },
            CompositorEvent::WindowTitleChanged { id, title } => EventMessage::WindowTitleChanged { id, title },
            CompositorEvent::Workspaces => EventMessage::Workspaces(()),
            CompositorEvent::ConfigReloaded => EventMessage::ConfigReloaded,
        };
        if let Err(e) = tx.send(msg).await {
            tracing::error!(%e, "failed to forward compositor event");
        }
    }
}
