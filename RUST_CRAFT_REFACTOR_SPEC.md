# Rust `*Craft` Refactoring Spec for `AnimationCraft` (`animationcraft` + `animationcraft-cli`)

> **Historical design reference — not the active implementation plan.**
> The current application is Rust/egui; see [README](README.md) and the current user assignment.
> Statements below about inline/shared-checkout work, untracked baselines, old ports,
> tool versions, completion status, or required new features are historical, not instructions.


## 1. Goal & Non-Negotiables
Refactor `/home/jesse/Work/omhas-animation` into a pure-Rust `*Craft` application matching the exact architecture and shared libraries of `/home/jesse/Work/filmcraft-codex` (`storytold/filmcraft`, `storytold/photocraft`, `storytold/effectcraft`):
- **DO NOT use `raylib`**. Use **`craft-ui`** (`storytold/craft-libs`), **`egui = "0.36"`**, and **`eframe = "0.36"`** (already cached locally in `/home/jesse/.cargo/git/checkouts/craft-libs-d830b991a08c6c3d` and `/home/jesse/Work/filmcraft-codex/Cargo.lock`).
- Follow the `never-crash` rule (`#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]`).
- **"Everything is a command" (`crates/engine`)**: Every UI button, slider, and CLI subcommand dispatches a registered command ID in `animationcraft-engine`. And if a feature exists as a CLI command in `photocraft-cli`, `effectcraft-cli`, `filmcraft-cli`, or `studio_pipeline_cli.py`, `animationcraft-engine` calls it directly!

## 2. Cargo Workspace Layout (`/home/jesse/Work/omhas-animation/Cargo.toml`)
Create a root `Cargo.toml` workspace with:
- `crates/engine` (`animationcraft-engine`)
- `crates/ui-egui` (`animationcraft-ui-egui`)
- `apps/animationcraft` (native Linux desktop app binary `animationcraft`)
- `apps/animationcraft-cli` (headless CLI binary `animationcraft-cli` supporting `info`, `commands`, `exec`, `run`, `mcp`)

Use these exact workspace dependencies (matching `/home/jesse/Work/filmcraft-codex/Cargo.toml` so Cargo uses the local cache):
```toml
[workspace]
resolver = "3"
members = ["crates/*", "apps/*"]
default-members = ["crates/*", "apps/*"]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT OR Apache-2.0"
rust-version = "1.95"

[workspace.dependencies]
animationcraft-engine = { path = "crates/engine" }
animationcraft-ui-egui = { path = "crates/ui-egui" }
craft-ui = { version = "0.1.0", git = "https://github.com/storytold/craft-libs", rev = "5c874d51eeb3de70e735c8ce87a59a7dedd47675", features = ["serde"] }
egui = "0.36"
eframe = { version = "0.36", default-features = false, features = ["default_fonts", "wgpu", "accesskit", "wayland", "x11"] }
rfd = "0.17"
image = { version = "0.25", default-features = false, features = ["png", "jpeg"] }
serde = { version = "1", features = ["derive", "rc"] }
serde_json = "1"
thiserror = "2"
log = "0.4"
rayon = "1"
```

## 3. Engine Commands in `crates/engine` (`animationcraft-engine`)
Implement `Session` + command registry with these commands (callable from both the `egui` UI and `animationcraft-cli`):
1. `intake.validate` — Calls `studio_pipeline_cli.py validate` or native Rust image inspection to classify any arbitrary image (`turnaround_sheet`, `pose_sheet`, `single_sprite`, or `scene_concept`) without hardcoded character assumptions.
2. `asset.cut_sheet` — Calls `/home/jesse/.local/bin/photocraft-cli` (`image.crop`, `layer.removeBackground --params '{"refine":true}'`, `layer.matting.colorDecontaminate`, `edit.fill --params '{"contents":"contentAware"}'`) and `studio_pipeline_cli.py cut-sheet` to break a sheet or scene into clean isolated PNGs + SVG limb masks.
3. `turnaround.optimize_accuracy` — Calls `studio_pipeline_cli.py optimize-accuracy` (`pipeline/codex_accuracy_optimizer.py`) or native Rust registration to align all turnaround views to `0px` vertical drift (`registered_turnaround/`) and compute `motion_range.json` (`Still 00_standing -> Pose Limit` deltas).
4. `anim.preview_2d` — Configures the Stage 1 **2D Side-Profile Walk / Run / Jump / Wave Gate** state (`mode`, `speed`, `pose_range_pct` from `0% Still` to `100% Pose Limit`, active frame index).
5. `mesh.build_4dir` — Builds the 4-direction (`0° Front`, `90° Right Side`, `180° Back`, `270° Left Side`) + per-span elliptical thickness (`sqrt(1 - xi^2)`) 3D mesh and `.glb` output.
6. `export.effectcraft` — Runs `/home/jesse/.local/bin/effectcraft-cli` (`script` + `render`) to create an `.ecproj` composite animation and render H.264 MP4 or transparent RGBA WebM (`--format webm --channels rgba`).
7. `export.filmcraft` — Runs `/home/jesse/.local/bin/filmcraft-cli` / `studio_pipeline_cli.py filmcraft-load` to drop the animated track into a `.fcproj` video timeline.
8. `export.blender` — Runs `studio_pipeline_cli.py blender-check` (or connects directly to Blender 5.3 MCP TCP port `127.0.0.1:9876`) to build the 4-direction rigged `.blend` + 48-frame turntable walk MP4.

## 4. Studio UI in `crates/ui-egui` (`animationcraft-ui-egui`)
Use `craft_ui` (`buttons`, `docking`, `icons`, `layout`, `tabs`, `theme`) + `egui 0.36` to provide the 3-column Image-to-Animation Bridge Studio:
- **Left Panel (Intake, Asset/Sheet Slicer & Layer/Part Outliner):**
  - Open/Drop Any Image button (`rfd` file picker + path input) + Demo Preset selector (`Hopper`, `Whiskers`, `Puddles`, `Sun`, `Coral Fish`).
  - Action buttons dispatching `intake.validate`, `asset.cut_sheet` (`PhotoCraft 0.10`), and `turnaround.optimize_accuracy` (`0px Drift Registration`).
  - Turnaround 5-View thumbnail strip (`0° Front`, `45° 3/4`, `90° Side`, `135° Back-3/4`, `180° Back`) + Pose Sheet 11-frame list + Layer/Part Outliner with visibility/solo checkboxes.
- **Center Viewport (Dual-Tab Stage):**
  - **Tab 1: 2D Side-Profile Action Gate (Primary Confirmation View):** Displays the animated 2D side-profile / pose-cycle sprite with ground baseline grid, foot-lift indicator, and real-time playback (`Walk`, `Run`, `Jump`, `Wave`, `Idle`).
  - **Tab 2: 3D 4-Direction Turnaround Preview:** Displays the 4-direction turnaround projection (`0°`, `45°`, `90°`, `135°`, `180°`, `270°`) / Blender 5.3 rendered preview with yaw slider and quick angle snap buttons (`0° Front`, `45° 3/4`, `90° Side`, `180° Back`, `Auto-Turntable`).
- **Right Panel (Motion Range Controls, Jev QA & 1-Click Export):**
  - Animation Mode selector (`Idle`, `Walk`, `Run`, `Jump`, `Wave`, `Static`).
  - `Animation Speed (0.25x..2.25x)` and `Pose Motion Range: Still (0% 00_standing) ↔ Limit (100% Pose Limit)` slider with live pixel delta telemetry (`Leg Swing ±px`, `Arm Raise ±px`, `Foot Lift +px` from `motion_range.json`).
  - 1-Click Export buttons dispatching `export.effectcraft`, `export.blender`, and `export.filmcraft`, showing live command output & generated file paths.

## 5. Verification
Run `cargo check --workspace` and `cargo test --workspace`, and test `cargo run -p animationcraft-cli -- commands` and `cargo run -p animationcraft-cli -- info`.
