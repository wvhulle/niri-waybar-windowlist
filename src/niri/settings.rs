use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DisplayConfig {
    pub(crate) show_all_outputs: bool,
    pub(crate) only_current_workspace: bool,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            show_all_outputs: false,
            only_current_workspace: true,
        }
    }
}
