# Magic Studio

The application is the Tauri shell in `apps/animationcraft-shell`. Its embedded UI, CLI and MCP share the Rust engine command registry and domain types.

## Run and discover

```bash
cargo build -p animationcraft-shell -p mc
./target/debug/animationcraft-shell
mc --help
mc commands
```

The Omarchy menu entry is **AnimationCraft Studio**. Rebuild the shell after changing `apps/animationcraft-shell/ui/`; those files are embedded. Keep generated song projects and media on the user-selected Speedy location, outside this source repository.

## CLI and MCP

`mc exec <id> key=value` executes one command. `mc run <file.jsonl|->` keeps one headless session across JSONL commands with `id` and `params`. Separate `exec` calls do not share the open project.

Register one AnimationCraft MCP server per client: command `mc`, args `["mcp"]`. It uses a headless engine session. To use an already configured live Studio bridge, explicitly select its port:

```bash
mc --bridge <port> exec ui.state
mc --bridge <port> mcp
```

A Studio window must have been started with `animationcraft-shell --control <port>` for that bridge to exist. Before restarting Studio, inspect pending/dirty state and preserve unsaved work. `ui.state` does not report unsaved-change safety.

## Command Center

`mc center` opens the native Ratatui Command Center. `center project=/absolute/project.acproj` resumes inspection; `center --print project=...` prints its state. F1/F2 switch Control/Booth monitoring views; F4 shows intake. F3 opens, F5 creates, F6 explicitly resumes intake, F7 populates the editable DeckCraft template (or reopens the existing document), F8 refreshes; Ctrl+S saves and Ctrl+Q exits without losing saved state. Inputs can also be supplied as `root= name= audio= transcript= captions= audiomap=` arguments. Studio never launches automatically.

Scripted clients use the same `project.intake` and `center.status` engine commands. UI and CLI share the production root `/run/media/jesse/Speedy/media`, stored as `projects_root` in `$XDG_CONFIG_HOME/animationcraft/settings.json` (or `~/.config/animationcraft/settings.json`). Inspect it with `mc exec project.root`; persist it with `mc exec project.root root=/run/media/jesse/Speedy/media`. `project.intake name=...` resolves `<root>/<song-slug>/project/<Name>.acproj`. Explicit `root=...` or `path=...` choices apply only to that request, appear in the intake receipt, and never change the production setting. Recent history never determines the root. Existing projects require `resume=true`; changed transcript replacement requires explicit `replace_transcript=true`. Received bytes and errors persist in the project even when validation fails. The transcript importer accepts native HyperFrames word arrays, with original SRT when supplied or an explicitly provisional word-by-word fallback using existing timestamps. Recording duration is probed without transcription or imported from an audiomap. Raw text/IDs/timing are preserved.

`center.status trace=true` emits a timestamped shared prerequisite/metric observation and records started/result/completed evidence in `center.events`; ordinary polling returns observation IDs without filling the last-three action history. Studio and Booth consume the same prerequisite states. Missing data remains unknown/missing, and Studio rendering acknowledgement is separate from engine readiness.

Recent activity stores three root action groups per project. Engine results and matched Studio acknowledgements remain separate; closed projects use a disk snapshot. Control shows the existing ten-entry recent-project catalog, not a complete filesystem inventory; selecting a different catalog project is not implemented.

Tauri New/Open invoke the adjacent CLI through the selected Studio bridge. `storyboard.init` populates the bundled native DeckCraft template from the imported transcript and canonical scene spans, registers the resulting deck and preserves an existing active deck on rerun. Studio uses its existing continuous document picture-placement and Save-new-version path. Existing native storyboard text fields are editable in place and save when the field changes; `mc --bridge <port> storyboard edit scene 14 "text" field=action` is a short spelling of the same `storyboard.edit` operation. `storyboard.read scene=14` returns the actual native `text_fields` names and values; text edits preserve the existing document and do not regenerate its template. Creative fields remain instructional until authored. Current shell source exposes guarded dirty-state and UI-feedback queries; a running older shell must be rebuilt and safely restarted before those updated contracts apply. Its UI mapping panel labels remaining gaps. Audio-tool provider/model provenance is reported unknown when the received source did not record it.

## Generate an image

`mc exec image.generate project=/absolute/project.acproj prompt='Describe the artwork' model=gpt-6-luna quality=auto` uses a fresh Codex session and the native image tool with the current login. The same `image.generate` command is exposed to MCP and the separate Project image-prompt block; it does not change chat settings. `mc commands image.generate` describes the contract. `mc` is the canonical CLI executable and package.

Generated PNGs and request receipts live in the selected project's `assets/generated-images/<request>/`. The one-shot subprocess stops after its first successful native image receipt; the adapter copies the receipt-correlated staged bytes rather than trusting an agent-authored file or save claim. A successful native tool receipt and full PNG decode are required before `project.add_asset` attaches it. Failures retain received files and logs without approval claims. The selected Codex model is distinct from the underlying image model; only `quality=auto` is verified, exact resolution is tool-controlled and actual dimensions are returned. Generation serializes engine mutations; the shell remains responsive but other engine requests wait. This block needs a rebuilt shell; terminal success alone does not verify UI rendering.

Reference-driven action sheets use the same native command: `image.generate reference_asset=<registered PNG id> poses='["three-quarter","front"]' character=<rig id> scene=2 animation=turn-around prompt="Preserve this character"`. The receipt retains the source artwork, pose order and storyboard context. `video.place_track generation_receipt=<generation.json>` carries that provenance into a rendered clip placement. See [reference → animation → video](docs/workflows/reference-to-animation-video.md) for the full workflow and its current manual steps.

## Existing material intake

Discover contracts from `mc commands workshop` and `commands project`. `workshop.set_audio` references original audio. `workshop.import_transcript` accepts JSON with optional original SRT via `captions`; preserve every word/timestamp and distinguish raw transcription from corrections. Shared `project.intake` reuses valid supplied/imported timing without ASR, otherwise runs the existing `audio.analyze` pipeline for linked audio, imports its native output and initializes a native DeckCraft document. Invalid supplied timing stops for correction instead of silently retranscribing. Pipeline/document failures keep the project openable with `intake_ready=false`, failed steps and `next_commands`; linking audio alone is not readiness. Direct audio analysis/transcription commands remain available for explicit retries. Existing imported or generated files are not proof of approval.

Storyboards and media commands operate in the selected project. `video.init target=hyperframes hyperframes_cli=/absolute/path/to/hyperframes` invokes the existing official initializer with transcription and skill installation disabled. It passes linked audio to official `init --audio --skip-transcribe` and imports existing timed JSON through official `transcribe <transcript.json> --dir <project> --json --no-runtime-install` (file import, no ASR). Existing projects are reused; explicitly pass `replace_scaffold=true` to rebuild a starter scaffold with backups of replaced scaffold files. Artist sources, existing captions and artwork are preserved. This imports song/timing, not completed authored visuals or a rendered video. After deriving the music-to-video machine plan from the native storyboard and building its frame files, `video.init target=hyperframes assemble=true audiomap=/path/to/existing/audiomap.json` invokes the installed owning skill’s `validate-plan.mjs` and `assemble-index.mjs`. It reuses that map’s already processed BGM without ASR/transcoding and preserves the prior index. `STORYBOARD.md` is generated machine input; the native DeckCraft document remains the editable storyboard. `video.server_start` accepts the same executable parameter; otherwise both resolve `HYPERFRAMES_CLI` or `hyperframes` on PATH, without an unversioned npx install. `storyboard.design` reports active `deck`/`pdf` paths and whether a real design draft is pending. `storyboard.save_version` saves the design itself; an additional `project.save` is not needed to clear design changes. Saved snapshots reopen clean, including legacy snapshots with a stale dirty flag; actual pending placements or removals stay dirty. Preview-image display is not native editable slide-object support. Keep runtime/tool failures and unavailable integration visible; do not substitute custom production pipelines.

`storyboard.import_openmontage path=/existing/project` imports labelled scene content; the obsolete static-document copy branch is removed. `workshop.build_storyboard template=backlot draft=true` populates the versioned `pipeline/templates/studio-template.deckcraft` layout internally and returns an actual editable PDF with native PDFCraft fields. Studio keeps its PDF view, image drop and Save controls. The source words/timestamps are read directly; creative note mapping reports unknown sections rather than discarding or rewriting them. Review the returned draft, then `storyboard.save_version draft=<manifest.json>` adopts it. `storyboard.edit` and on-page text editing fill/save those same PDF fields. An explicit `transcript=<native JSON>` override requires `draft=true`; `allow_timing_overrun=true` permits an unvalidated draft with a visible warning, preserving every original timestamp. It does not validate timing, trim words or rerun ASR. Unchanged draft resume preserves PDF edits.

## Source map

| Area | Source |
|---|---|
| Studio UI and transport | `apps/animationcraft-shell/ui/`, `apps/animationcraft-shell/src/` |
| Terminal and MCP | `apps/mc/src/` |
| Command contracts/dispatch | `crates/engine/src/commands.rs`, `lib.rs` |
| Project persistence | `crates/engine/src/project.rs` |
| Workshop/transcript | `crates/engine/src/workshop.rs` |
| Storyboard | `crates/engine/src/storyboard.rs` |
| Tool resolution | `crates/engine/src/workspace.rs`, `pipeline/craft_cli.py` |
| Current Studio interaction contract | `apps/animationcraft-shell/agent.md` |

Read `AGENTS.md` before changes. Run `cargo check --workspace`; normal commit hooks run workspace checks/tests. Commit explicit owned paths; no generated media, credentials or local agent configuration. Source history is recovered through Git, not duplicated application trees in this checkout.

Shared audio intake retains the official HyperFrames audio project and deterministically binds native timed words into `compositions/captions.html` using the versioned official caption-highlight template. `video.init captions=true` is idempotent: unchanged input preserves caption-source edits and `caption-overrides.json`; changed timing conflicts rather than overwriting an edited source. Initial five-word groups are provisional, not semantic/listening approval. `project.intake hyperframes_cli=/absolute/path` uses the same operation; its audio extraction skips optional beat analysis (`audio.analyze analyze_beats=false`) because captions do not require librosa.

## Project agent coordination

Studio and Booth use the existing `agent.send`, `agent.log` and `agent.session` commands. With the target Studio bridge selected, `exec agent.session` reports its project, session, active model and latest request. `exec agent.send message='Help identify the missing CLI capability' new_chat=false session=<reported-session> wait=0 _request_id=<unique-id>` submits to that same conversation; `exec agent.log refresh=true` retrieves and persists its reply, and `agent.session` reports queued/running/replied/stalled/error state. Polling never resends the prompt. `agent.new_chat` explicitly starts a short fresh conversation and archives history. A directory containing multiple `.acproj` files blocks chat rather than mixing contexts.

Active model/provider selection belongs to each Studio engine, initialized from shared defaults when the window loads. The drawer and bridged `agent.config selection='{...}'` change only that instance. Credentials and shared defaults remain unchanged. Booth F9 composes into the pinned project chat; Enter sends through the current CLI, Esc retains the draft. Status/activity remain visible. Blue means not started, red failed, yellow stalled, green working; `[OK]` marks confirmed completion.
