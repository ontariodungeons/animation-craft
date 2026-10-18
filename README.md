# AnimationCraft — Modular 2D/3D Turnaround Animation Studio

AnimationCraft is a pure-Rust `*Craft` application for 2D/3D character turnaround, pose-sheet animation, and Blender-free 3D mesh/GLB export. It follows the exact architecture of [filmcraft-codex](https://github.com/storytold/filmcraft): everything is a command, everything is agent-drivable, and nothing panics.

```
2D Image Upload
  ──► intake.validate          (classify image: turnaround_sheet / pose_sheet / single_sprite / scene_concept)
  ──► asset.cut_sheet          (PhotoCraft slice → isolated PNGs + SVG limb masks)
  ──► turnaround.optimize_accuracy (0px drift registration + motion_range.json)
  ──► anim.preview_2d          (2D side-profile walk/run/jump/wave gate)
  ──► mesh.build_4dir          (4-direction 512×512 atlas mesh → .glb)
  ──► export.effectcraft / export.filmcraft / export.blender
```

## Repository Layout

```
/home/jesse/Work/omhas-animation/
├── Cargo.toml                      # Rust workspace root (members: crates/*, apps/*)
├── README.md                       # this file
├── CLAUDE.md                       # behavioral guidelines for agents
├── crates/
│   ├── engine/                     # animationcraft-engine
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs              # Session + EngineError + execute dispatcher
│   │       ├── session.rs          # Session state + Asset struct
│   │       └── commands.rs         # Command registry (9 commands) + unit tests
│   └── ui-egui/                    # animationcraft-ui-egui
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs              # AnimationCraftApp (eframe::App), UiState, UiCommand
│           ├── menus.rs            # Top menu bar (File / Turnaround / Export / Help)
│           ├── panels/
│           │   ├── mod.rs
│           │   ├── left.rs         # Intake, 5-view strip, Pose sheet, Part Outliner
│           │   ├── center.rs       # Dual-tab: 2D Gate | 3D Turnaround Viewport
│           │   └── right.rs        # Anim mode, Speed/Range slider, Jev QA, 1-Click Export
│           ├── commands.rs         # (placeholder)
│           ├── menus.rs            # (see above)
│           └── panels/             # (see above)
└── apps/
    ├── animationcraft/             # Native desktop app binary (`animationcraft`)
    │   ├── Cargo.toml
    │   └── src/
    │       ├── main.rs             # eframe entry point, args parsing
    │       └── args.rs             # CLI args for the desktop app
    └── animationcraft-cli/         # Headless CLI binary (`animationcraft-cli`)
        ├── Cargo.toml
        └── src/
            ├── main.rs             # info / commands / exec / run / mcp subcommands
            └── args.rs             # key=value param parsing
```

## Dependencies (matching filmcraft-codex)

| Crate | Version | Source |
|---|---|---|
| `craft-ui` | 0.1.0 | git `storytold/craft-libs` rev `5c874d5` (features: `serde`) |
| `egui` | 0.36 | crates.io |
| `eframe` | 0.36 | crates.io (features: `default_fonts`, `wgpu`, `accesskit`, `wayland`, `x11`) |
| `image` | 0.25 | crates.io (features: `png`, `jpeg`) |
| `serde` / `serde_json` | 1 | crates.io |
| `thiserror` | 2 | crates.io |
| `log` | 0.4 | crates.io |
| `rayon` | 1 | crates.io |
| `rfd` | 0.17 | crates.io |

## Build & Test

```bash
# Check everything compiles
cargo check --workspace

# Run all unit tests
cargo test --workspace

# Build release binaries
cargo build --release --workspace
```

## Run

```bash
# Desktop app (opens the 3-column Studio UI)
cargo run -p animationcraft

# Desktop app with an image to validate on startup
cargo run -p animationcraft -- exports/vault_slices/view_0.png

# Headless CLI
cargo run -p animationcraft-cli -- info
cargo run -p animationcraft-cli -- commands
cargo run -p animationcraft-cli -- exec intake.validate image=test.png
cargo run -p animationcraft-cli -- run script.jsonl
```

## Engine Commands

All commands are registered in `crates/engine/src/commands.rs` and dispatchable from the egui UI, the CLI, or a future MCP/control channel.

| Command ID | Label | Params | External Tool |
|---|---|---|---|
| `intake.validate` | Validate Image Intake | `{"image": "path"}` | `studio_pipeline_cli.py validate` |
| `asset.cut_sheet` | Cut Sheet (PhotoCraft) | `{"asset": "id"}` | `photocraft-cli` + `studio_pipeline_cli.py cut-sheet` |
| `turnaround.optimize_accuracy` | Optimize Turnaround Accuracy | `{}` | `studio_pipeline_cli.py optimize-accuracy` |
| `anim.preview_2d` | Preview 2D Animation | `{"mode": "walk\|run\|jump\|wave\|sit\|look_up\|look_down"}` | native |
| `mesh.build_4dir` | Build 4-Direction Mesh | `{"format": "glb"}` | native |
| `export.effectcraft` | Export EffectCraft | `{"output": "path.ecproj"}` | `effectcraft-cli` |
| `export.filmcraft` | Export FilmCraft | `{"output": "path.fcproj"}` | `filmcraft-cli` |
| `export.blender` | Export Blender 5.3 | `{"output": "path.glb", "script": "path.py"}` | `studio_pipeline_cli.py blender-check` / Blender 127.0.0.1:9876 |
| `command.list` | List Commands | `{}` | native |

## Architecture Principles

1. **Everything is a command** — every UI button, slider, and CLI subcommand maps to a single `CommandSpec` with an id, label, menu path, params schema, enablement check, and run function.
2. **Never crash** — `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, ...)]`; all functions return `Result<T, EngineError>`.
3. **No raylib** — UI is `craft-ui` + `egui 0.36` + `eframe 0.36`.
4. **Agent-drivable** — the CLI (`animationcraft-cli`) exposes every command via `exec`/`run`; a future MCP server can drive the same `Session::execute`.
5. **External tool integration** — commands call `photocraft-cli`, `effectcraft-cli`, `filmcraft-cli`, and `studio_pipeline_cli.py` as subprocesses; Blender 5.3 is reached at `127.0.0.1:9876` via `studio_pipeline_cli.py blender-check`.

## External Tools Required

| Tool | Path | Purpose |
|---|---|---|
| `photocraft-cli` | `~/.local/bin/photocraft-cli` | image crop, background removal, color decontamination |
| `effectcraft-cli` | `~/.local/bin/effectcraft-cli` | `.ecproj` / `.jsx` export |
| `filmcraft-cli` | `~/.local/bin/filmcraft-cli` | `.fcproj` video timeline import |
| `studio_pipeline_cli.py` | `studio_pipeline_cli.py` | intake validate, cut-sheet, optimize-accuracy, blender-check |
| `blender` | `~/.local/bin/blender` | Blender 5.3 armature + rig (via MCP port 9876) |

## Verification Gates

Before committing, run:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

All 9 tests currently pass:

```
test result: ok. 6 passed; 0 failed   (animationcraft-engine)
test result: ok. 2 passed; 0 failed   (animationcraft-cli)
test result: ok. 1 passed; 0 failed   (animationcraft)
```

## License

MIT OR Apache-2.0
# animation-craft
