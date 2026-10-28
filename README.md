# Magic Studio

Canonical repository: [ontariodungeons/magic-studio](https://github.com/ontariodungeons/magic-studio). Jesse reviews the app built from `main`.

The application is the Tauri shell in `apps/magicstudio-shell`. Its embedded UI, CLI and MCP share the Rust engine command registry and domain types.

## Run and discover

```bash
CARGO_BUILD_JOBS=1 cargo build -p magicstudio-shell
./target/debug/magicstudio-shell
magic --help
magic commands
```

The Omarchy menu entry is **Magic Studio**. Rebuild the shell after changing `apps/magicstudio-shell/ui/`; those files are embedded. Keep generated song projects and media on the user-selected Speedy location, outside this source repository.

The native **File** menu provides New Project (`Ctrl+N`), Open (`Ctrl+O`), Save (`Ctrl+S`), Close Project (`Ctrl+W`), Application Settings (`Ctrl+,`) and guarded Quit (`Ctrl+Q`). New uses the same workflow blocks as Home. Application Settings is available without an open project and persists the production project root through the shared `project.root` command; it does not move existing projects. Agent/model settings remain separate in the chat drawer. The native Edit menu supplies standard text editing actions.

## CLI and MCP

`magic` is the CLI-Anything Click + ReplSkin harness for Magic Studio (`agent-harness/cli_anything/magic/`). It provides first-class domain command groups (`magic studio`, `magic project`, `magic storyboard`, `magic character`, `magic part`, `magic image`, `magic motion`, `magic animation`, `magic audio`, `magic words`, `magic video`, `magic skills`, `magic crew`, `magic agent`, `magic analytics`, `magic ui`, `magic export`, `magic session`, `magic app`) plus `magic exec <id> [key=value ...]` and `magic run [file.jsonl|-]`.

Session state is persisted in `$XDG_RUNTIME_DIR/magicstudio/` (scoped per `HERDR_PANE_ID` when running inside Herdr). After `magic project open <project.magic>` or `magic studio <project.magic>`, subsequent commands automatically target the live Studio window if open, or execute headlessly via `magicstudio-shell --headless` and persist to the `.magic` file.

Register one Magic Studio MCP server per client: command `magic`, args `["mcp"]`. To target an explicit Studio bridge port or socket:

```bash
magic --bridge <port> ui state
magic --bridge <port> mcp
```

A Studio window must have been started with `magicstudio-shell --control <port>` (or discovered via `$XDG_RUNTIME_DIR/magicstudio-studio/*.sock`) for that bridge to exist. Before restarting Studio, inspect pending/dirty state via `magic ui feedback` and preserve unsaved work.

### Open independent project windows

`magic studio new` (or `magic app open`) opens a fresh project window and binds the session to its ready instance. `magic studio <project.magic>` opens that document in its own window, or focuses its existing window. The application allocates an available control endpoint automatically; no workspace or port choice is required. File > New/Open preserve the current document in its window. File > Close closes only that document; Quit closes only that window after the existing unsaved-work guard.

MCP `app_open` uses the same opening operation and attaches that MCP session to the returned instance. An attached MCP session retains its instance and project identity; a stopped or changed target is an error, never a reason to select another window. Existing explicit `--bridge` endpoints remain available for inspection.

## Design every lyric line

`magic words design` designs every sung line from the project's `lyrics-design.json` and uses it in the video. The recipe sets the palette, the band (`top` or `bottom`), the base lettering and the hero words: each hero's lettering, size, colour, move (`pop`, `settle`, `fall`, `rise`, `loop`, `sleep`, `east`, `west`) and optional stroke (`underline`, `swoop`, `loop`, `hill`, `down`). Lines are measured with the fonts the video renders, so nothing leaves the frame. The caption track keeps only the sections without a design. Fine-tune any line afterwards on the Design page.

## Rig an image

`magic image generate reference_asset=captain rig_parts='[{"id":"legs","description":"both legs and boots","parent":"body"},{"id":"body","description":"coat and torso"},{"id":"head","description":"head, hat and beard","parent":"body"}]' prompt='The captain from the reference' name=captain_rig_sheet` has Gaffer draw a rig sheet from `pipeline/templates/rig_sheet_spec.json`: the assembled asset in cell 1, then each part (back to front) with its hidden joint areas painted in. `magic character rig-from-sheet sheet=captain_rig_sheet name=captain_rig` cuts the cells, places each part on the original image, takes every visible pixel from the original and the sheet only for hidden areas, finds the joints, and writes `assets/characters/<name>/rig/rig.json` with `check.png`, `layers.png` and `placement.json` to review. Correct a joint without a new sheet with `pivots='{"arm":[0.9,0.6]}'` ([u, v] inside the part). Connect it with the `next` command it prints.

## Animate a song section

`magic motion place-section character=ship_rig track=rest points='[[338,660],[768,580],[1198,660]]' id=ship-s1 start=0 duration=17.7 rect='[251,0,1417,1080]'` draws the path on a loaded layered rig, exports it as a transparent clip, runs the motion gate, and places the clip at the section's time in the HyperFrames video. A clip that fails the gate is returned with its failed checks and is not placed. Cut a rig from one image first with `magic part break-apart ... rig_order='["flag","rigging","hull"]'`.

## Generate an image

`magic image generate project=/absolute/project.magic prompt='Describe the artwork' model=gpt-6-luna quality=auto` uses a fresh Codex session and the native image tool with the current login. The same `image.generate` command is exposed to MCP and the separate Project image-prompt block; it does not change chat settings. `magic commands image.generate` describes the contract. `magic` is the canonical CLI executable and package.

Generated PNGs and request receipts live in the selected project's `assets/generated-images/<request>/`. The one-shot subprocess stops after its first successful native image receipt; the adapter copies the receipt-correlated staged bytes rather than trusting an agent-authored file or save claim. A successful native tool receipt and full PNG decode are required before `project.add_asset` attaches it. Failures retain received files and logs without approval claims. The selected Codex model is distinct from the underlying image model; only `quality=auto` is verified, exact resolution is tool-controlled and actual dimensions are returned. Generation serializes engine mutations; the shell remains responsive but other engine requests wait. This block needs a rebuilt shell; terminal success alone does not verify UI rendering.

Reference-driven action sheets use the same native command: `image.generate reference_asset=<registered PNG id> poses='["three-quarter","front"]' character=<rig id> scene=2 animation=turn-around prompt="Preserve this character"`. The receipt retains the source artwork, pose order and storyboard context. `video.place_track generation_receipt=<generation.json>` carries that provenance into a rendered clip placement. See [reference → animation → video](docs/workflows/reference-to-animation-video.md) for the full workflow and its current manual steps.

## Blender animation

The Animation tab's Blender mode renders in a separate, resource-limited worker. `blender.start`,
`blender.status`, `blender.cancel` and `blender.list` expose the same workflow to CLI/MCP.
Existing layered tracks can be migrated into packed editable `.blend` scenes; subsequent
renders load that scene. See [Blender animation](docs/workflows/blender-animation.md) for
limits, preview/video handoff and the staged replacement of the legacy editor.

## Existing material intake

Discover contracts from `magic commands workshop` and `magic commands project`. `workshop.set_audio` references original audio. `workshop.import_transcript` accepts JSON with optional original SRT via `captions`; preserve every word/timestamp and distinguish raw transcription from corrections. Shared `project.intake` reuses valid supplied/imported timing without ASR, otherwise runs the existing `audio.analyze` pipeline for linked audio, imports its native output and initializes a native DeckCraft document. Invalid supplied timing stops for correction instead of silently retranscribing. Pipeline/document failures keep the project openable with `intake_ready=false`, failed steps and `next_commands`; linking audio alone is not readiness. Direct audio analysis/transcription commands remain available for explicit retries. Existing imported or generated files are not proof of approval.

For a fresh song, run `magic exec project.intake path=/absolute/song/project/Song.magic transcript=/absolute/words.json captions=/absolute/song.srt audiomap=/absolute/audiomap.json`; omit optional inputs you do not have. Inspect `intake_ready`, `storyboard_ready`, `steps`, `problems` and `next_commands` in the response. `ok:true` means the request was handled. Inspect the saved project with `magic --project /absolute/song/project/Song.magic video status` and `magic --project /absolute/song/project/Song.magic storyboard init`. The latter preserves an existing editable document. Workshop validates the actual transcript bytes, including a native storyboard's source, before marking transcription complete; a missing, malformed or invalid transcript remains incomplete after reopen. A starting storyboard still needs creative authoring and visual review.

New song opens directly in the existing Project workflow and its colored status lights; the retired popup and its handlers are removed. Home and Ctrl+N share `beginNewProject`. Saving the chosen audio invokes `project.intake prepare_only=true`: the upstream music-to-video setup runs `hyperframes init --example blank --skill hyperframes --audio <recording> --skip-transcribe --non-interactive`, its native timeline confirms the audio track, and `hyperframes preview` starts the embedded Studio server. The storyboard starts with the canonical Magic Studio DeckCraft template (`storyboard.template` inspects its path); transcription, scene creation and artwork placement remain explicit actions. HyperFrames is the sole Studio target. Add assets registers generic images; drag them onto the page only where you choose. The existing Transcription block accepts timed lyrics or an explicitly selected audio transcription. `audio.analyze` sends the original recording to the installed `hyperframes transcribe --model large-v3` (override with `model=...`), then uses native SRT export; optional music analysis uses `hyperframes audio --only sections,onsets,moments --json` and retains its raw result in `hyperframes/audio-analysis.json`; it does not first transcode the song. Creating scene pages from that timing is another explicit action. Project controls invoke the adjacent `magic` executable on the window’s selected control port. The normal CLI `project.intake` chain remains available for deliberately automated intake.

`storyboard.design` reports the existing content check and the named fields still needing work. Animation and Lyrics UI access follows that check. **Proceed with this draft** invokes `workshop.approve_draft approved=true allow_incomplete=true`; it permits access for the current document while retaining the missing-field report. Editing the document or changing its audio reference invalidates that document-specific override. Live app behavior and saved native output remain distinct from creative approval or a completed video.

Storyboards and media commands operate in the selected project. `video.init target=hyperframes hyperframes_cli=/absolute/path/to/hyperframes` invokes the existing official initializer with transcription deferred and official skill setup enabled. It passes linked audio to official `init --audio --skip-transcribe` and imports existing timed JSON through official `transcribe <transcript.json> --dir <project> --json --no-runtime-install` (file import, no ASR). Existing projects are reused; `replace_scaffold=true` is rejected — official init never rebuilds or replaces an existing project. Artist sources, existing captions and artwork are preserved. This imports song/timing, not completed authored visuals or a rendered video. The retired music-to-video assembler is rejected before any project mutation; current authoring instructions are installed by `hyperframes skills update music-to-video`, which now resolves to `/hyperframes` and its music references. `video.server_start` accepts the same executable parameter; otherwise both resolve `HYPERFRAMES_CLI` or `hyperframes` on PATH, without an unversioned npx install. `storyboard.design` reports active `deck`/`pdf` paths and whether a real design draft is pending. `storyboard.save_version` saves the design itself; an additional `project.save` is not needed to clear design changes. Saved snapshots reopen clean, including legacy snapshots with a stale dirty flag; actual pending placements or removals stay dirty. Preview-image display is not native editable slide-object support. Keep runtime/tool failures and unavailable integration visible; do not substitute custom production pipelines.

`storyboard.import_openmontage path=/existing/project` imports labelled scene content; the obsolete static-document copy branch is removed. `workshop.build_storyboard draft=true` populates the canonical native DeckCraft template layout internally; the storyboard PDF is an export, not the authoring surface. Studio keeps its PDF view, image drop and Save controls. The source words/timestamps are read directly; creative note mapping reports unknown sections rather than discarding or rewriting them. Review the returned draft, then `storyboard.save_version draft=<manifest.json>` adopts it. `storyboard.edit` and on-page text editing fill/save the native deck’s text fields. An explicit `transcript=<native JSON>` override requires `draft=true`; `allow_timing_overrun=true` permits an unvalidated draft with a visible warning, preserving every original timestamp. It does not validate timing, trim words or rerun ASR. Unchanged draft resume preserves saved edits.

## Source map

| Area | Source |
|---|---|
| Studio UI and transport | `apps/magicstudio-shell/ui/`, `apps/magicstudio-shell/src/` |
| CLI-Anything `magic` harness & MCP | `agent-harness/cli_anything/magic/` |
| Command contracts/dispatch | `crates/engine/src/commands.rs`, `lib.rs` |
| Project persistence | `crates/engine/src/project.rs` |
| Workshop/transcript | `crates/engine/src/workshop.rs` |
| Storyboard | `crates/engine/src/storyboard.rs` |
| Tool resolution | `crates/engine/src/workspace.rs` |
| Current Studio interaction contract | `apps/magicstudio-shell/agent.md` |

Read `AGENTS.md` before changes. Run `cargo check --workspace` and the checks that match the change, and report what actually ran. Commit explicit owned paths; no generated media, credentials or local agent configuration. Source history is recovered through Git, not duplicated application trees in this checkout.

Shared audio intake retains the official HyperFrames audio project and deterministically binds native timed words into `compositions/captions.html` using the versioned official caption-highlight template. `video.init captions=true` is idempotent: unchanged input preserves caption-source edits and `caption-overrides.json`; changed timing conflicts rather than overwriting an edited source. Initial five-word groups are provisional, not semantic/listening approval. `project.intake hyperframes_cli=/absolute/path` uses the same operation; its audio extraction skips optional beat analysis (`audio.analyze analyze_beats=false`) because captions do not require librosa.

## Project agent coordination

Studio uses the existing `agent.send`, `agent.log` and `agent.session` commands. With the target Studio bridge selected, `magic agent session` reports its project, session, active model and latest request. `magic agent send message='Help identify the missing CLI capability' new_chat=false session=<reported-session> wait=0 _request_id=<unique-id>` submits to that same conversation; `magic agent log refresh=true` retrieves and persists its reply, and `agent.session` reports queued/running/replied/stalled/error state. Polling never resends the prompt. `agent.new_chat` explicitly starts a short fresh conversation and archives history. A directory containing multiple `.magic` files blocks chat rather than mixing contexts.

Active model/provider selection belongs to each Studio engine, initialized from shared defaults when the window loads. The drawer and bridged `magic agent config selection='{...}'` change only that instance. Credentials and shared defaults remain unchanged. Status/activity remain visible. Blue means not started, red failed, yellow stalled, green working; `[OK]` marks confirmed completion.

Contained asset, recording and HyperFrames paths are stored relative to the `.magic` file and resolved from its folder on reopen. Legacy absolute connections remain readable. Project text drafts survive UI refreshes; File > Save includes pending storyboard edits. HyperFrames Studio edits its composition files directly inside the same project folder.

Project, storyboard and Design read `hyperframes/transcript.json` first. Imports write the native word array there and preserve any legacy project-root transcript files; one shared reader adds SRT phrase grouping in memory without maintaining another editable copy.
