use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CompositorIpcError {
    #[error("IPC error: {0}")]
    Io(#[source] io::Error),

    #[error("compositor returned error: {0}")]
    Reply(String),

    #[error("unexpected compositor response; expected {expected}: {actual:?}")]
    UnexpectedResponse {
        expected: &'static str,
        actual: Box<niri_ipc::Response>,
    },

    #[error("event channel closed")]
    EventChannelClosed,
}

impl CompositorIpcError {
    #[must_use]
    pub fn unexpected_response(expected: &'static str, actual: niri_ipc::Response) -> Self {
        Self::UnexpectedResponse {
            expected,
            actual: Box::new(actual),
        }
    }
}
