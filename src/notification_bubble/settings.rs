use serde::Deserialize;

#[derive(Debug, Clone, Copy)]
pub struct BubbleColor {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl BubbleColor {
    pub fn rgba(self) -> (f64, f64, f64, f64) {
        (self.r, self.g, self.b, self.a)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BubbleColors {
    pub low: BubbleColor,
    pub normal: BubbleColor,
    pub critical: BubbleColor,
}

impl Default for BubbleColors {
    fn default() -> Self {
        Self {
            low: BubbleColor { r: 0.6, g: 0.6, b: 0.6, a: 0.7 },
            normal: BubbleColor { r: 0.3, g: 0.6, b: 1.0, a: 1.0 },
            critical: BubbleColor { r: 1.0, g: 0.3, b: 0.2, a: 1.0 },
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NotificationConfig {
    pub(crate) enabled: bool,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}
