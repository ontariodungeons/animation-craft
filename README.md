# AnimationCraft Studio

The application is the Tauri shell in `apps/animationcraft-shell`. Its embedded UI, CLI and MCP share the Rust engine command registry and domain types.

## Run and discover

```bash
cargo build -p animationcraft-shell --bin animationcraft-cli
./target/debug/animationcraft-shell
animationcraft-cli --help
animationcraft-cli commands
```

The Omarchy menu entry is **AnimationCraft Studio**. Rebuild the shell after changing `apps/animationcraft-shell/ui/`; those files are embedded. Keep generated song projects and media on the user-selected Speedy location, outside this source repository.

## CLI and MCP

`animationcraft-cli exec <id> key=value` executes one command. `animationcraft-cli run <file.jsonl|->` keeps one headless session across JSONL commands with `id` and `params`. Separate `exec` calls do not share the open project.

Register one AnimationCraft MCP server per client: command `animationcraft-cli`, args `["mcp"]`. It uses a headless engine session. To use an already configured live Studio bridge, explicitly select its port:

```bash
animationcraft-cli --bridge <port> exec ui.state
animationcraft-cli --bridge <port> mcp
```

A Studio window must have been started with `animationcraft-shell --control <port>` for that bridge to exist. Do not automatically launch/switch another window or restart services. `ui.state` does not report unsaved-change safety.

## Existing material intake

Discover contracts from `animationcraft-cli commands workshop` and `commands project`. `workshop.set_audio` references original audio. `workshop.import_transcript` accepts JSON with optional original SRT via `captions`; preserve every word/timestamp and distinguish raw transcription from corrections. Audio analysis/transcription is an explicit operation, never a prerequisite for reusing valid existing timing. Existing imported or generated files are not proof of approval.

Storyboards and media commands operate in the selected project. Preview-image display is not native editable slide-object support. Keep runtime/tool failures and unavailable integration visible; do not substitute custom production pipelines.

## Source map

| Area | Source |
|---|---|
| Studio UI and transport | `apps/animationcraft-shell/ui/`, `apps/animationcraft-shell/src/` |
| Terminal and MCP | `apps/animationcraft-cli/src/` |
| Command contracts/dispatch | `crates/engine/src/commands.rs`, `lib.rs` |
| Project persistence | `crates/engine/src/project.rs` |
| Workshop/transcript | `crates/engine/src/workshop.rs` |
| Storyboard | `crates/engine/src/storyboard.rs` |
| Tool resolution | `crates/engine/src/workspace.rs`, `pipeline/craft_cli.py` |
| Current Studio interaction contract | `apps/animationcraft-shell/agent.md` |

Read `AGENTS.md` before changes. Run `cargo check --workspace`; normal commit hooks run workspace checks/tests. Commit explicit owned paths; no generated media, credentials or local agent configuration. Source history is recovered through Git, not duplicated application trees in this checkout.
