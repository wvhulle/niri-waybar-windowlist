mod tracker;
pub(crate) mod settings;

pub mod border_colors;
pub mod client;
pub mod event_stream;
pub mod output_matching;

use std::ops::Deref;

pub use client::CompositorClient;
use niri_ipc::{socket::Socket, Reply, Request};

use crate::CompositorIpcError;

// ── IPC helpers (from ipc.rs) ──

#[tracing::instrument(level = "TRACE", err)]
pub(crate) fn send_request(request: Request) -> Result<Reply, CompositorIpcError> {
    connect_socket()?
        .send(request)
        .map_err(CompositorIpcError::Io)
}

#[tracing::instrument(level = "TRACE", err)]
pub(crate) fn connect_socket() -> Result<Socket, CompositorIpcError> {
    Socket::connect().map_err(CompositorIpcError::Io)
}

pub(crate) fn validate_handled(response: Reply) -> Result<(), CompositorIpcError> {
    match response {
        Ok(niri_ipc::Response::Handled) => Ok(()),
        Ok(other) => Err(CompositorIpcError::unexpected_response("Handled", other)),
        Err(msg) => Err(CompositorIpcError::Reply(msg)),
    }
}

// ── WindowInfo (from window_info.rs) ──

pub type WindowSnapshot = Vec<WindowInfo>;

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub(crate) inner: niri_ipc::Window,
    pub(crate) output_name: Option<String>,
}

impl WindowInfo {
    pub fn get_output(&self) -> Option<&str> {
        self.output_name.as_deref()
    }
}

impl Deref for WindowInfo {
    type Target = niri_ipc::Window;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
