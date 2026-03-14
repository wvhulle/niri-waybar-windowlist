mod click_actions;
mod process_info;

pub use click_actions::*;
pub use process_info::*;

use std::collections::HashMap;

use regex::Regex;
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Deserialize)]
pub struct OutputDimensions {
    #[serde(default)]
    pub min_button_width: Option<i32>,
    #[serde(default)]
    pub max_button_width: Option<i32>,
    #[serde(default)]
    pub max_taskbar_width: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Settings {
    apps: HashMap<String, Vec<AppRule>>,
    notifications: NotificationConfig,
    show_all_outputs: bool,
    only_current_workspace: bool,
    show_window_titles: bool,
    min_button_width: i32,
    max_button_width: Option<i32>,
    icon_size: i32,
    icon_spacing: i32,
    max_taskbar_width: i32,
    max_taskbar_width_per_output: HashMap<String, i32>,
    dimensions_per_output: HashMap<String, OutputDimensions>,
    scroll_arrow_left: String,
    scroll_arrow_right: String,
    click_actions: ClickActions,
    ignore_rules: Vec<IgnoreRule>,
    context_menu: Vec<ContextMenuItem>,
    multi_select_modifier: ModifierKey,
    multi_select_menu: Vec<MultiSelectMenuItem>,
    drag_hover_focus: bool,
    drag_hover_focus_delay: u32,
    truncate_titles: bool,
    allow_title_linebreaks: bool,
    show_tooltip: bool,
    tooltip_delay: u32,
    button_alignment: ButtonAlignment,
    left_click_focus_on_press: bool,
    audio_indicator: AudioIndicatorConfig,
    process_info: ProcessInfoConfig,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            apps: HashMap::new(),
            notifications: NotificationConfig::default(),
            show_all_outputs: false,
            only_current_workspace: true,
            show_window_titles: true,
            min_button_width: 150,
            max_button_width: None,
            icon_size: 24,
            icon_spacing: 6,
            max_taskbar_width: 1200,
            max_taskbar_width_per_output: HashMap::new(),
            dimensions_per_output: HashMap::new(),
            scroll_arrow_left: "◀".to_string(),
            scroll_arrow_right: "▶".to_string(),
            click_actions: ClickActions::default(),
            ignore_rules: Vec::new(),
            context_menu: default_context_menu(),
            multi_select_modifier: ModifierKey::Ctrl,
            multi_select_menu: default_multi_select_menu(),
            drag_hover_focus: true,
            drag_hover_focus_delay: 500,
            truncate_titles: true,
            allow_title_linebreaks: false,
            show_tooltip: true,
            tooltip_delay: 300,
            button_alignment: ButtonAlignment::default(),
            left_click_focus_on_press: false,
            audio_indicator: AudioIndicatorConfig::default(),
            process_info: ProcessInfoConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ModifierKey {
    #[default]
    Ctrl,
    Shift,
    Alt,
    Super,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AudioIndicatorConfig {
    pub enabled: bool,
    pub playing_icon: String,
    pub muted_icon: String,
    pub clickable: bool,
}

impl Default for AudioIndicatorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            playing_icon: "󰕾".to_string(),
            muted_icon: "󰖁".to_string(),
            clickable: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ButtonAlignment {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NotificationConfig {
    enabled: bool,
    map_app_ids: HashMap<String, String>,
    use_desktop_entry: bool,
    use_fuzzy_matching: bool,
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

#[derive(Debug, Clone, Deserialize)]
pub struct AppRule {
    #[serde(rename = "match", deserialize_with = "parse_regex")]
    pattern: Regex,
    #[serde(default)]
    class: Option<String>,
    #[serde(default)]
    click_actions: Option<ClickActions>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct IgnoreRule {
    pub app_id: Option<String>,
    pub title: Option<String>,
    #[serde(deserialize_with = "parse_optional_regex")]
    pub title_regex: Option<Regex>,
    pub title_contains: Option<String>,
    pub workspace: Option<u64>,
}

impl Default for IgnoreRule {
    fn default() -> Self {
        Self {
            app_id: None,
            title: None,
            title_regex: None,
            title_contains: None,
            workspace: None,
        }
    }
}

fn parse_regex<'de, D>(deserializer: D) -> Result<Regex, D::Error>
where
    D: Deserializer<'de>,
{
    let pattern = String::deserialize(deserializer)?;
    Regex::new(&pattern).map_err(serde::de::Error::custom)
}

fn parse_optional_regex<'de, D>(deserializer: D) -> Result<Option<Regex>, D::Error>
where
    D: Deserializer<'de>,
{
    let pattern: Option<String> = Option::deserialize(deserializer)?;
    pattern
        .map(|p| Regex::new(&p).map_err(serde::de::Error::custom))
        .transpose()
}

impl Settings {
    pub fn get_click_actions(&self, app_id: Option<&str>, title: Option<&str>) -> ClickActions {
        if let (Some(id), Some(t)) = (app_id, title) {
            if let Some(rules) = self.apps.get(id) {
                for rule in rules {
                    if rule.pattern.is_match(t) {
                        if let Some(ref actions) = rule.click_actions {
                            return actions.clone();
                        }
                    }
                }
            }
        }
        self.click_actions.clone()
    }

    pub fn should_ignore(
        &self,
        app_id: Option<&str>,
        title: Option<&str>,
        workspace_id: Option<u64>,
    ) -> bool {
        for rule in &self.ignore_rules {
            let app_match = rule
                .app_id
                .as_ref()
                .map_or(true, |id| app_id == Some(id.as_str()));
            let title_match = rule
                .title
                .as_ref()
                .map_or(true, |t| title == Some(t.as_str()));
            let title_contains_match = rule.title_contains.as_ref().map_or(true, |contains| {
                title.map_or(false, |t| t.contains(contains))
            });
            let title_regex_match = rule
                .title_regex
                .as_ref()
                .map_or(true, |regex| title.map_or(false, |t| regex.is_match(t)));
            let workspace_match = rule.workspace.map_or(true, |ws| workspace_id == Some(ws));

            if app_match
                && title_match
                && title_contains_match
                && title_regex_match
                && workspace_match
            {
                return true;
            }
        }
        false
    }

    pub fn notifications_enabled(&self) -> bool {
        self.notifications.enabled
    }

    pub fn notifications_app_map(&self, app_id: &str) -> Option<&str> {
        self.notifications
            .map_app_ids
            .get(app_id)
            .map(String::as_str)
    }

    pub fn notifications_use_desktop_entry(&self) -> bool {
        self.notifications.use_desktop_entry
    }

    pub fn notifications_use_fuzzy_matching(&self) -> bool {
        self.notifications.use_fuzzy_matching
    }

    pub fn show_all_outputs(&self) -> bool {
        self.show_all_outputs
    }

    pub fn only_current_workspace(&self) -> bool {
        self.only_current_workspace
    }

    pub fn show_window_titles(&self) -> bool {
        self.show_window_titles
    }

    pub fn min_button_width(&self, output: Option<&str>) -> i32 {
        output
            .and_then(|name| self.dimensions_per_output.get(name))
            .and_then(|dims| dims.min_button_width)
            .unwrap_or(self.min_button_width)
    }

    pub fn max_button_width(&self, output: Option<&str>) -> Option<i32> {
        output
            .and_then(|name| self.dimensions_per_output.get(name))
            .and_then(|dims| dims.max_button_width)
            .or(self.max_button_width)
    }

    pub fn icon_size(&self) -> i32 {
        self.icon_size
    }

    pub fn icon_spacing(&self) -> i32 {
        self.icon_spacing
    }

    pub fn max_taskbar_width_for_output(&self, output: Option<&str>) -> i32 {
        output
            .and_then(|name| {
                self.dimensions_per_output
                    .get(name)
                    .and_then(|dims| dims.max_taskbar_width)
                    .or_else(|| self.max_taskbar_width_per_output.get(name).copied())
            })
            .unwrap_or(self.max_taskbar_width)
    }

    pub fn scroll_arrow_left(&self) -> &str {
        &self.scroll_arrow_left
    }

    pub fn scroll_arrow_right(&self) -> &str {
        &self.scroll_arrow_right
    }

    pub fn context_menu(&self) -> &[ContextMenuItem] {
        &self.context_menu
    }

    pub fn multi_select_modifier(&self) -> ModifierKey {
        self.multi_select_modifier
    }

    pub fn multi_select_menu(&self) -> &[MultiSelectMenuItem] {
        &self.multi_select_menu
    }

    pub fn drag_hover_focus(&self) -> bool {
        self.drag_hover_focus
    }

    pub fn drag_hover_focus_delay(&self) -> u32 {
        self.drag_hover_focus_delay
    }

    pub fn truncate_titles(&self) -> bool {
        self.truncate_titles
    }

    pub fn allow_title_linebreaks(&self) -> bool {
        self.allow_title_linebreaks
    }

    pub fn show_tooltip(&self) -> bool {
        self.show_tooltip
    }

    pub fn tooltip_delay(&self) -> u32 {
        self.tooltip_delay
    }

    pub fn button_alignment(&self) -> ButtonAlignment {
        self.button_alignment
    }

    pub fn left_click_focus_on_press(&self) -> bool {
        self.left_click_focus_on_press
    }

    pub fn audio_indicator(&self) -> &AudioIndicatorConfig {
        &self.audio_indicator
    }

    pub fn process_info(&self) -> &ProcessInfoConfig {
        &self.process_info
    }

    pub fn should_show_process_info(&self, app_id: Option<&str>) -> bool {
        self.process_info.enabled
            && app_id.map_or(false, |id| self.process_info.title_patterns.contains_key(id))
    }

    pub fn process_info_pattern(&self, app_id: &str) -> Option<&Regex> {
        self.process_info.title_patterns.get(app_id)
    }
}
