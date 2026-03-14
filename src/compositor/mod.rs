mod ipc;
mod tracker;

pub mod client;
pub mod event_stream;
pub mod window_info;

pub use client::CompositorClient;
pub use event_stream::{CompositorEvent, NiriEventStream};
pub use window_info::{WindowInfo, WindowSnapshot};
