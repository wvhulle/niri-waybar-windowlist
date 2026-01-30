use std::sync::Arc;
use async_channel::Sender;
use futures::{Stream, StreamExt};
use waybar_cffi::gtk::glib;
use crate::{
    compositor::{CompositorClient, StreamShutdownHandle, WindowSnapshot, WorkspaceEventStream},
    icons::IconResolver,
    notifications::{self, NotificationData},
    session,
    settings::Settings,
};

#[derive(Debug, Clone)]
pub struct SharedState(Arc<StateInner>);

#[derive(Debug)]
struct StateInner {
    settings: Settings,
    icon_resolver: IconResolver,
    compositor: CompositorClient,
}

impl SharedState {
    pub fn create(settings: Settings) -> Self {
        Self(Arc::new(StateInner {
            compositor: CompositorClient::create(settings.clone()),
            icon_resolver: IconResolver::new(),
            settings,
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

    pub fn create_event_stream(&self) -> impl Stream<Item = EventMessage> {
        let (tx, rx) = async_channel::unbounded();

        if self.settings().notifications_enabled() {
            glib::spawn_future_local(forward_notifications(tx.clone()));
        }

        let window_stream = self.compositor().create_window_stream();
        let workspace_stream = self.compositor().create_workspace_stream();

        let window_handle = window_stream.shutdown_handle().clone();
        let workspace_handle = workspace_stream.shutdown_handle().clone();

        glib::spawn_future_local(forward_window_updates(tx.clone(), window_stream));
        glib::spawn_future_local(forward_workspace_changes(tx, workspace_stream));
        glib::spawn_future_local(watch_session_unlock(window_handle, workspace_handle));

        async_stream::stream! {
            while let Ok(event) = rx.recv().await {
                yield event;
            }
        }
    }
}

pub enum EventMessage {
    Notification(Box<NotificationData>),
    WindowUpdate(WindowSnapshot),
    Workspaces(()),
}

async fn forward_notifications(tx: Sender<EventMessage>) {
    let mut notification_stream = Box::pin(notifications::create_stream());
    while let Some(notification) = notification_stream.next().await {
        if let Err(e) = tx.send(EventMessage::Notification(Box::new(notification))).await {
            tracing::error!(%e, "failed to forward notification");
        }
    }
}

async fn forward_window_updates(tx: Sender<EventMessage>, stream: crate::compositor::WindowEventStream) {
    while let Some(snapshot) = stream.next_snapshot().await {
        if let Err(e) = tx.send(EventMessage::WindowUpdate(snapshot)).await {
            tracing::error!(%e, "failed to forward window update");
        }
    }
}

async fn forward_workspace_changes(tx: Sender<EventMessage>, stream: WorkspaceEventStream) {
    while stream.next_workspaces().await.is_some() {
        if let Err(e) = tx.send(EventMessage::Workspaces(())).await {
            tracing::error!(%e, "failed to forward workspace change");
        }
    }
}

async fn watch_session_unlock(window_handle: StreamShutdownHandle, workspace_handle: StreamShutdownHandle) {
    let mut unlock = Box::pin(session::unlock_stream());
    while unlock.next().await.is_some() {
        tracing::info!("session unlocked, interrupting event streams to force reconnection");
        window_handle.interrupt();
        workspace_handle.interrupt();
    }
}
