use std::io;

use futures::StreamExt;
use niri_waybar_windowlist::mpris_indicator::{self, AudioState, PlaybackStatus};
use tracing_subscriber::{fmt::time::uptime, EnvFilter};
use waybar_cffi::gtk::glib;

fn status_symbol(status: PlaybackStatus) -> &'static str {
    match status {
        PlaybackStatus::Playing => "▶ Playing",
        PlaybackStatus::Paused => "⏸ Paused",
        PlaybackStatus::Stopped => "⏹ Stopped",
    }
}

fn print_state(state: &AudioState) {
    print!("\x1B[2J\x1B[H");
    println!("MPRIS Player Monitor\n");

    if state.by_desktop_entry.is_empty() && state.by_bus_suffix.is_empty() {
        println!("  (no active players)");
        return;
    }

    if !state.by_desktop_entry.is_empty() {
        println!("By desktop entry:");
        for (entry, status) in &state.by_desktop_entry {
            println!("  {entry}: {}", status_symbol(*status));
        }
    }

    if !state.by_bus_suffix.is_empty() {
        if !state.by_desktop_entry.is_empty() {
            println!();
        }
        println!("By bus suffix:");
        for (suffix, status) in &state.by_bus_suffix {
            println!("  {suffix}: {}", status_symbol(*status));
        }
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("niri_waybar_windowlist=debug")),
        )
        .with_timer(uptime())
        .with_writer(io::stderr)
        .init();

    let ctx = glib::MainContext::default();
    ctx.block_on(async {
        let (_monitor, stream) = mpris_indicator::start();

        let mut stream = Box::pin(stream);
        while let Some(state) = stream.next().await {
            print_state(&state);
        }
    });
}
