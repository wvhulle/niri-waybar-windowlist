use std::collections::HashMap;

use regex::Regex;
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ProcessInfoConfig {
    pub enabled: bool,
    pub source: ProcessInfoSource,
    #[serde(deserialize_with = "parse_regex_map")]
    pub title_patterns: HashMap<String, Regex>,
    pub poll_interval_ms: u64,
    pub layout: ProcessInfoLayout,
    pub separator: String,
    pub shorten_home: bool,
    pub show_basename_only: bool,
    pub cwd_font_style: FontStyle,
    pub cmd_font_style: FontStyle,
}

impl Default for ProcessInfoConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            source: ProcessInfoSource::default(),
            title_patterns: default_title_patterns(),
            poll_interval_ms: 1000,
            layout: ProcessInfoLayout::default(),
            separator: " · ".to_string(),
            shorten_home: true,
            show_basename_only: true,
            cwd_font_style: FontStyle::Italic,
            cmd_font_style: FontStyle::Normal,
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

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProcessInfoLayout {
    #[default]
    SingleLine,
    TwoLines,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    Bold,
    BoldItalic,
}

fn parse_regex_map<'de, D>(deserializer: D) -> Result<HashMap<String, Regex>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw: HashMap<String, String> = HashMap::deserialize(deserializer)?;
    raw.into_iter()
        .map(|(k, v)| {
            Regex::new(&v)
                .map(|r| (k, r))
                .map_err(serde::de::Error::custom)
        })
        .collect()
}

fn default_title_patterns() -> HashMap<String, Regex> {
    let pattern =
        Regex::new(r"^(?P<cwd>.+?)(?:\s-\s(?P<cmd>.+))?$").expect("default pattern is valid");
    ["foot", "alacritty", "kitty", "wezterm"]
        .into_iter()
        .map(|id| (id.to_string(), pattern.clone()))
        .collect()
}
