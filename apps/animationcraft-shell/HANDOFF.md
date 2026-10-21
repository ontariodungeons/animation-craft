# AnimationCraft shell — handoff (2026-10-21)

The Tauri shell in `apps/animationcraft-shell` **is AnimationCraft now**. The egui app
(`crates/ui-egui`, `apps/animationcraft`) is "the older UI": a reference only, tagged
`older-egui-ui`. Don't maintain or extend it. Port every feature into the shell in the shell's design.
The user deletes the older UI after they confirm the checklist below.

## Read these skills first (in this order)

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
- Always upgrade dependencies that are behind to their latest stable release, as part of the work.
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

- **Engine evaluates, page draws.** Add engine commands, e.g. `motion.tracks {character}` and
  `motion.sample {character, track, fps}`, which return per-frame layer transforms (position, rotation, scale,
  opacity, with pivots and parents applied). They must follow exactly the rules of `pipeline/layered_motion.py`
  (`keys_at`, `oscillate_at`, `envelope_at`, `sampled_term`, `channel`). Add a parity test against the
  exporter on the bee spec. The page plays the samples on a canvas with the layer PNGs. No third, hand-written
  evaluator in JS.
- The stage: the character in its **rest pose** (no blink lids or alternate states stacked), track picker,
  play/scrub, timing against the song.
- Clicking an object opens its **layer panel**: stack order, thumbnails, show/hide, states (eyes open/closed),
  per-layer fix actions.
- The bee: `/run/media/jesse/Speedy/media/merriweather/bumble-bee/Bumble Bee.acproj`. Its rig spec is
  `assets/characters/bee/bee_motion_spec.json`. The mouth is baked into `eyes-open.png`/`eyes-closed.png`.
  The kit in `merriweather-videos/remotion-composer/public/track06/layers/` has `eyes-open-no-mouth.png`,
  `mouth-smile.png` and `mouth-open.png`.

## Known issues seen in the shell

- A project with no storyboard shows a broken image. It needs an empty state with the next step.
- A layered object whose layers don't share one canvas raises an error toast on open (Hey Ho, `older-fish`).
  Show it quietly on the card instead.
- The Animate card stacks every layer, including the blink lids, so the bee gets a strange mouth and eyebrow.
  It needs the rest pose.
- Agent replies show raw markdown. Render it with a vendored library, not a hand-written parser.
- Hey Ho's `remotion/` has no installed packages (`npm error could not determine executable`).

## Feature checklist: older UI → shell (the user confirms this, then the older UI is deleted)

Legend: ✅ in the shell · ◻ to do · ⚙ becomes a CLI chain (the shell shows a step).

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
- ◻ Intake: validate, three-question, compare to reference
- ◻ Pose sheet viewer: slice kinds, sheet tools, OCR labels, overlays (anchors, crops, names), fit/100%
- ◻ Turnaround views (front/right/back/left), optimize accuracy

**Animation view**
- ◻ Stage with rest pose; zoom (fit, 100%, ±), center
- ◻ Actions (idle, walk, run, jump, wave, talk, track), jump height, run lock, WASD roam
- ◻ Pose controls: head turn and tilt, eye blink and gaze, mouth opening and smile, arm spread, elbow and knee
  bend, ear tilt and spread, reset
- ◻ Parts: show/hide, solo, show all, mark reviewed, joint anchors, labels, separation preview
- ◻ 3D: mesh detail, depth, view blending, surface relief and bump; 4-direction mesh (`mesh.build_4dir`)

**Exports and checks**
- ◻ Exports menu: layered motion video, cutout MP4, EffectCraft (.jsx, run script), FilmCraft, Blender 5.3,
  2D layers + 6-action track, agent rig prompt, Farm School lineup
- ◻ Validate motion clip, vision analysis, Doctor (`app.doctor`)

**Tools and MCP (last step)**
- ✅ `--control` bridge in the shell (same protocol)
- ◻ `animationcraft-cli` help, MCP instructions, `ui_look`/`app_quit` errors and AGENTS.md/README/skills:
  point to `animationcraft-shell --control <port>` and remove references to the older app
- ◻ After the user confirms: delete `crates/ui-egui` and `apps/animationcraft`, plus their workspace entries and references
