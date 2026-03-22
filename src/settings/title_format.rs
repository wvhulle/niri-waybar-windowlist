use std::collections::HashMap;

use serde::{Deserialize, Deserializer};

pub use crate::title_format::rules::{default_rules, TitleFormatRule};

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TitleFormatConfig {
    pub enabled: bool,
    /// User-provided rules are merged on top of the built-in defaults,
    /// so specifying a single rule (e.g. `foot`) does not erase the
    /// other built-in rules (kitty, firefox, ...).
    #[serde(deserialize_with = "deserialize_rules_merged")]
    pub rules: HashMap<String, TitleFormatRule>,
    /// Interval in milliseconds for `/proc` polling of terminal foreground processes.
    pub poll_interval_ms: u64,
}

fn deserialize_rules_merged<'de, D>(
    deserializer: D,
) -> Result<HashMap<String, TitleFormatRule>, D::Error>
where
    D: Deserializer<'de>,
{
    let user_rules: HashMap<String, TitleFormatRule> = HashMap::deserialize(deserializer)?;
    let mut merged = default_rules();
    merged.extend(user_rules);
    Ok(merged)
}

impl Default for TitleFormatConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rules: default_rules(),
            poll_interval_ms: 1000,
        }
    }
}
