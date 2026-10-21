# AnimationCraft shell — handoff (2026-10-21)

The Tauri shell in `apps/animationcraft-shell` **is AnimationCraft now**. The egui app
(`crates/ui-egui`, `apps/animationcraft`) is "the older UI": a reference only, tagged
`older-egui-ui`. Don't maintain or extend it. Port every feature into the shell in the shell's design.
The user deletes the older UI after they confirm the checklist below.

## Ownership

You (Codex, Herdr pane `wD:pW`) own this branch, `feat/production-pipeline`, and the running shell on port
19897 from now on. No other agent commits here while you work. If you run the older UI to compare
(port 19898), close it when you're done, following craft-app-lifecycle.

## Read these skills first (in this order)

0. **`.agents/skills/animationcraft-shell-ui/SKILL.md` (repo): the shell's design system. Study it first; every screen follows it.**
1. `~/.agents/skills/artcraft-suite/SKILL.md`: the *craft apps and when to use each one.
2. `~/.agents/skills/animationcraft/SKILL.md`: engine commands, CLI, MCP, the workflow.
3. `.agents/skills/storyboard/SKILL.md` (repo): the storyboard PDF and scene loop.
4. `.agents/skills/layered-motion/SKILL.md` (repo): the motion spec (layers, pivots, parents, tracks with
   `keys`/`oscillate`/`ellipse`/`follow`); `pipeline/layered_motion.py` is its exporter.
5. `~/.agents/skills/2d-asset-to-animation/SKILL.md` and `.agents/skills/image-to-animation/SKILL.md`:
   from artwork to parts, rig and motion.
6. `~/.agents/skills/character-rigging/SKILL.md`, `~/.agents/skills/pose-library-design/SKILL.md`,
   `~/.agents/skills/character-animation-qa/SKILL.md`.
7. `~/.agents/skills/photocraft/SKILL.md` (matting, cut sheet, layers) and `~/.agents/skills/effectcraft/SKILL.md` (exports).
8. `~/.agents/skills/cli-anything/SKILL.md`: script → verify → `animationcraft-cli` command → one-line README example.
9. `.agents/skills/craft-app-lifecycle/SKILL.md`: start and stop the live app cleanly.
10. `~/.agents/skills/figcraft/SKILL.md` / `~/.agents/skills/designcraft/SKILL.md`: UI design and missing icons.
11. Process: `~/.agents/skills/brainstorming/SKILL.md` before each new feature (describe the design,
    then build it live), and `~/.agents/skills/systematic-debugging/SKILL.md` for bugs.

## The user's rules (non-negotiable)

- **Animation happens in AnimationCraft's Animation view, never in HyperFrames or Remotion.** Video targets
  (HyperFrames/Remotion/FilmCraft) only assemble the final video.
- Use the *craft tools and CLIs; no hand-rolled solutions, no silent fallbacks. A feature that isn't
  implemented gets researched, then built with the right tool. Fix the pipeline, never hand-patch outputs.
- Generic, never character-specific code. Creative choices (style, audience, artist) live in `intent.md` /
  `STUDIO.md`, not in code.
- Projects are self-contained: outputs go inside the song folder (never `/tmp`, `.codex`, `~/Documents`).
- Rig and Matting become automated CLI chains, not manual slider screens. In the shell they appear as
  steps: run, green/red light, result picture.
- Upgrade a dependency you touch to its latest **stable** release (never alpha or beta: e.g. Tauri stays on
  2.x while 3.0 is in alpha) and note the version change in the commit. Don't start unrelated upgrade sweeps.
- Commit at every pause, with messages that read as a memory trace. Never push merriweather to upstream.
- Delete only your own files, by exact path. Use `gio trash` and `xdg-open`.
- Don't start the studio server unless it's needed (it's the red/green switch in the top bar).
- UI: focused, tasteful, icons over words (words go in tooltips). The user wants to spend their time designing with
  the agent, on timing and polish. Keep setup out of their way.
- Never touch other people's servers (ports 3002 and 3021–3023).
- Claude's memory notes with more context: `~/.claude/projects/-home-jesse-Work-omhas-animation/memory/*.md`.

## How the shell works

- `src/main.rs`: Tauri 2.12 with the `unstable` multi-webview feature. The **project** webview (`ui/`) is
  stacked with the **studio** webview (the video target's studio) in one GTK box. `pack()`/`layout()` give each one
  an explicit height; positions are ignored on Linux.
- One `Engine` (`Mutex<animationcraft_engine::Session>`) per window. Page commands and the bridge share it, and
  the engine writes the `.acproj` itself.
- `src/bridge.rs` + `ui/bridge.js`: `--control <port>` uses the same JSON-lines protocol as the older app.
  Use it to see and drive the app:
  ```
  ./target/debug/animationcraft-shell --control 19897 [<project.acproj>] [--open animate|agent] [--view studio]
  animationcraft-cli --bridge 19897 exec ui.look [region=<dom id or css>] [click=<id|label>] output=<png>
  animationcraft-cli --bridge 19897 exec ui.state | ui.elements | ui.click id=… | ui.set id=… value=… | ui.eval js='return …'
  animationcraft-cli --bridge 19897 exec <any engine command> …   # runs on the window's session; the page refreshes
  ```
  Verify **every** visible change with `ui.look` and open the PNG before saying it's done.
- `ui/`: `app.js` (Project view: blocks Song, Lyrics & notes, Transcription, Animate, plus the storyboard
  pages), `home.js` (Home: New song, Open, recent cards), `agent.js` (agent drawer: opencode session or Herdr
  agent, permissions, questions, model, Point, paste or drop pictures), `bridge.js`, `style.css` (tokens on `:root`).
- Build: `cargo build -p animationcraft-shell` (the `ui/` folder is embedded at build time, so rebuild after UI edits).
  The pre-commit hook runs `cargo check` and the tests.

## Next: the Animation view (top bar: Home · Project | Animation | Studio; Split only while the server is on)

- **Gate first:** do the research in `.agents/skills/animationcraft-shell-ui/` (`research.md`), present the
  proposed Animation view to the user, and build only after they approve it.
- **Engine evaluates, page draws. Move the evaluator, don't write a new one.** The older stage already
  evaluates this spec in Rust: `evaluate_layered_motion_spec` and `eval_oscillate_term` in
  `crates/ui-egui/src/panels/center_motion.rs`, written to match `pipeline/layered_motion.py` (the exporter, which
  renders the final video and so is the truth). Move that code into `crates/engine` (it's pure; adapt its
  inputs from `UiState` to plain params) and expose it as commands, e.g. `motion.tracks {character}` and
  `motion.sample {character, track, fps}`, returning per-frame layer transforms with pivots and parents applied.
  Add a parity test against the exporter's numbers on the bee spec. The page only plays the samples on a canvas
  with the layer PNGs. There is no evaluator in JS.
- The stage: the character in its **rest pose** (no blink lids or alternate states stacked), track picker,
  play/scrub, timing against the song.
- Clicking an object opens its **layer panel**: stack order, thumbnails, show/hide, states (eyes open/closed),
  per-layer fix actions.
- The bee: `/run/media/jesse/Speedy/media/merriweather/bumble-bee/Bumble Bee.acproj`. Its rig spec is
  `assets/characters/bee/bee_motion_spec.json`. The mouth is baked into `eyes-open.png`/`eyes-closed.png`.
  The kit in `merriweather-videos/remotion-composer/public/track06/layers/` has `eyes-open-no-mouth.png`,
  `mouth-smile.png` and `mouth-open.png`.

## Known issues seen in the shell

- Fixed in `1cedbfc`: chat polling replayed every message's fade animation and lost selections/question
  drafts. Transcript nodes now persist; model options avoid needless replacement. Enter sends,
  Shift+Enter adds a line, and failed/in-flight sends preserve draft text and attachments. Verified live
  on 19897 (zero repeated fades over 5.2s) and with `tests/agent-drawer.mjs` (25 assertions).
- Global OpenCode `animationcraft` MCP now uses `animationcraft-cli --bridge 19897 mcp`, authorized
  by the user. It follows the shell's project; no project path is in the command. Config was backed up
  and reloaded, and the inner agent verified the connection after switching projects. Restart persistence
  still needs a restart check. Coordinate with the inner agent through the session API, not its composer.
- Design proposal and all 76 review acceptance criteria: `.agents/skills/animationcraft-shell-ui/research.md`.
  User requires the animation inspector on the left and useful artwork after import. Full Animation
  frame approved on 2026-10-21; implementation is in progress. Permanent app knowledge is `apps/animationcraft-shell/agent.md`; keep
  earlier-app/migration instructions here and in coordinator messages, never in that permanent file.

- A project with no storyboard shows a broken image. It needs an empty state with the next step.
- A layered object whose layers don't share one canvas raises an error toast on open (Hey Ho, `older-fish`).
  Show it quietly on the card instead.
- The Animate card stacks every layer, including the blink lids, so the bee gets a strange mouth and eyebrow.
  It needs the rest pose.
- Agent replies show raw markdown. Render it with a vendored library, not a hand-written parser.
- Hey Ho's `remotion/` has no installed packages (`npm error could not determine executable`).

## Pull every feature from the older UI until all of it is in the shell

The older UI stays in the tree (`crates/ui-egui`, `apps/animationcraft`, tag `older-egui-ui`) until the user
confirms the checklist. It's the **source** for every feature: read its code before porting a feature, and
run it beside the shell to compare behavior (`cargo run -p animationcraft -- --control 19898`; the shell uses
19897). Port behavior and engine calls, never egui code or its look. Don't edit or maintain it, and don't delete it
yourself.

Where each area lives in the older UI (`crates/ui-egui/src/`):
- Home, recent, new project: `panels/start.rs`, `panels/workshop.rs` (`new_project_dialog`)
- Menus (every File/Edit/View/Export entry): `menus.rs`, plus the engine's command list (`animationcraft-cli exec command.list`)
- Workshop stages, actions, briefs, subjects, storyboard pane: `panels/workshop.rs`
- Asset header (load, rename, manage), import: `panels/header.rs`, `panels/asset_browser.rs`
- Parts list (visibility, solo, reviewed, anchors, labels, separation), turnaround views: `panels/asset_browser.rs`
- Pose sheet viewer (slices, OCR, overlays, zoom): `panels/pose_viewer.rs`
- Stage, actions, cutout rig evaluation, jump kinematics, zoom: `panels/center.rs`, `panels/center_motion.rs`
- Transport (actions, jump, run lock, WASD, zoom buttons): `panels/transport.rs`
- Pose controls and 3D shape inspector: `panels/right.rs`, `panels/inspector_widgets.rs`, `face_preview.rs`
- 3D preview: `panels/center_3d.rs`; left tools: `panels/left.rs`
- Agent pane (already ported): `panels/agent.rs`
- Bridge commands the CLI/MCP relied on (`ui.*`, `ui.look`, `vision`): `bridge.rs`, `automation.rs`, `lib.rs`

For each ported feature: tick it in the checklist below, with the commit hash.

## Feature checklist: older UI → shell (the user confirms this, then the older UI is deleted)

Legend: ✅ in the shell · ◻ to do · ⚙ becomes a CLI chain (the shell shows a step) · ❓ ask the user first:
a manual screen, or part of the rig/matting CLI chain? Don't decide it yourself.

**Home and project**
- ✅ Home: New song (name, audio, folder), Open, Recent (remove from list)
- ◻ Community links (Discord, ArtCraft) and the workflow steps strip from the older Home
- ◻ "Open AnimationStudio" without a project (standalone stage)
- ◻ Save / Close project (File menu; the engine saves on change, so check what is still needed)
- ✅ Agent pane: opencode session or Herdr agent, permissions, questions, model, Point, paste or drop pictures

**Workshop (Project view)**
- ✅ Song, Lyrics & notes, Transcription, Animate blocks; storyboard pages; studio switch; Split/Studio views
- ◻ Stage actions per block: set audio, set target, add liner notes, panels or assets (`workshop.add`), import
  transcript, split panels (PhotoCraft), build storyboard, approve draft and scenes, remove a file (with confirmation)
- ◻ "Copy agent brief" per stage
- ◻ Add subject: name it, "Add to Animate assets", "Add & open rig view"
- ◻ Storyboard: click a lyric line to open its scene; per-scene edit and approve (`storyboard.*`)
- ◻ Search everything (`project.search`)

**Assets**
- ◻ Import with "Goes to" (character/background) → `project.add_asset`
- ◻ Asset list: load, rename/reorder, remove (`project.load_asset`, `project.update_asset`, `project.remove_asset`)
- ⚙ Make transparent (`asset.make_transparent`, PhotoCraft), extract background, assess, fix list, request sheet
- ⚙ Build cutout rig, cut sheet (PhotoCraft), break apart, puppet warp, color to alpha, content-aware fill
- ❓ Intake: validate, three-question, compare to reference
- ❓ Pose sheet viewer: slice kinds, sheet tools, OCR labels, overlays (anchors, crops, names), fit/100%
- ◻ Turnaround views (front/right/back/left), optimize accuracy

**Animation view**
- ✅ `964aba3` Stage with rest pose; zoom (fit, 100%, ±), center (fit centers the artwork)
- ✅ `8dbf449` Pen path, show/hide guide, grid, duration/start/end scale sliders, path undo; refined section colors/icons
- ◻ Actions (idle, walk, run, jump, wave, talk, track), jump height, run lock, WASD roam
- ◻ Pose controls: head turn and tilt, eye blink and gaze, mouth opening and smile, arm spread, elbow and knee
  bend, ear tilt and spread, reset
- ◻ Parts: show/hide, solo, show all, mark reviewed, joint anchors, labels, separation preview
- ❓ 3D: mesh detail, depth, view blending, surface relief and bump; 4-direction mesh (`mesh.build_4dir`)

**Exports and checks**
- ◻ Exports menu: layered motion video, cutout MP4, EffectCraft (.jsx, run script), FilmCraft, Blender 5.3,
  2D layers + 6-action track, agent rig prompt, Farm School lineup
- ◻ Validate motion clip, vision analysis, Doctor (`app.doctor`)

**Tools and MCP (last step)**
- ✅ `--control` bridge in the shell (same protocol)
- ◻ `animationcraft-cli` help, MCP instructions, `ui_look`/`app_quit` errors and AGENTS.md/README/skills:
  point to `animationcraft-shell --control <port>` and remove references to the older app
- ◻ After the user confirms: delete `crates/ui-egui` and `apps/animationcraft`, plus their workspace entries and references

## The agent inside, and cua-style integration (core product, not a side panel)

The user wants to spend their time designing with an agent: timing, matching song and visuals, polish.
The agent drawer is where that happens. Every feature must work for the agent too, and whatever the agent
does must be visible to the user.

- **Cua-style, from the inside.** Cua Driver (github.com/trycua/cua) gives agents background computer use
  over MCP without taking the user's mouse. The shell owns its UI, so `--control` already does this natively:
  `ui.look` (picture, state, ids), `ui.click`/`ui.set`/`ui.key`/`ui.elements`, plus every engine command on the
  live session. Screen-scraping and global input on Wayland are the fallback we don't need.
- **Next improvements, in order:** (1) the inner opencode session gets the shell's MCP permanently
  (`animationcraft-cli --bridge <port> mcp`; propose the `opencode.json` change to the user); (2) the agent's
  actions are visible (agent cursor/highlight, action trail, undo); (3) pointing works both ways, with drawing and
  comments on the stage or storyboard sent as notes, like HyperFrames Studio desktop, and the agent can point back;
  (4) shared context goes with every message (view, selection, time on the timeline), so "make this blink
  later" resolves; (5) permissions and questions stay as inline cards, long jobs show progress with a status light;
  (6) markdown in replies is rendered with a vendored library.
- **Verify with a real task:** ask the inner agent to do something (e.g. hide the bee's blink lids) and watch it
  through `ui.look`.

## Current next slice

Animation layout approved. Shell has the bee stage, engine track playback/scrub and layer visibility.
Pen path + grid + start/end scale and colored sliders are live in `8dbf449`.
Inner validator checked endpoints, scale/exporter parity and byte-exact undo on a copy.
Visual evidence: Bumble Bee `.agent/looks/look-1792566893867.png`. Reference app 19898 closed after comparison. Remaining controls are not implied complete by section headings. Keep the user's
drawn path, slider values and composer draft across any restart. User now prioritizes 2D; defer 3D
techniques. The in-app agent validates operations and CLI configuration; coordinator owns UI changes.

## Nelly import and native layout checkpoint (2026-10-21)

- Nelly live project: `/run/media/jesse/Speedy/media/merriweather/nelly/Nelly.acproj`.
  Imported the existing full HyperFrames composition with dereferenced local media from
  `merriweather-videos/projects/kathy-cd-track-01/hyperframes`; server is managed on 3058.
  The registered front reference image is connected and visible in Animation (one image, not yet an articulated rig).
- User removed Split as a top-level concept: Project / Animation / Studio. Linux Studio now uses a GTK
  overlay allocation under the bar, beside the docked agent, instead of vertically stacked webviews.
  `ui.look` composites both native webviews so agents see Studio too.
- Inner validator found the raw transcript is not the approved wording and an alternative transcription
  stops at 45.31s. Imported **artifacts/lyric-verification/lyrics.words.json** plus **assets/subtitles.srt**
  (32 approved cues) through the repaired `workshop.import_transcript` command. Never guess lyrics.
- Nelly's visible OpenCode session: `ses_ea2919b49ffe6gTnPUHWPTFyGS`; instructed to build the template
  and verify an editable PDF roundtrip on a copy, stopping at tool failures. Previous validator session
  `ses_ea35ffb58ffeW3tnC2Nm3bziln` told to stop Nelly writes.
- Image-to-animation audit: existing `asset.build_cutout_rig` calls `guided_cutout_rig_builder.py`.
  Current handler ignores params, writes outside song, runs rendering before returning. The script's
  joint discovery is silhouette/proportion heuristic; pose library is fixed biped data. No new cuts run
  on Nelly. User questioned previous defects; do not treat automated cuts as reviewed artwork.
  Existing rig-to-spec converter is in the 2d-asset-to-animation skill's scripts (not yet integrated).
- OpenMontage has `bg_remove` (rembg), SVG rig/pose/timeline/render/review tools, and the
  `skills/creative/animated-drawing.md` route (Meta AnimatedDrawings, skeleton + mesh warp).
  Only skill found for AnimatedDrawings, executable/service installation not established.
  HyperFrames `remove-background --info` found cached u2net_human_seg, CPU only; this is background
  removal, not semantic part separation. Three.js SkinnedMesh supplies deformation/playback, not image segmentation.
