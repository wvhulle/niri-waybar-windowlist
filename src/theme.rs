use std::path::PathBuf;

use waybar_cffi::gtk::gdk;

fn niri_config_path() -> PathBuf {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        });
    config_home.join("niri/config.kdl")
}

fn parse_hex_color(hex: &str) -> Option<gdk::RGBA> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f64 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f64 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f64 / 255.0;
    Some(gdk::RGBA::new(r, g, b, 1.0))
}

/// A color that is either solid or a gradient with `from` and `to` endpoints.
/// When rendered as an indicator, `from` is used at the center and `to` at the edges.
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
/// Handles both `active-gradient from="..." to="..."` and `active-color "#..."` forms.
fn parse_indicator_color(border: &kdl::KdlDocument, gradient_key: &str, color_key: &str) -> Option<IndicatorColor> {
    if let Some(node) = border.get(gradient_key) {
        let from = node.get("from").and_then(|e| e.as_string()).and_then(parse_hex_color);
        let to = node.get("to").and_then(|e| e.as_string()).and_then(parse_hex_color);
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
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            panic!("failed to read niri config at {}: {e}", path.display());
        }
    };
    parse_border_colors(&content)
}

fn parse_border_colors(content: &str) -> BorderColors {
    let doc: kdl::KdlDocument = content
        .parse()
        .unwrap_or_else(|e| panic!("failed to parse niri config: {e}"));

    let border = doc
        .get("layout")
        .and_then(|n| n.children())
        .and_then(|d| d.get("border"))
        .and_then(|n| n.children())
        .expect("niri config missing layout.border section");

    let active = parse_indicator_color(border, "active-gradient", "active-color")
        .expect("niri config missing layout.border active color/gradient");

    let urgent = parse_indicator_color(border, "urgent-gradient", "urgent-color")
        .expect("niri config missing layout.border urgent color/gradient");

    BorderColors { active, urgent }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gradient_config() {
        let config = r##"
layout {
    border {
        active-gradient angle=130 from="#3e999f" relative-to="workspace-view" to="#4271ae"
        urgent-gradient angle=130 from="#c82829" relative-to="workspace-view" to="#f5871f"
    }
}
"##;
        let colors = parse_border_colors(config);
        match colors.active {
            IndicatorColor::Gradient { from, to } => {
                assert!((from.red() - 0x3e as f64 / 255.0).abs() < 0.01);
                assert!((to.red() - 0x42 as f64 / 255.0).abs() < 0.01);
            }
            IndicatorColor::Solid(_) => panic!("expected gradient"),
        }
        match colors.urgent {
            IndicatorColor::Gradient { from, to } => {
                assert!((from.red() - 0xc8 as f64 / 255.0).abs() < 0.01);
                assert!((to.red() - 0xf5 as f64 / 255.0).abs() < 0.01);
            }
            IndicatorColor::Solid(_) => panic!("expected gradient"),
        }
    }

    #[test]
    #[should_panic(expected = "niri config missing")]
    fn panics_on_missing_border() {
        parse_border_colors("layout { gaps 8; }");
    }

    #[test]
    fn parse_hex() {
        let color = parse_hex_color("#4271ae").unwrap();
        assert!((color.red() - 0x42 as f64 / 255.0).abs() < 0.001);
        assert!((color.green() - 0x71 as f64 / 255.0).abs() < 0.001);
        assert!((color.blue() - 0xae as f64 / 255.0).abs() < 0.001);
    }

    #[test]
    fn loads_real_config() {
        let colors = load_border_colors();
        eprintln!("active={:?} urgent={:?}", colors.active, colors.urgent);
    }
}
