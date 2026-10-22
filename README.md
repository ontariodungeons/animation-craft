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

## Command Center

`animationcraft-cli center` opens the native Ratatui Command Center. `center project=/absolute/project.acproj` resumes inspection; `center --print project=...` prints its state. F1/F2 switch Control/Booth monitoring views; F4 shows intake. F3 opens, F5 creates, F6 explicitly resumes intake, F7 populates the editable DeckCraft template (or reopens the existing document), F8 refreshes; Ctrl+S saves and Ctrl+Q exits without losing saved state. Inputs can also be supplied as `root= name= audio= transcript= captions= audiomap=` arguments. Studio never launches automatically.

Scripted clients use the same `project.intake` and `center.status` engine commands. UI and CLI share the production root `/run/media/jesse/Speedy/media`, stored as `projects_root` in `$XDG_CONFIG_HOME/animationcraft/settings.json` (or `~/.config/animationcraft/settings.json`). Inspect it with `animationcraft-cli exec project.root`; persist it with `animationcraft-cli exec project.root root=/run/media/jesse/Speedy/media`. `project.intake name=...` resolves `<root>/<song-slug>/project/<Name>.acproj`. Explicit `root=...` or `path=...` choices apply only to that request, appear in the intake receipt, and never change the production setting. Recent history never determines the root. Existing projects require `resume=true`; changed transcript replacement requires explicit `replace_transcript=true`. Received bytes and errors persist in the project even when validation fails. The transcript importer accepts native HyperFrames word arrays, with original SRT when supplied or an explicitly provisional word-by-word fallback using existing timestamps. Recording duration is probed without transcription or imported from an audiomap. Raw text/IDs/timing are preserved.

Recent activity stores three root action groups per project. Engine results and matched Studio acknowledgements remain separate; closed projects use a disk snapshot. Control shows the existing ten-entry recent-project catalog, not a complete filesystem inventory; selecting a different catalog project is not implemented.

Tauri New/Open invoke the adjacent CLI through the selected Studio bridge. `storyboard.init` populates the bundled native DeckCraft template from the imported transcript and canonical scene spans, registers the resulting deck and preserves an existing active deck on rerun. Studio uses its existing continuous document picture-placement and Save-new-version path; it is not a general native slide text editor. Creative fields remain instructional until authored. Current shell source exposes guarded dirty-state and UI-feedback queries; a running older shell must be rebuilt and safely restarted before those updated contracts apply. Its UI mapping panel labels remaining gaps. Audio-tool provider/model provenance is reported unknown when the received source did not record it.

## Generate an image

`animationcraft-cli exec image.generate project=/absolute/project.acproj prompt='Describe the artwork' model=gpt-6-luna quality=auto` uses a fresh Codex session and the native image tool with the current login. The same `image.generate` command is exposed to MCP and the separate Project image-prompt block; it does not change chat settings. `animationcraft-cli commands image.generate` describes the contract. The executable remains `animationcraft-cli`; no separate `ac-cli` implementation is installed.

Generated PNGs and request receipts live in the selected project's `assets/generated-images/<request>/`. The one-shot subprocess stops after its first successful native image receipt; the adapter copies the receipt-correlated staged bytes rather than trusting an agent-authored file or save claim. A successful native tool receipt and full PNG decode are required before `project.add_asset` attaches it. Failures retain received files and logs without approval claims. The selected Codex model is distinct from the underlying image model; only `quality=auto` is verified, exact resolution is tool-controlled and actual dimensions are returned. Generation serializes engine mutations; the shell remains responsive but other engine requests wait. This block needs a rebuilt shell; terminal success alone does not verify UI rendering.

## Existing material intake

Discover contracts from `animationcraft-cli commands workshop` and `commands project`. `workshop.set_audio` references original audio. `workshop.import_transcript` accepts JSON with optional original SRT via `captions`; preserve every word/timestamp and distinguish raw transcription from corrections. Audio analysis/transcription is an explicit operation, never a prerequisite for reusing valid existing timing. Existing imported or generated files are not proof of approval.

Storyboards and media commands operate in the selected project. `storyboard.design` reports active `deck`/`pdf` paths and whether a real design draft is pending. `storyboard.save_version` saves the design itself; an additional `project.save` is not needed to clear design changes. Saved snapshots reopen clean, including legacy snapshots with a stale dirty flag; actual pending placements or removals stay dirty. Preview-image display is not native editable slide-object support. Keep runtime/tool failures and unavailable integration visible; do not substitute custom production pipelines.

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
