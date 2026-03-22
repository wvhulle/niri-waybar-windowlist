use std::collections::HashMap;

use regex::Regex;
use serde::{Deserialize, Deserializer};

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

fn rule(pattern: &str, format: &str) -> TitleFormatRule {
    TitleFormatRule {
        pattern: Regex::new(pattern).expect("builtin pattern is valid"),
        format: format.to_string(),
    }
}

pub fn default_rules() -> HashMap<String, TitleFormatRule> {
    let terminal_pattern = r"^(?P<cwd>.+?)(?:\s-\s(?P<cmd>.+))?$";
    let terminal_format = "<i>{{ cwd | shorten_home }}</i>{% if cmd %} · {{ cmd }}{% endif %}";

    // Firefox: "Page · Site — Mozilla Firefox" or "Page — Mozilla Firefox" or
    // "Page" Uses em-dash only to avoid mismatching hyphens in page titles
    let firefox_pattern = r"^(?P<page>.+?)(?:\s·\s(?P<site>.+?))?\s—\s.+$|^(?P<page>.+)$";
    let firefox_format = "{% if site %}<i>{{ site }}</i> · {% endif %}{{ page }}";

    // Chromium: "Page Title - Browser Name"
    let chromium_pattern = r"^(?P<page>.+)\s-\s(?P<site>.+)$|^(?P<page>.+)$";
    let chromium_format =
        "{{ page }}{% if site %} <span alpha='60%'>— {{ site }}</span>{% endif %}";

    // "filename - Editor Name" or "filename · Editor Name"
    let editor_pattern = r"^(?P<file>.+?)(?:\s[-·]\s(?P<editor>.+))?$";
    let editor_format = "<b>{{ file | basename }}</b>{% if editor %} <span alpha='60%'>— {{ \
                         editor }}</span>{% endif %}";

    [
        // Terminals
        ("foot", rule(terminal_pattern, terminal_format)),
        ("Alacritty", rule(terminal_pattern, terminal_format)),
        ("kitty", rule(terminal_pattern, terminal_format)),
        ("wezterm", rule(terminal_pattern, terminal_format)),
        ("ghostty", rule(terminal_pattern, terminal_format)),
        (
            "org.wezfurlong.wezterm",
            rule(terminal_pattern, terminal_format),
        ),
        // Browsers
        ("firefox", rule(firefox_pattern, firefox_format)),
        ("chromium-browser", rule(chromium_pattern, chromium_format)),
        ("google-chrome", rule(chromium_pattern, chromium_format)),
        ("brave-browser", rule(chromium_pattern, chromium_format)),
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
