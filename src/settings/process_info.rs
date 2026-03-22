use std::collections::HashMap;

use regex::Regex;
use serde::{Deserialize, Deserializer};

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

#[derive(Debug, Clone, Deserialize)]
pub struct TitleFormatRule {
    #[serde(deserialize_with = "parse_regex")]
    pub pattern: Regex,
    pub format: String,
}

fn parse_regex<'de, D>(deserializer: D) -> Result<Regex, D::Error>
where
    D: Deserializer<'de>,
{
    let pattern = String::deserialize(deserializer)?;
    Regex::new(&pattern).map_err(serde::de::Error::custom)
}

fn deserialize_rules_with_defaults<'de, D>(
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

fn rule(pattern: &str, format: &str) -> TitleFormatRule {
    TitleFormatRule {
        pattern: Regex::new(pattern).expect("builtin pattern is valid"),
        format: format.to_string(),
    }
}

fn default_rules() -> HashMap<String, TitleFormatRule> {
    let terminal_pattern = r"^(?P<cwd>.+?)(?:\s-\s(?P<cmd>.+))?$";
    let terminal_format =
        "<i>{{ cwd | shorten_home }}</i>{% if cmd %} · {{ cmd }}{% endif %}";

    // "Page Title — Site Name" or "Page Title - Site Name"
    let browser_pattern = r"^(?P<page>.+?)(?:\s[—-]\s(?P<site>.+))?$";
    let browser_format =
        "{{ page }}{% if site %} <span alpha='60%'>— {{ site }}</span>{% endif %}";

    // "filename - Editor Name" or "filename · Editor Name"
    let editor_pattern = r"^(?P<file>.+?)(?:\s[-·]\s(?P<editor>.+))?$";
    let editor_format =
        "<b>{{ file | basename }}</b>{% if editor %} <span alpha='60%'>— {{ editor }}</span>{% endif %}";

    [
        // Terminals
        ("foot", rule(terminal_pattern, terminal_format)),
        ("Alacritty", rule(terminal_pattern, terminal_format)),
        ("kitty", rule(terminal_pattern, terminal_format)),
        ("wezterm", rule(terminal_pattern, terminal_format)),
        ("ghostty", rule(terminal_pattern, terminal_format)),
        ("org.wezfurlong.wezterm", rule(terminal_pattern, terminal_format)),
        // Browsers
        ("firefox", rule(browser_pattern, browser_format)),
        ("chromium-browser", rule(browser_pattern, browser_format)),
        ("google-chrome", rule(browser_pattern, browser_format)),
        ("brave-browser", rule(browser_pattern, browser_format)),
        // Editors
        ("code", rule(editor_pattern, editor_format)),
        ("Code", rule(editor_pattern, editor_format)),
        ("codium", rule(editor_pattern, editor_format)),
        ("zed", rule(editor_pattern, editor_format)),
    ]
    .into_iter()
    .map(|(id, r)| (id.to_string(), r))
    .collect()
}
