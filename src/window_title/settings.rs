use std::collections::HashMap;

use serde::{Deserialize, Deserializer};

use super::parse::{default_rules, TitleFormatRule};

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TitleDisplayConfig {
    pub(crate) show_window_titles: bool,
    pub(crate) truncate_titles: bool,
    pub(crate) allow_title_linebreaks: bool,
}

impl Default for TitleDisplayConfig {
    fn default() -> Self {
        Self {
            show_window_titles: true,
            truncate_titles: true,
            allow_title_linebreaks: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TitleFormatConfig {
    pub(crate) enabled: bool,
    #[serde(deserialize_with = "deserialize_rules_merged")]
    pub(crate) rules: HashMap<String, TitleFormatRule>,
    pub(crate) poll_interval_ms: u64,
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
