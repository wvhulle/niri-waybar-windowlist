use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
};

use minijinja::{AutoEscape, Environment};
use regex::Regex;
use serde::{de, Deserialize, Deserializer};

// ── Title format rule ──

#[derive(Debug, Clone, Deserialize)]
pub struct TitleFormatRule {
    #[serde(deserialize_with = "parse_regex")]
    pub pattern: Regex,
    pub format: String,
    /// When `true`, poll `/proc` for the foreground process cwd/command
    /// instead of relying on the compositor window title.
    #[serde(default)]
    pub poll_proc: bool,
}

fn parse_regex<'de, D>(deserializer: D) -> Result<Regex, D::Error>
where
    D: Deserializer<'de>,
{
    let pattern = String::deserialize(deserializer)?;
    Regex::new(&pattern).map_err(de::Error::custom)
}

fn rule(pattern: &str, format: &str) -> TitleFormatRule {
    TitleFormatRule {
        pattern: Regex::new(pattern).expect("builtin pattern is valid"),
        format: format.to_string(),
        poll_proc: false,
    }
}

fn terminal_rule(pattern: &str, format: &str) -> TitleFormatRule {
    TitleFormatRule {
        pattern: Regex::new(pattern).expect("builtin pattern is valid"),
        format: format.to_string(),
        poll_proc: true,
    }
}

pub fn default_rules() -> HashMap<String, TitleFormatRule> {
    let terminal_pattern = r"^(?P<cwd>.+?)(?:(?:\s-\s|>\s?)(?P<cmd>.+))?$";
    let terminal_format = "<i>{{ cwd | shorten_home }}</i>{% if cmd %} · {{ cmd }}{% endif %}";

    // Firefox: "Page · Site — Mozilla Firefox" or "Page — Mozilla Firefox"
    let firefox_pattern = r"^(?P<page>.+?)(?:\s·\s(?P<site>.+?))?\s—\s.+$";
    let firefox_format = "{% if site %}<i>{{ site }}</i> · {% endif %}{{ page }}";

    // Chromium: "Page Title - Browser Name"
    let chromium_pattern = r"^(?P<page>.+?)(?:\s-\s(?P<site>.+))?$";
    let chromium_format =
        "{{ page }}{% if site %} <span alpha='60%'>— {{ site }}</span>{% endif %}";

    // "filename - Editor Name" or "filename · Editor Name"
    let editor_pattern = r"^(?P<file>.+?)(?:\s[-·]\s(?P<editor>.+))?$";
    let editor_format = "<b>{{ file | basename }}</b>{% if editor %} <span alpha='60%'>— {{ \
                         editor }}</span>{% endif %}";

    [
        // Terminals (poll /proc for foreground process info)
        ("foot", terminal_rule(terminal_pattern, terminal_format)),
        (
            "Alacritty",
            terminal_rule(terminal_pattern, terminal_format),
        ),
        // Kitty in single-instance mode shares one PID across all OS windows,
        // so /proc polling yields identical stale data. Rely on the compositor
        // title instead (set via `kitty @ set-window-title`).
        ("kitty", rule(terminal_pattern, terminal_format)),
        ("wezterm", terminal_rule(terminal_pattern, terminal_format)),
        ("ghostty", terminal_rule(terminal_pattern, terminal_format)),
        (
            "org.wezfurlong.wezterm",
            terminal_rule(terminal_pattern, terminal_format),
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

// ── Template rendering ──

fn create_template_env() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_auto_escape_callback(|_| AutoEscape::Html);
    env.add_filter("basename", |path: String| -> String {
        Path::new(&path)
            .file_name()
            .map_or(path.clone(), |n| n.to_string_lossy().into_owned())
    });
    env.add_filter("shorten_home", |path: String| -> String {
        dirs::home_dir()
            .filter(|home| path.starts_with(&*home.to_string_lossy()))
            .map(|home| format!("~{}", &path[home.to_string_lossy().len()..]))
            .unwrap_or(path)
    });
    env
}

thread_local! {
    static TEMPLATE_ENV: Environment<'static> = create_template_env();
}

pub fn render_with_rule(rule: &TitleFormatRule, captures: &BTreeMap<&str, &str>) -> Option<String> {
    TEMPLATE_ENV.with(|env| {
        match env.render_str(
            &rule.format,
            minijinja::context! { ..minijinja::Value::from_serialize(captures) },
        ) {
            Ok(rendered) => Some(rendered),
            Err(e) => {
                tracing::warn!(%e, "template render failed");
                None
            }
        }
    })
}
