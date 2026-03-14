use niri_ipc::{socket::Socket, Reply, Request};

use crate::CompositorIpcError;

#[tracing::instrument(level = "TRACE", err)]
pub(super) fn send_request(request: Request) -> Result<Reply, CompositorIpcError> {
    connect_socket()?
        .send(request)
        .map_err(CompositorIpcError::Io)
}

#[tracing::instrument(level = "TRACE", err)]
pub(super) fn connect_socket() -> Result<Socket, CompositorIpcError> {
    Socket::connect().map_err(CompositorIpcError::Io)
}

pub(super) fn validate_handled(response: Reply) -> Result<(), CompositorIpcError> {
    match response {
        Ok(niri_ipc::Response::Handled) => Ok(()),
        Ok(other) => Err(CompositorIpcError::unexpected_response("Handled", other)),
        Err(msg) => Err(CompositorIpcError::Reply(msg)),
    }
}
