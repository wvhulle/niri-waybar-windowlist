use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{self, Command},
};

fn find_library() -> PathBuf {
    // Check common build output locations
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let target_dir = Path::new(manifest_dir).join("target");

    for profile in ["debug", "release"] {
        let candidate = target_dir.join(profile).join("libniri_waybar_windowlist.so");
        if candidate.exists() {
            return candidate;
        }
    }

    eprintln!("error: libniri_waybar_windowlist.so not found in target/debug or target/release");
    eprintln!("hint: run `cargo build` first");
    process::exit(1);
}

fn main() {
    let lib_path = find_library();
    eprintln!("using library: {}", lib_path.display());

    let tmp_dir = env::temp_dir().join("waybar-windowlist-test");
    fs::create_dir_all(&tmp_dir).expect("failed to create temp directory");

    let config_path = tmp_dir.join("config.jsonc");
    let style_path = tmp_dir.join("style.css");

    let config = format!(
        r#"{{
  "layer": "top",
  "position": "bottom",
  "height": 40,
  "modules-center": ["cffi/niri_window_buttons"],
  "cffi/niri_window_buttons": {{
    "module_path": "{}",
    "show_window_titles": true,
    "icon_size": 24,
    "icon_spacing": 6
  }}
}}"#,
        lib_path.display()
    );

    // Minimal style so the bar is visible
    let style = r"
* {
    font-family: sans-serif;
    font-size: 14px;
}

window#waybar {
    background-color: rgba(30, 30, 46, 0.9);
    color: #cdd6f4;
}
";

    fs::write(&config_path, config).expect("failed to write config");
    fs::write(&style_path, style).expect("failed to write style");

    eprintln!("config: {}", config_path.display());
    eprintln!("style:  {}", style_path.display());
    eprintln!("starting waybar...");

    let status = Command::new("waybar")
        .arg("--config")
        .arg(&config_path)
        .arg("--style")
        .arg(&style_path)
        .env("RUST_LOG", env::var("RUST_LOG").unwrap_or_else(|_| "niri_waybar_windowlist=info".into()))
        .status()
        .expect("failed to start waybar — is it installed?");

    if !status.success() {
        eprintln!("waybar exited with: {status}");
        process::exit(status.code().unwrap_or(1));
    }
}
