use std::collections::HashMap;

use serde::Deserialize;

pub use crate::title_format::rules::{default_rules, TitleFormatRule};

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ProcessInfoConfig {
    pub enabled: bool,
    pub source: ProcessInfoSource,
    pub rules: HashMap<String, TitleFormatRule>,
    pub poll_interval_ms: u64,
}

impl Default for ProcessInfoConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            source: ProcessInfoSource::default(),
            rules: default_rules(),
            poll_interval_ms: 1000,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProcessInfoSource {
    #[default]
    TitleRegex,
    Proc,
}
