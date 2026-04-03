use std::{
    env,
    path::Path,
    process::{self, Command},
};

fn main() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest_path = manifest_dir.join("Cargo.toml");

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

    let config_path = manifest_dir.join("config.json");
    eprintln!("config: {}", config_path.display());
    eprintln!("starting waybar...");

    let status = Command::new("waybar")
        .arg("--config")
        .arg(&config_path)
        .current_dir(manifest_dir)
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
