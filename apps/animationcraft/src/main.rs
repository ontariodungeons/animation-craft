//! AnimationCraft desktop app.
//!
//! Usage: `animationcraft [--control <port>] [image.png …]`; `--help` prints the options.
//!
//! `--control <port>` starts a localhost JSON-lines control server for agents and MCP.

#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

mod args;

use animationcraft_ui_egui::AnimationCraftApp;
use serde_json::json;

/// Reverse-DNS app id: Linux `.desktop` file, hicolor icon name and Wayland app id.
const APP_ID: &str = "ai.storyteller.animationcraft.codex";

fn main() -> eframe::Result {
    let args = args::Args::parse(std::env::args());
    if args.help {
        tell(args::USAGE, false);
        return Ok(());
    }
    if args.version {
        tell(&format!("animationcraft {}\n", env!("CARGO_PKG_VERSION")), false);
        return Ok(());
    }

    let mut app = AnimationCraftApp::new();

    if let Some(port) = args.control_port {
        app.start_control_server(port);
    }

    // If an image was passed on the command line, run intake.validate on it.
    if let Some(path) = args.image_path {
        let _ = app.dispatch("intake.validate", json!({ "image": path }));
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("AnimationCraft Studio")
            .with_inner_size([1600.0, 980.0])
            .with_min_inner_size([900.0, 560.0])
            .with_drag_and_drop(true)
            .with_app_id(APP_ID),
        ..Default::default()
    };

    eframe::run_native(
        "AnimationCraft Studio",
        options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
}

/// Print `text` (stdout, or stderr for an error).
fn tell(text: &str, error: bool) {
    if error {
        eprint!("{text}");
    } else {
        print!("{text}");
    }
}

#[cfg(test)]
mod tests {
    use animationcraft_ui_egui::AnimationCraftApp;

    #[test]
    fn app_constructs_without_panic() {
        let _app = AnimationCraftApp::new();
    }
}
