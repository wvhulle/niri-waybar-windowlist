use serde::Deserialize;

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
            playing_icon: "\u{25B6}".to_string(),
            muted_icon: "\u{23F8}".to_string(),
            clickable: true,
        }
    }
}
