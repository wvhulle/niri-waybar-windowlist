use std::{io::ErrorKind, thread, time::Duration};

use async_channel::{Receiver, Sender};
use niri_ipc::{Event, Request};

use super::{connect_socket, tracker::WindowTracker, validate_handled, WindowSnapshot};
use crate::{niri::CompositorIpcError, waybar_module::EventMessage};

pub enum CompositorEvent {
    FullSnapshot(WindowSnapshot),
    FocusChanged { old: Option<u64>, new: Option<u64> },
    Workspaces,
    ConfigReloaded,
}

pub struct NiriEventStream {
    receiver: Receiver<CompositorEvent>,
}

impl NiriEventStream {
    pub(super) fn start(filter_workspace: bool) -> Self {
        let (tx, rx) = async_channel::unbounded();
        thread::spawn(move || {
            if let Err(e) = run_event_stream(&tx, filter_workspace) {
                tracing::error!(%e, "niri event stream terminated");
            }
        });

        Self { receiver: rx }
    }

    pub async fn next(&self) -> Option<CompositorEvent> {
        self.receiver.recv().await.ok()
    }
}

pub(crate) async fn forward_events(
    tx: async_channel::Sender<EventMessage>,
    stream: NiriEventStream,
) {
    while let Some(event) = stream.next().await {
        let msg = match event {
            CompositorEvent::FullSnapshot(snapshot) => EventMessage::FullSnapshot(snapshot),
            CompositorEvent::FocusChanged { old, new } => EventMessage::FocusChanged { old, new },
            CompositorEvent::Workspaces => EventMessage::Workspaces(()),
            CompositorEvent::ConfigReloaded => EventMessage::ConfigReloaded,
        };
        if let Err(e) = tx.send(msg).await {
            tracing::error!(%e, "failed to forward compositor event");
        }
    }
}

fn run_event_stream(
    tx: &Sender<CompositorEvent>,
    filter_workspace: bool,
) -> Result<(), CompositorIpcError> {
    const MAX_BACKOFF_SECS: u64 = 30;
    let mut backoff_secs = 1u64;
    let mut window_state = WindowTracker::new();

    loop {
        match try_run_event_stream(tx, &mut window_state, filter_workspace) {
            Ok(()) | Err(CompositorIpcError::EventChannelClosed) => {
                tracing::info!("niri event stream ended");
                return Ok(());
            }
            Err(e) => {
                tracing::warn!(%e, backoff_secs, "niri event stream error, reconnecting");
                thread::sleep(Duration::from_secs(backoff_secs));
                backoff_secs = (backoff_secs * 2).min(MAX_BACKOFF_SECS);
            }
        }
    }
}

fn try_run_event_stream(
    tx: &Sender<CompositorEvent>,
    window_state: &mut WindowTracker,
    filter_workspace: bool,
) -> Result<(), CompositorIpcError> {
    let mut socket = connect_socket()?;
    let response = socket
        .send(Request::EventStream)
        .map_err(CompositorIpcError::Io)?;
    validate_handled(response)?;

    tracing::info!("event stream connected");
    let mut event_reader = socket.read_events();

    loop {
        match event_reader() {
            Ok(event) => {
                let is_workspace_change = matches!(event, Event::WorkspacesChanged { .. });
                let is_config_reload = matches!(event, Event::ConfigLoaded { .. });

                let events = window_state.process_event(event, filter_workspace);
                events.into_iter().try_for_each(|compositor_event| {
                    tx.send_blocking(compositor_event)
                        .map_err(|_| CompositorIpcError::EventChannelClosed)
                })?;

                if is_workspace_change {
                    tx.send_blocking(CompositorEvent::Workspaces)
                        .map_err(|_| CompositorIpcError::EventChannelClosed)?;
                }

                if is_config_reload {
                    tx.send_blocking(CompositorEvent::ConfigReloaded)
                        .map_err(|_| CompositorIpcError::EventChannelClosed)?;
                }
            }
            Err(e) if e.kind() == ErrorKind::InvalidData => {
                tracing::debug!("skipping unknown niri event: {e}");
            }
            Err(e) => {
                return Err(CompositorIpcError::Io(e));
            }
        }
    }
}
