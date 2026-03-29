use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DragConfig {
    pub(crate) drag_hover_focus: bool,
    pub(crate) drag_hover_focus_delay: u32,
}

impl Default for DragConfig {
    fn default() -> Self {
        Self {
            drag_hover_focus: true,
            drag_hover_focus_delay: 500,
        }
    }
}
