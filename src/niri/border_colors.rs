use std::{env, fs, path::PathBuf};

use waybar_cffi::gtk::gdk;

fn niri_config_path() -> PathBuf {
    let config_home = env::var_os("XDG_CONFIG_HOME").map_or_else(
        || PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".config"),
        PathBuf::from,
    );
    config_home.join("niri/config.kdl")
}

fn parse_hex_color(hex: &str) -> Option<gdk::RGBA> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = f64::from(u8::from_str_radix(&hex[0..2], 16).ok()?) / 255.0;
    let g = f64::from(u8::from_str_radix(&hex[2..4], 16).ok()?) / 255.0;
    let b = f64::from(u8::from_str_radix(&hex[4..6], 16).ok()?) / 255.0;
    Some(gdk::RGBA::new(r, g, b, 1.0))
}

/// A color that is either solid or a gradient with `from` and `to` endpoints.
/// When rendered as an indicator, `from` is used at the center and `to` at the
/// edges.
#[derive(Debug, Clone, Copy)]
pub enum IndicatorColor {
    Solid(gdk::RGBA),
    Gradient { from: gdk::RGBA, to: gdk::RGBA },
}

#[derive(Debug, Clone, Copy)]
pub struct BorderColors {
    pub active: IndicatorColor,
    pub urgent: IndicatorColor,
}

/// Parse a border/gradient node into an `IndicatorColor`.
/// Handles both `active-gradient from="..." to="..."` and `active-color "#..."`
/// forms.
fn parse_indicator_color(
    border: &kdl::KdlDocument,
    gradient_key: &str,
    color_key: &str,
) -> Option<IndicatorColor> {
    if let Some(node) = border.get(gradient_key) {
        let from = node
            .get("from")
            .and_then(|e| e.as_string())
            .and_then(parse_hex_color);
        let to = node
            .get("to")
            .and_then(|e| e.as_string())
            .and_then(parse_hex_color);
        match (from, to) {
            (Some(f), Some(t)) => return Some(IndicatorColor::Gradient { from: f, to: t }),
            (Some(c), None) | (None, Some(c)) => return Some(IndicatorColor::Solid(c)),
            (None, None) => {}
        }
    }
    if let Some(node) = border.get(color_key) {
        let hex = node.entries().first().and_then(|e| e.value().as_string());
        if let Some(c) = hex.and_then(parse_hex_color) {
            return Some(IndicatorColor::Solid(c));
        }
    }
    None
}

pub fn load_border_colors() -> BorderColors {
    let path = niri_config_path();
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            panic!("failed to read niri config at {}: {e}", path.display());
        }
    };
    parse_border_colors(&content)
}

/// Niri default colors (from focus-ring defaults: #7fc8ff active, #9b0000
/// urgent).
fn default_active() -> IndicatorColor {
    IndicatorColor::Solid(gdk::RGBA::new(
        f64::from(0x7f) / 255.0,
        f64::from(0xc8) / 255.0,
        f64::from(0xff) / 255.0,
        1.0,
    ))
}

fn default_urgent() -> IndicatorColor {
    IndicatorColor::Solid(gdk::RGBA::new(f64::from(0x9b) / 255.0, 0.0, 0.0, 1.0))
}

fn parse_border_colors(content: &str) -> BorderColors {
    let doc: kdl::KdlDocument = content
        .parse()
        .unwrap_or_else(|e| panic!("failed to parse niri config: {e}"));

    let layout = doc.get("layout").and_then(|n| n.children());

    // Try border first, then focus-ring as fallback (niri enables focus-ring by
    // default).
    let section = layout
        .and_then(|d| d.get("border").and_then(|n| n.children()))
        .or_else(|| layout.and_then(|d| d.get("focus-ring").and_then(|n| n.children())));

    let (active, urgent) = match section {
        Some(s) => (
            parse_indicator_color(s, "active-gradient", "active-color").unwrap_or(default_active()),
            parse_indicator_color(s, "urgent-gradient", "urgent-color").unwrap_or(default_urgent()),
        ),
        None => (default_active(), default_urgent()),
    };

    BorderColors { active, urgent }
}
