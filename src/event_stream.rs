use async_channel::Sender;
use futures::StreamExt;
use waybar_cffi::gtk::glib;

use crate::{
    audio,
    compositor::{CompositorEvent, NiriEventStream},
    notifications::{self, NotificationData},
    settings::ProcessInfoSource,
    SharedState,
};

pub enum EventMessage {
    Notification(Box<NotificationData>),
    FullSnapshot(crate::compositor::WindowSnapshot),
    FocusChanged { old: Option<u64>, new: Option<u64> },
    WindowTitleChanged { id: u64, title: Option<String> },
    Workspaces(()),
    AudioUpdate(audio::AudioState),
    ProcessInfoTick,
    ConfigReloaded,
}

pub fn create_event_stream(
    state: &SharedState,
) -> (
    impl futures::Stream<Item = EventMessage>,
    Option<audio::AudioMonitor>,
) {
    let (tx, rx) = async_channel::unbounded();
    let mut audio_monitor = None;

    if state.settings().notifications_enabled() {
        glib::spawn_future_local(forward_notifications(tx.clone()));
    }

    if state.settings().audio_indicator().enabled {
        let (monitor, stream) = audio::start();
        let tx_audio = tx.clone();
        glib::spawn_future_local(async move {
            let mut stream = Box::pin(stream);
            while let Some(state) = stream.next().await {
                if let Err(e) = tx_audio.send(EventMessage::AudioUpdate(state)).await {
                    tracing::error!(%e, "failed to forward audio update");
                }
            }
        });
        audio_monitor = Some(monitor);
    }

    let pi = state.settings().process_info();
    if pi.enabled && pi.source == ProcessInfoSource::Proc {
        let interval = pi.poll_interval_ms;
        glib::spawn_future_local(forward_process_info_ticks(tx.clone(), interval));
    }

    glib::spawn_future_local(forward_compositor_events(
        tx,
        state.compositor().create_event_stream(),
    ));

    let stream = async_stream::stream! {
        while let Ok(event) = rx.recv().await {
            yield event;
        }
    };

    (stream, audio_monitor)
}

async fn forward_notifications(tx: Sender<EventMessage>) {
    let mut notification_stream = Box::pin(notifications::create_stream());
    while let Some(notification) = notification_stream.next().await {
        if let Err(e) = tx
            .send(EventMessage::Notification(Box::new(notification)))
            .await
        {
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
            CompositorEvent::WindowTitleChanged { id, title } => {
                EventMessage::WindowTitleChanged { id, title }
            }
            CompositorEvent::Workspaces => EventMessage::Workspaces(()),
            CompositorEvent::ConfigReloaded => EventMessage::ConfigReloaded,
        };
        if let Err(e) = tx.send(msg).await {
            tracing::error!(%e, "failed to forward compositor event");
        }
    }
}
