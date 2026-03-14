use std::ops::Deref;

pub type WindowSnapshot = Vec<WindowInfo>;

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub(super) inner: niri_ipc::Window,
    pub(super) output_name: Option<String>,
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
