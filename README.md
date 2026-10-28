# Magic Studio

Canonical repository: [ontariodungeons/magic-studio](https://github.com/ontariodungeons/magic-studio). Jesse reviews the app built from `main`.

The application is the Tauri shell in `apps/magicstudio-shell`. Its embedded UI, CLI and MCP share the Rust engine command registry and domain types.

## Run and discover

```bash
cargo build -p magicstudio-shell -p mc
./target/debug/magicstudio-shell
mc --help
mc commands
```

The Omarchy menu entry is **Magic Studio**. Rebuild the shell after changing `apps/magicstudio-shell/ui/`; those files are embedded. Keep generated song projects and media on the user-selected Speedy location, outside this source repository.

The native **File** menu provides New Project (`Ctrl+N`), Open (`Ctrl+O`), Save (`Ctrl+S`), Close Project (`Ctrl+W`), Application Settings (`Ctrl+,`) and guarded Quit (`Ctrl+Q`). New uses the same workflow blocks as Home. Application Settings is available without an open project and persists the production project root through the shared `project.root` command; it does not move existing projects. Agent/model settings remain separate in Sproket. The native Edit menu supplies standard text editing actions.

## CLI and MCP

`mc exec <id> key=value` executes one command. `mc run <file.jsonl|->` keeps one headless session across JSONL commands with `id` and `params`. Separate `exec` calls do not share the open project.

Register one Magic Studio MCP server per client: command `mc`, args `["mcp"]`. It uses a headless engine session. To use an already configured live Studio bridge, explicitly select its port:

```bash
mc --bridge <port> exec ui.state
mc --bridge <port> mcp
```

A Studio window must have been started with `magicstudio-shell --control <port>` for that bridge to exist. Before restarting Studio, inspect pending/dirty state and preserve unsaved work. `ui.state` does not report unsaved-change safety.

## Command Center

`mc center` opens the native Ratatui Command Center. `center project=/absolute/project.magic` resumes inspection; `center --print project=...` prints its state. F1/F2/F3/F4 switch the Workshop, Booth, Muppets and Telemetry tabs; F6 shows intake. F3 opens, F5 creates, F6 explicitly resumes intake, F7 populates the editable DeckCraft template (or reopens the existing document), F8 refreshes; Ctrl+S saves and Ctrl+Q exits without losing saved state. Inputs can also be supplied as `root= name= audio= transcript= captions= audiomap=` arguments. Studio never launches automatically.

The Telemetry tab (F4) is read-only and on demand: while it is open it samples
host CPU, memory and swap, the busiest processes, the Blender worker log, the
Studio's own activity journal, the Omarchy user journal and each coding agent's
recorded quota (`omarchy agent usage`); closing the tab stops all of it. The
gang strip selects an agent with `,` and `.`; `P` pokes the selected idle or
stalled agent with a status request and `p` opens the composer to write your
own message. A working agent is never interrupted, and a refusal from the
multiplexer is reported as a refusal.

Live TUI control uses `mc tui`, which talks to the running terminal instance rather than constructing a separate headless session. `mc tui look` returns the actual rendered rows, selected controls, agent identities/activity, events, project catalog, render-worker receipt, telemetry and input drafts as JSON. Every response follows a completed frame and carries its instance, generation and frame number. `mc tui read` prints only that screen text; `mc tui controls` lists all named controls and keyboard equivalents. Indices are zero-based. The numbered agent cards and MOPS agent tabs are clickable; `G` opens MOPS, `1`–`9`/`0` select its agents and left/right cycle them. `mc tui click id=<id>` follows the same hit target as a mouse click. With several TUIs open, use `mc tui instances` and add `instance=<pid>`; the CLI refuses to guess.

Booth aligns stage labels and status lights in fixed columns. Its middle panel shows project/Studio/save/selection/media facts from cached observations. `F9` or the Sproket title button opens project chat; F9/Escape closes it while retaining the draft. Drafts stay bound to their originating project; selecting another project cannot redirect them. Enter sends only from that window. Offline or mismatched Studio connections stay unavailable and retain drafts; the TUI does not launch an app to send. CLI equivalents are `mc tui chat`, `mc tui chat text=...`, `mc tui chat.send`, and `mc tui chat.close`. `mc tui look section=project_overview` exposes the same facts. Artwork borders animate at most four times per second only while a fresh worker observation says running; completed, stale and unavailable observations remain static. Green is limited to status lights in that panel; its frame and low-memory bars use blue.

Jesse and Gonzo use the master **WORKSHOP** (`mc workshop`, or F1). **ANIMAL** is the F4 telemetry view; `view name=telemetry` remains an alias for `animal`. The master shows host dials and all projects. An assigned agent sees only BOOTH, project MOPS, and project ANIMAL.

Select a Workshop card and press **B / Agent B** to prepare an assignment without launching or messaging an agent. To explicitly assign one teammate and its task, use:

```bash
mc tui booth.prepare project=/absolute/song.magic pane=<pane-id> name=<agent-name> task='The assigned task'
# Optional explicit additional project commands: allow=project.save,motion.sample
```

The response contains `onboarding.assignment`, `onboarding.launch`, and `onboarding.environment`. Put `MAGICSTUDIO_ASSIGNMENT=<that assignment file>` in the **agent process environment**, including its MCP configuration; each child `mc` command then inherits the scope. To inspect its Booth manually, run the returned launch command. Do not give it a master TUI instance ID. By default the scope permits project inspection and notes to explicitly assigned teammates; additional engine commands require a master grant. Sproket history can be read, while sending requires `allow=agent.send`. Reprepare the same project to add a teammate or replace grants, then reload its Booth/MCP session.

This context filters CLI/MCP command discovery, validates project identity before dispatch, prevents save-as into another project, and keeps native TUI endpoints in a project-specific namespace. Project MOPS uses Workshop assignment tasks and project receipts; it never reads a teammate's unrelated terminal history. Project ANIMAL excludes host processes, host logs and other projects' worker ownership. These are workflow permissions under the shared Linux account, not an OS/filesystem sandbox; the assigned environment must stay configured.

Workshop uses responsive project cards (one to four columns), with observed active projects first and an `I` toggle for inactive projects. Unknown discovery stays `Unknown`; a frame meter appears only for an observed project-matched worker receipt. Arrows or `j`/`k` select without attaching; click selects a card, and Enter/Open or F2 enters its Booth. `D` opens selected-project details, including the full path and copyable CLI commands; `C` copies the selected command without executing it. Repeated names include a short folder label. The Workshop film header reports fleet counts rather than another project's render.

`mc tui project.select index=0` selects without attaching; `mc tui project.open-selected` opens Booth; `mc tui project.inactive show=true` expands inactive projects. `mc tui look section=workshop` returns selected-project data, card observations, visible ordering and column count. Project indices refer to the shared catalog, not their sorted screen positions.

```bash
mc tui look
mc tui look section=worker
mc tui view name=mops
mc tui agent name=gonzo           # use pane=<id> when names repeat
mc tui look section=elements      # visible button IDs and terminal-cell rectangles
mc tui click id=agent-card-wG:p7
mc tui event index=0
mc tui note text='Additional details for the selected agent'
mc tui note.send                  # explicit submission; working/unavailable recipients refuse
mc tui view name=telemetry
mc tui look section=telemetry
mc tui key key=F8                 # same handler as the keyboard, including all form controls
mc tui reload                    # same foreground process; waits for a fresh generation/frame
```

Reading never submits a note or project chat. Composing and sending are separate actions; refused submissions retain the note draft. `task_source` distinguishes a terminal title from a recorded assignment. Telemetry reports its sampling state and age; unavailable values remain null. `mc tui input field=audio value=/path/song.wav` edits the intake form; `intake.create`, `intake.resume`, `project.open`, `project.save`, `storyboard.open` and `doctor` invoke the same guarded operations as their keys. `mc tui key` and `mc tui text` expose every remaining keyboard and text-input control without synthetic input into another terminal pane. Full CLI parity does not imply an automatic agent follow-up queue or a complete filesystem project inventory.

Scripted clients use the same `project.intake` and `center.status` engine commands. UI and CLI share the production root `/run/media/jesse/Speedy/media`, stored as `projects_root` in `$XDG_CONFIG_HOME/magicstudio/settings.json` (or `~/.config/magicstudio/settings.json`). Inspect it with `mc exec project.root`; persist it with `mc exec project.root root=/run/media/jesse/Speedy/media`. `project.intake name=...` resolves `<root>/<song-slug>/project/<Name>.magic`. Explicit `root=...` or `path=...` choices apply only to that request, appear in the intake receipt, and never change the production setting. Recent history never determines the root. Existing projects require `resume=true`; changed transcript replacement requires explicit `replace_transcript=true`. Received bytes and errors persist in the project even when validation fails. The transcript importer accepts native HyperFrames word arrays, with original SRT when supplied or an explicitly provisional word-by-word fallback using existing timestamps. Recording duration is probed without transcription or imported from an audiomap. Raw text/IDs/timing are preserved.

`center.status trace=true` emits a timestamped shared prerequisite/metric observation and records started/result/completed evidence in `center.events`; ordinary polling returns observation IDs without filling the last-three action history. Studio and Booth consume the same prerequisite states. Missing data remains unknown/missing, and Studio rendering acknowledgement is separate from engine readiness.

Recent activity stores three root action groups per project. Engine results and matched Studio acknowledgements remain separate; closed projects use a disk snapshot. Control shows the existing ten-entry recent-project catalog, not a complete filesystem inventory; selecting a different catalog project is not implemented.

Tauri New/Open invoke the adjacent CLI through the selected Studio bridge. `storyboard.init` waits for the creator’s selected DeckCraft template and builds from the imported transcript and canonical scene spans; `storyboard.init empty=true` without a selection creates an empty manifest. It registers the resulting deck and preserves an existing active deck on rerun. Studio uses its existing continuous document picture-placement and Save-new-version path. DeckCraft is the storyboard authoring document; PDF is an export. Existing saved PDF-form edits are carried into the native deck on its next save. The Open storyboard action opens that deck in DeckCraft; Choose/Open template uses the creator’s selected native template. Existing native storyboard text fields are editable in place and save when the field changes; `mc --bridge <port> storyboard edit scene 14 "text" field=action` is a short spelling of the same `storyboard.edit` operation. `storyboard.read scene=14` returns the actual native `text_fields` names and values; text edits preserve the existing document and do not regenerate its template. Creative fields remain instructional until authored. Current shell source exposes guarded dirty-state and UI-feedback queries; a running older shell must be rebuilt and safely restarted before those updated contracts apply. Its UI mapping panel labels remaining gaps. Audio-tool provider/model provenance is reported unknown when the received source did not record it.

### Open independent project windows

`mc app` opens a fresh project window and returns its ready instance immediately. `mc app /absolute/project.magic` opens that document in its own window, or focuses its existing window. The application allocates an available control endpoint automatically; no workspace or port choice is required. File > New/Open preserve the current document in its window. File > Close closes only that document; Quit closes only that window after the existing unsaved-work guard.

MCP `app_open` uses the same opening operation and attaches that MCP session to the returned instance. An attached MCP session retains its instance and project identity; a stopped or changed target is an error, never a reason to select another window. Existing explicit `--bridge` endpoints remain available for inspection.

## Design every lyric line

`mc exec words.design` (or `mc --bridge <port> exec words.design` with the Studio open) designs every sung line from the project's `lyrics-design.json` and uses it in the video. The recipe sets the palette, the band (`top` or `bottom`), the base lettering and the hero words: each hero's lettering, size, colour, move (`pop`, `settle`, `fall`, `rise`, `loop`, `sleep`, `east`, `west`) and optional stroke (`underline`, `swoop`, `loop`, `hill`, `down`). Lines are measured with the fonts the video renders, so nothing leaves the frame. The caption track keeps only the sections without a design. Fine-tune any line afterwards on the Design page.

## Rig an image

`mc --bridge <port> exec image.generate reference_asset=captain rig_parts='[{"id":"legs","description":"both legs and boots","parent":"body"},{"id":"body","description":"coat and torso"},{"id":"head","description":"head, hat and beard","parent":"body"}]' prompt='The captain from the reference' name=captain_rig_sheet` has Gaffer draw a rig sheet from `pipeline/templates/rig_sheet_spec.json`: the assembled asset in cell 1, then each part (back to front) with its hidden joint areas painted in. `mc --bridge <port> exec rig.from_sheet sheet=captain_rig_sheet name=captain_rig` cuts the cells, places each part on the original image, takes every visible pixel from the original and the sheet only for hidden areas, finds the joints, and writes `assets/characters/<name>/rig/rig.json` with `check.png`, `layers.png` and `placement.json` to review. Correct a joint without a new sheet with `pivots='{"arm":[0.9,0.6]}'` ([u, v] inside the part). Connect it with the `next` command it prints.

## Animate a song section

`mc --bridge <port> exec motion.place_section character=ship_rig track=rest points='[[338,660],[768,580],[1198,660]]' id=ship-s1 start=0 duration=17.7 rect='[251,0,1417,1080]'` draws the path on a loaded layered rig, exports it as a transparent clip, runs the motion gate, and places the clip at the section's time in the HyperFrames video. A clip that fails the gate is returned with its failed checks and is not placed. Cut a rig from one image first with `image.break_apart ... rig_order='["flag","rigging","hull"]'`.

## Generate an image

`mc exec image.generate project=/absolute/project.magic prompt='Describe the artwork' model=gpt-6-luna quality=auto` uses a fresh Codex session and the native image tool with the current login. The same `image.generate` command is exposed to MCP and the separate Project image-prompt block; it does not change chat settings. `mc commands image.generate` describes the contract. `mc` is the canonical CLI executable and package.

Generated PNGs and request receipts live in the selected project's `assets/generated-images/<request>/`. The one-shot subprocess stops after its first successful native image receipt; the adapter copies the receipt-correlated staged bytes rather than trusting an agent-authored file or save claim. A successful native tool receipt and full PNG decode are required before `project.add_asset` attaches it. Failures retain received files and logs without approval claims. The selected Codex model is distinct from the underlying image model; only `quality=auto` is verified, exact resolution is tool-controlled and actual dimensions are returned. Generation serializes engine mutations; the shell remains responsive but other engine requests wait. This block needs a rebuilt shell; terminal success alone does not verify UI rendering.

Reference-driven action sheets use the same native command: `image.generate reference_asset=<registered PNG id> poses='["three-quarter","front"]' character=<rig id> scene=2 animation=turn-around prompt="Preserve this character"`. The receipt retains the source artwork, pose order and storyboard context. `video.place_track generation_receipt=<generation.json>` carries that provenance into a rendered clip placement. See [reference → animation → video](docs/workflows/reference-to-animation-video.md) for the full workflow and its current manual steps.

## Blender animation

The Animation tab's Blender mode renders in a separate, resource-limited worker. `blender.start`,
`blender.status`, `blender.cancel` and `blender.list` expose the same workflow to CLI/MCP.
Existing layered tracks can be migrated into packed editable `.blend` scenes; subsequent
renders load that scene. See [Blender animation](docs/workflows/blender-animation.md) for
limits, preview/video handoff and the staged replacement of the legacy editor.

## Existing material intake

Discover contracts from `mc commands workshop` and `commands project`. `workshop.set_audio` references original audio. `workshop.import_transcript` accepts JSON with optional original SRT via `captions`; preserve every word/timestamp and distinguish raw transcription from corrections. Shared `project.intake` reuses valid supplied/imported timing without ASR, otherwise runs the existing `audio.analyze` pipeline for linked audio, imports its native output and initializes a native DeckCraft document. Invalid supplied timing stops for correction instead of silently retranscribing. Pipeline/document failures keep the project openable with `intake_ready=false`, failed steps and `next_commands`; linking audio alone is not readiness. Direct audio analysis/transcription commands remain available for explicit retries. Existing imported or generated files are not proof of approval.

For a fresh song, run `mc exec project.intake path=/absolute/song/project/Song.magic transcript=/absolute/words.json captions=/absolute/song.srt audiomap=/absolute/audiomap.json`; omit optional inputs you do not have. Inspect `intake_ready`, `storyboard_ready`, `steps`, `problems` and `next_commands` in the response. `ok:true` means the request was handled. Inspect the saved project with `mc /absolute/song/project/Song.magic workshop.status` and `mc /absolute/song/project/Song.magic storyboard.init`. The latter preserves an existing editable document. Workshop validates the actual transcript bytes, including a native storyboard's source, before marking transcription complete; a missing, malformed or invalid transcript remains incomplete after reopen. A starting storyboard still needs creative authoring and visual review.

New song opens directly in the existing Project workflow and its colored status lights; the retired popup and its handlers are removed. Home and Ctrl+N share `beginNewProject`. Saving the chosen audio invokes `project.intake prepare_only=true`: the upstream music-to-video setup runs `hyperframes init --example blank --skill hyperframes --audio <recording> --skip-transcribe --non-interactive`, its native timeline confirms the audio track, and `hyperframes preview` starts the embedded Studio server. The storyboard starts empty and waits for the creator’s selected DeckCraft template (`storyboard.template path=...`); no layout, transcription, scene guesses or artwork placements are invented. HyperFrames is the sole Studio target. Add assets registers generic images; drag them onto the page only where you choose. The existing Transcription block accepts timed lyrics or an explicitly selected audio transcription. `audio.analyze` sends the original recording to the installed `hyperframes transcribe --model large-v3` (override with `model=...`), then uses native SRT export; optional music analysis uses `hyperframes audio --only sections,onsets,moments --json` and retains its raw result in `hyperframes/audio-analysis.json`; it does not first transcode the song. Creating scene pages from that timing is another explicit action. Project controls invoke the adjacent `mc` executable on the window’s selected control port. The normal CLI `project.intake` chain remains available for deliberately automated intake.

`storyboard.design` reports the existing content check and the named fields still needing work. Animation and Lyrics UI access follows that check. **Proceed with this draft** invokes `workshop.approve_draft approved=true allow_incomplete=true`; it permits access for the current document while retaining the missing-field report. Editing the document or changing its audio reference invalidates that document-specific override. `center.status` includes `creative_followup`: missing fields, registered assets and suggested commands for the LLM to discuss with the creator. It is context for chosen follow-up work, not permission to generate or place artwork automatically. Live app behavior and saved native output remain distinct from creative approval or a completed video.

Storyboards and media commands operate in the selected project. `video.init target=hyperframes hyperframes_cli=/absolute/path/to/hyperframes` invokes the existing official initializer with transcription deferred and official skill setup enabled. It passes linked audio to official `init --audio --skip-transcribe` and imports existing timed JSON through official `transcribe <transcript.json> --dir <project> --json --no-runtime-install` (file import, no ASR). Existing projects are reused; `replace_scaffold=true` is rejected — official init never rebuilds or replaces an existing project. Artist sources, existing captions and artwork are preserved. This imports song/timing, not completed authored visuals or a rendered video. The retired music-to-video assembler is rejected before any project mutation; current authoring instructions are installed by `hyperframes skills update music-to-video`, which now resolves to `/hyperframes` and its music references. `video.server_start` accepts the same executable parameter; otherwise both resolve `HYPERFRAMES_CLI` or `hyperframes` on PATH, without an unversioned npx install. `storyboard.design` reports active `deck`/`pdf` paths and whether a real design draft is pending. `storyboard.save_version` saves the design itself; an additional `project.save` is not needed to clear design changes. Saved snapshots reopen clean, including legacy snapshots with a stale dirty flag; actual pending placements or removals stay dirty. Preview-image display is not native editable slide-object support. Keep runtime/tool failures and unavailable integration visible; do not substitute custom production pipelines.

`storyboard.import_openmontage path=/existing/project` imports labelled scene content; the obsolete static-document copy branch is removed. `workshop.build_storyboard draft=true` populates the selected native DeckCraft template layout internally; the storyboard PDF is an export, not the authoring surface. Studio keeps its PDF view, image drop and Save controls. The source words/timestamps are read directly; creative note mapping reports unknown sections rather than discarding or rewriting them. Review the returned draft, then `storyboard.save_version draft=<manifest.json>` adopts it. `storyboard.edit` and on-page text editing fill/save the native deck’s text fields. An explicit `transcript=<native JSON>` override requires `draft=true`; `allow_timing_overrun=true` permits an unvalidated draft with a visible warning, preserving every original timestamp. It does not validate timing, trim words or rerun ASR. Unchanged draft resume preserves saved edits.

## Source map

| Area | Source |
|---|---|
| Studio UI and transport | `apps/magicstudio-shell/ui/`, `apps/magicstudio-shell/src/` |
| Terminal and MCP | `apps/mc/src/` |
| Command contracts/dispatch | `crates/engine/src/commands.rs`, `lib.rs` |
| Project persistence | `crates/engine/src/project.rs` |
| Workshop/transcript | `crates/engine/src/workshop.rs` |
| Storyboard | `crates/engine/src/storyboard.rs` |
| Tool resolution | `crates/engine/src/workspace.rs` |
| Current Studio interaction contract | `apps/magicstudio-shell/agent.md` |

Read `AGENTS.md` before changes. Run `cargo check --workspace` and the checks that match the change, and report what actually ran. Commit explicit owned paths; no generated media, credentials or local agent configuration. Source history is recovered through Git, not duplicated application trees in this checkout.

Shared audio intake retains the official HyperFrames audio project and deterministically binds native timed words into `compositions/captions.html` using the versioned official caption-highlight template. `video.init captions=true` is idempotent: unchanged input preserves caption-source edits and `caption-overrides.json`; changed timing conflicts rather than overwriting an edited source. Initial five-word groups are provisional, not semantic/listening approval. `project.intake hyperframes_cli=/absolute/path` uses the same operation; its audio extraction skips optional beat analysis (`audio.analyze analyze_beats=false`) because captions do not require librosa.

## Project agent coordination

Studio and Booth use the existing `agent.send`, `agent.log` and `agent.session` commands. With the target Studio bridge selected, `exec agent.session` reports its project, session, active model and latest request. `exec agent.send message='Help identify the missing CLI capability' new_chat=false session=<reported-session> wait=0 _request_id=<unique-id>` submits to that same conversation; `exec agent.log refresh=true` retrieves and persists its reply, and `agent.session` reports queued/running/replied/stalled/error state. Polling never resends the prompt. `agent.new_chat` explicitly starts a short fresh conversation and archives history. A directory containing multiple `.magic` files blocks chat rather than mixing contexts.

Active model/provider selection belongs to each Studio engine, initialized from shared defaults when the window loads. The drawer and bridged `agent.config selection='{...}'` change only that instance. Credentials and shared defaults remain unchanged. Booth F9 composes into the pinned project chat; Enter sends through the current CLI, Esc retains the draft. Status/activity remain visible. Blue means not started, red failed, yellow stalled, green working; `[OK]` marks confirmed completion.

Contained asset, recording and HyperFrames paths are stored relative to the `.magic` file and resolved from its folder on reopen. Legacy absolute connections remain readable. Project text drafts survive UI refreshes; File > Save includes pending storyboard edits. HyperFrames Studio edits its composition files directly inside the same project folder.

Project, storyboard and Design read `hyperframes/transcript.json` first. Imports write the native word array there and preserve any legacy project-root transcript files; one shared reader adds SRT phrase grouping in memory without maintaining another editable copy.
