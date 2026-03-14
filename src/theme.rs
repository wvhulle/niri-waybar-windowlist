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

fn gradient_color(node: &kdl::KdlNode) -> Option<&str> {
    node.get("to")
        .or_else(|| node.get("from"))
        .and_then(|e| e.as_string())
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

#[derive(Debug, Clone, Copy)]
pub struct BorderColors {
    pub active: gdk::RGBA,
    pub urgent: gdk::RGBA,
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
    let doc: kdl::KdlDocument = content.parse()
        .unwrap_or_else(|e| panic!("failed to parse niri config: {e}"));

    let border = doc.get("layout")
        .and_then(|n| n.children())
        .and_then(|d| d.get("border"))
        .and_then(|n| n.children())
        .expect("niri config missing layout.border section");

    let active = border.get("active-gradient")
        .and_then(gradient_color)
        .and_then(parse_hex_color)
        .expect("niri config missing layout.border.active-gradient color");

    let urgent = border.get("urgent-gradient")
        .and_then(gradient_color)
        .and_then(parse_hex_color)
        .expect("niri config missing layout.border.urgent-gradient color");

    BorderColors { active, urgent }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_niri_config() {
        let config = r##"
layout {
    border {
        active-gradient angle=130 from="#3e999f" relative-to="workspace-view" to="#4271ae"
        urgent-gradient angle=130 from="#c82829" relative-to="workspace-view" to="#f5871f"
    }
}
"##;
        let colors = parse_border_colors(config);
        assert!((colors.active.red() - 0x42 as f64 / 255.0).abs() < 0.01);
        assert!((colors.urgent.red() - 0xf5 as f64 / 255.0).abs() < 0.01);
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
        eprintln!("active=({},{},{}) urgent=({},{},{})",
            colors.active.red(), colors.active.green(), colors.active.blue(),
            colors.urgent.red(), colors.urgent.green(), colors.urgent.blue());
    }
}
