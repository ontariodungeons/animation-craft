# AnimationCraft

AnimationCraft's active desktop application is **Rust + egui/eframe**. It imports
artwork and reference sheets, previews available animation data, and delegates
image/media operations to installed Craft CLIs through the Rust engine and Python
pipeline. Inputs may be any supported image: no face, body type, number of limbs,
pose count, or sheet layout can be assumed.

**Start here for current development.** Older HTML/TypeScript files and dated
specifications are references, not the active native UI implementation or an
automatic task list. Read [AGENTS.md](AGENTS.md) before editing alongside other
agents.

## Where the running application lives

| Area | Entry points |
|---|---|
| Native app | `apps/animationcraft/src/main.rs` |
| UI state and dispatch | `crates/ui-egui/src/lib.rs` |
| Center views and playback | `crates/ui-egui/src/panels/center.rs` |
| Sheets, poses and parts browser | `crates/ui-egui/src/panels/pose_viewer.rs` |
| Asset sidebar / inspector | `crates/ui-egui/src/panels/left.rs`, `right.rs` |
| Headless CLI and MCP server | `apps/animationcraft-cli/src/main.rs` |
| Engine commands and asset data | `crates/engine/src/commands.rs`, `session.rs` |
| Pipeline entry point | `studio_pipeline_cli.py`, `pipeline/` |
| External CLI discovery | `pipeline/craft_cli.py`, Rust `resolve_craft_cli` |
| Older browser implementation | `public/index.html`, `src/`, `server.ts`, `package.json` |

Editing `src/animation/*.ts` or running the Bun bundle build does **not** update
the Rust viewport. Reuse useful behavior from the older frontend deliberately;
do not rebuild its asset-specific geometry as the default for arbitrary imports.
The older web mouth/eye mappings are in `public/index.html` and
`src/animation/proceduralAnimation.ts`; their existence does not mean the native
renderer is already connected to them.

## Build and run

From the repository/worktree root:

```bash
cargo check --workspace
cargo build --bin animationcraft --bin animationcraft-cli
./target/debug/animationcraft --control 19897
```

In another terminal, use the **same instance's port**:

```bash
./target/debug/animationcraft-cli --bridge 19897 exec ui.state
./target/debug/animationcraft-cli --bridge 19897 exec ui.screenshot output=/tmp/animationcraft-review.png
./target/debug/animationcraft-cli --bridge 19897 mcp
```

Without `--bridge`, the CLI/MCP uses a separate headless engine session; it does
not control or inherit the open GUI's state. A `run` script keeps one session
across its commands. Discover current contracts instead of relying on copied
parameter tables or hardcoded command counts:

```bash
./target/debug/animationcraft-cli help
./target/debug/animationcraft-cli commands
./target/debug/animationcraft-cli info
python3 studio_pipeline_cli.py cut-sheet --help
photocraft-cli commands --json
```

The working directory affects pipeline and output resolution. Launch each agent's
app from its own worktree and use separate output directories. One app process
per port is required for isolated UI state; several MCP clients targeting the
same port still share one window, selection, playback state and screenshot queue.
Do not click or restart another agent's app. Agree on port ownership in the
handoff rather than assuming the example port is free.

### Stopping the instance you started

A live window stays open until it is told to close. Save the project first, then
use the instance's own control channel; do not rely on a window-manager close
request (Hyprland 0.56 no longer accepts `hyprctl dispatch closewindow` or
`killactive`, and a dirty EffectCraft/PhotoCraft/VectorCraft window would cancel
the close to show its save prompt anyway):

```bash
./target/debug/animationcraft-cli --bridge 19897 exec project.save
./target/debug/animationcraft-cli --bridge 19897 exec app.quit
```

The MCP server exposes the same operation as the `app_quit` tool when it is
started with `--bridge`; headless MCP has no app process to quit and says so. The
reply is `{"closing": true}` and the control port becomes free once the process
exits. Wait for that before relaunching on the same port.

If the control channel is unreachable, terminate the exact process you started
(`$!` from its launch, or the PID owning the port), not a pattern:

```bash
kill "$pid"          # SIGTERM: normal shutdown
# after 5s, only if it is still running:
kill -9 "$pid"
```

Do not use `pkill -f`: the pattern can match the shell issuing it. Do not stop
another agent's instance or reuse its port. See
[`.agents/skills/craft-app-lifecycle/SKILL.md`](.agents/skills/craft-app-lifecycle/SKILL.md)
for the equivalent PhotoCraft, EffectCraft, VectorCraft and FilmCraft commands.

## Song workflow commands (one line each — for you and for agents)

Every command is the same in the app, `animationcraft-cli` and MCP. Open the project first, e.g.
`animationcraft-cli run -` with one JSON per line, starting `{"id":"project.open","params":{"path":"<song>/<Song>.acproj"}}`.
Each result includes `next` steps for an agent.

| Job | One line |
|---|---|
| New song project (audio referenced, not copied) | `animationcraft-cli exec project.new path=<projects>/<song>/<Song>.acproj audio=<song.wav>` |
| Transcribe with FilmCraft (also makes `<song>.fcproj`) | `animationcraft-cli exec audio.transcribe` |
| Use an existing transcript (checked first) | `animationcraft-cli exec workshop.import_transcript path=<song>.transcript.json` |
| Create the storyboard template | `animationcraft-cli exec storyboard.init` |
| Import scenes from an OpenMontage project | `animationcraft-cli exec storyboard.import_openmontage path=<old project>` |
| List chapters (scenes) | `animationcraft-cli exec storyboard.toc` |
| Read one scene | `animationcraft-cli exec storyboard.read scene=1` |
| Search lyrics, prompts, notes | `animationcraft-cli exec storyboard.search query=bell` |
| Edit a scene (lyric, prompt, panel, note, time, reference, packet section) | `animationcraft-cli exec storyboard.edit scene=1 field=note value="bell too high"` |
| Split a storyboard grid into panels (PhotoCraft) | `animationcraft-cli exec workshop.split_panels image=<grid.png> columns=3 rows=2 prompts=<prompt.md>` |
| Rebuild the storyboard PDF | `animationcraft-cli exec workshop.build_storyboard` |
| Approve a scene (user only) | `animationcraft-cli exec storyboard.approve scene=1` |
| Add one image to its destination (copied to `assets/characters/<name>/parts/` or `assets/backgrounds/<name>/`, source recorded in `provenance.json`) | `animationcraft-cli --bridge 19897 exec project.add_asset path=<image.png> character=bee load=true` |
| Make it transparent (PhotoCraft; writes `<name>_transparent.png` beside it) | `animationcraft-cli --bridge 19897 exec asset.make_transparent` |
| Split an image into subject + filled background (PhotoCraft; writes `<name>_subject.png`, `<name>_background.png` beside it) | `animationcraft-cli --bridge 19897 exec asset.extract_background asset=<id> rect=[x,y,w,h]` |
| Assess an asset: clean or recreate? | `animationcraft-cli exec assets.assess path=<layer.png>` |
| Add to the asset fix list | `animationcraft-cli exec assets.fixlist_add asset=<id> path=<png> decision=recreate problem="torn edge"` |
| Show the fix list | `animationcraft-cli exec assets.fixlist` |
| One-pass asset sheet brief for the image agent | `animationcraft-cli exec assets.request_sheet` |
| Place storyboard panels in FilmCraft at their lines | `animationcraft-cli exec video.place_references` |

## Current functional boundaries

- Original images, pose sheets, part sheets and turnarounds are distinct inputs.
  Missing references remain missing; never borrow another character's artwork.
- A pose reference sheet is not a timed walk/run cycle. Actual animation requires
  appropriate rig/sequence data and a renderer that consumes the controls.
- A command being registered or returning success does not establish correct
  rendering, segmentation, facial motion, topology or export quality.
- Keep `needs_review`, absent measurements and tool failures visible. Nonempty
  PNGs and matching frame counts are not visual acceptance criteria.
- Use installed selection, mask and deformation tools before implementing image
  processing algorithms. Preserve source files and review tool results; subject
  detection and global color removal can damage light artwork.
- PhotoCraft facial previews deform existing pixels; procedural mouth motion is
  not audio-driven lip-sync. An input is not guaranteed to contain a detectable
  face. Inspect the connected runtime and its output before claiming support.
- Native mesh/export paths require their own inputs and verification. Do not
  treat the historical linear upload-to-GLB diagram as a verified universal flow.

## Validation and collaboration

Rust compilation catches type/ownership errors, not whether a control changes the
right artwork. Use meaningful, asset-independent checks for state transitions,
error propagation, preservation and temporal behavior. Generated fixtures must
actually exercise the behavior. Avoid named-character assumptions, exact UI-text
assertions, conditional-assertion false passes and tautological output checks.

Use live visual review for appearance and motion, reporting it separately from
portable behavior checks. Run relevant checks and the build; document inherited
failures honestly. Do not claim all tests pass based on this README—results depend
on the revision and environment. Do not reformat unrelated files to hide a dirty
baseline.

Work on isolated branches. Commit before handoff; submit a PR with the commit SHA,
owned files, command contracts, observed evidence and limitations. The coordinator
reviews and integrates PRs; contributors do not merge their own changes unless
explicitly directed. Current ownership lives in the active coordination messages,
not in historical plans.

## Historical design references

- [Rust migration proposal](RUST_CRAFT_REFACTOR_SPEC.md)
- [Earlier web/pipeline design](docs/superpowers/specs/2026-10-18-modular-2d-3d-blender-filmcraft-pipeline-design.md)
- [Cutter usage](docs/pose-and-part-cutter.md)

Dated plans retain useful context but are not permission to implement every listed
feature. The user's current task and reviewed contracts define scope.

## License

MIT OR Apache-2.0
