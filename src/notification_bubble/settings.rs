use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NotificationConfig {
    pub(crate) enabled: bool,
    pub(crate) map_app_ids: HashMap<String, String>,
    pub(crate) use_desktop_entry: bool,
    pub(crate) use_fuzzy_matching: bool,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            map_app_ids: HashMap::new(),
            use_desktop_entry: true,
            use_fuzzy_matching: false,
        }
    }
}
