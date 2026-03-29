use std::collections::HashMap;

use regex::Regex;
use serde::{de, Deserialize, Deserializer};

use crate::{
    mpris_indicator::settings::AudioIndicatorConfig,
    niri::settings::DisplayConfig,
    notification_bubble::settings::NotificationConfig,
    right_click_menu::settings::{
        default_context_menu, default_multi_select_menu, ContextMenuItem, MultiSelectMenuItem,
    },
    window_button::settings::{ClickActions, ModifierKey, TooltipConfig},
    window_list::settings::DragConfig,
    window_title::{
        parse::TitleFormatRule,
        settings::{TitleDisplayConfig, TitleFormatConfig},
    },
};

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Settings {
    apps: HashMap<String, Vec<AppRule>>,
    notifications: NotificationConfig,
    icon_size: i32,
    icon_spacing: i32,
    click_actions: ClickActions,
    ignore_rules: Vec<IgnoreRule>,
    context_menu: Vec<ContextMenuItem>,
    multi_select_modifier: ModifierKey,
    multi_select_menu: Vec<MultiSelectMenuItem>,
    audio_indicator: AudioIndicatorConfig,
    title_format: TitleFormatConfig,
    #[serde(flatten)]
    display: DisplayConfig,
    #[serde(flatten)]
    title_display: TitleDisplayConfig,
    #[serde(flatten)]
    tooltip: TooltipConfig,
    #[serde(flatten)]
    drag: DragConfig,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            apps: HashMap::new(),
            notifications: NotificationConfig::default(),
            icon_size: 24,
            icon_spacing: 6,
            click_actions: ClickActions::default(),
            ignore_rules: Vec::new(),
            context_menu: default_context_menu(),
            multi_select_modifier: ModifierKey::Ctrl,
            multi_select_menu: default_multi_select_menu(),
            audio_indicator: AudioIndicatorConfig::default(),
            title_format: TitleFormatConfig::default(),
            display: DisplayConfig::default(),
            title_display: TitleDisplayConfig::default(),
            tooltip: TooltipConfig::default(),
            drag: DragConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppRule {
    #[serde(rename = "match", deserialize_with = "parse_regex")]
    pattern: Regex,
    #[serde(default)]
    click_actions: Option<ClickActions>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct IgnoreRule {
    pub app_id: Option<String>,
    pub title: Option<String>,
    #[serde(deserialize_with = "parse_optional_regex")]
    pub title_regex: Option<Regex>,
    pub title_contains: Option<String>,
    pub workspace: Option<u64>,
}

fn parse_regex<'de, D>(deserializer: D) -> Result<Regex, D::Error>
where
    D: Deserializer<'de>,
{
    let pattern = String::deserialize(deserializer)?;
    Regex::new(&pattern).map_err(de::Error::custom)
}

fn parse_optional_regex<'de, D>(deserializer: D) -> Result<Option<Regex>, D::Error>
where
    D: Deserializer<'de>,
{
    let pattern: Option<String> = Option::deserialize(deserializer)?;
    pattern
        .map(|p| Regex::new(&p).map_err(de::Error::custom))
        .transpose()
}

impl Settings {
    pub fn get_click_actions(&self, app_id: Option<&str>, title: Option<&str>) -> ClickActions {
        app_id
            .zip(title)
            .and_then(|(id, t)| {
                self.apps.get(id)?.iter().find_map(|rule| {
                    rule.pattern
                        .is_match(t)
                        .then_some(rule.click_actions.as_ref())
                        .flatten()
                        .cloned()
                })
            })
            .unwrap_or_else(|| self.click_actions.clone())
    }

    pub fn should_ignore(
        &self,
        app_id: Option<&str>,
        title: Option<&str>,
        workspace_id: Option<u64>,
    ) -> bool {
        self.ignore_rules.iter().any(|rule| {
            rule.app_id
                .as_ref()
                .is_none_or(|id| app_id == Some(id.as_str()))
                && rule
                    .title
                    .as_ref()
                    .is_none_or(|t| title == Some(t.as_str()))
                && rule
                    .title_contains
                    .as_ref()
                    .is_none_or(|contains| title.is_some_and(|t| t.contains(contains)))
                && rule
                    .title_regex
                    .as_ref()
                    .is_none_or(|regex| title.is_some_and(|t| regex.is_match(t)))
                && rule.workspace.is_none_or(|ws| workspace_id == Some(ws))
        })
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
        self.display.show_all_outputs
    }

    pub fn only_current_workspace(&self) -> bool {
        self.display.only_current_workspace
    }

    pub fn show_window_titles(&self) -> bool {
        self.title_display.show_window_titles
    }

    pub fn icon_size(&self) -> i32 {
        self.icon_size
    }

    pub fn icon_spacing(&self) -> i32 {
        self.icon_spacing
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
        self.drag.drag_hover_focus
    }

    pub fn drag_hover_focus_delay(&self) -> u32 {
        self.drag.drag_hover_focus_delay
    }

    pub fn truncate_titles(&self) -> bool {
        self.title_display.truncate_titles
    }

    pub fn allow_title_linebreaks(&self) -> bool {
        self.title_display.allow_title_linebreaks
    }

    pub fn show_tooltip(&self) -> bool {
        self.tooltip.show_tooltip
    }

    pub fn tooltip_delay(&self) -> u32 {
        self.tooltip.tooltip_delay
    }

    pub fn audio_indicator(&self) -> &AudioIndicatorConfig {
        &self.audio_indicator
    }

    pub fn title_format_rule(&self, app_id: &str) -> Option<&TitleFormatRule> {
        if self.title_format.enabled {
            self.title_format.rules.get(app_id)
        } else {
            None
        }
    }

    pub fn should_poll_proc(&self, app_id: Option<&str>) -> bool {
        self.title_format.enabled
            && app_id.is_some_and(|id| {
                self.title_format
                    .rules
                    .get(id)
                    .is_some_and(|rule| rule.poll_proc)
            })
    }

    pub fn proc_poll_interval(&self) -> Option<u64> {
        if self.title_format.enabled && self.title_format.rules.values().any(|rule| rule.poll_proc)
        {
            Some(self.title_format.poll_interval_ms)
        } else {
            None
        }
    }
}
