use async_channel::{Receiver, Sender};
use niri_ipc::{Event, Request};

use super::{connect_socket, tracker::WindowTracker, validate_handled, WindowSnapshot};
use crate::CompositorIpcError;

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
        std::thread::spawn(move || {
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
                std::thread::sleep(std::time::Duration::from_secs(backoff_secs));
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
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                tracing::debug!("skipping unknown niri event: {e}");
            }
            Err(e) => {
                return Err(CompositorIpcError::Io(e));
            }
        }
    }
}
