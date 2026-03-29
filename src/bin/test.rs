use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{self, Command},
};

fn build_library() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let manifest_path = Path::new(manifest_dir).join("Cargo.toml");
    let target_dir = Path::new(manifest_dir).join("target");

    eprintln!("building library...");
    let status = Command::new("cargo")
        .arg("build")
        .arg("--lib")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .status()
        .expect("failed to run cargo build — is cargo installed?");

    if !status.success() {
        eprintln!("cargo build failed with: {status}");
        process::exit(status.code().unwrap_or(1));
    }

    let lib_path = target_dir
        .join("debug")
        .join("libniri_waybar_windowlist.so");
    if !lib_path.exists() {
        eprintln!("error: {} not found after build", lib_path.display());
        process::exit(1);
    }

    lib_path
}

fn main() {
    let lib_path = build_library();
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
        .env(
            "RUST_LOG",
            env::var("RUST_LOG").unwrap_or_else(|_| "niri_waybar_windowlist=trace".into()),
        )
        .status()
        .expect("failed to start waybar — is it installed?");

    if !status.success() {
        eprintln!("waybar exited with: {status}");
        process::exit(status.code().unwrap_or(1));
    }
}
