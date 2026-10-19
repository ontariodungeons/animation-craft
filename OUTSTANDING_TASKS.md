# AnimationCraft — Outstanding Functionality & CLI Integration Tasks

Status: **Draft for review** (do not begin implementation until reviewed)
Date: 2026-10-19
Owner: to be assigned
Related: `RUST_CRAFT_REFACTOR_SPEC.md`, `docs/superpowers/specs/2026-10-18-modular-2d-3d-blender-filmcraft-pipeline-design.md`

This document lists functionality and CLI/MCP integration that is **not yet implemented or not yet exposed**, discovered by auditing:

- the `animationcraft` engine command registry (`crates/engine/src/commands.rs`),
- the `animationcraft-cli` CLI surface (`apps/animationcraft-cli/src/main.rs`),
- the animationcraft MCP tool surface,
- the sibling `*Craft` CLIs installed locally (PhotoCraft, EffectCraft, FilmCraft, VectorCraft).

Legend: `[ ]` open · `[x]` done · **(quick win)** small/reversible · **(feature)** net-new · **(integration)** wires an existing external tool.

---

## 0. Current state (baseline)

### Engine — 31 registered commands
`project.list_assets`, `project.add_asset`, `project.remove_asset`, `project.update_asset`,
`project.load_asset`, `asset.make_transparent`, `asset.build_cutout_rig`, `image.break_apart`,
`image.puppet_warp`, `image.color_to_alpha`, `image.content_aware_fill`, `export.cutout_mp4`,
`intake.validate`, `intake.three_question`, `intake.compare_reference`, `part.show_all`,
`export.agent_prompt`, `asset.cut_sheet`, `asset.load_manifest`, `part.set_state`,
`turnaround.optimize_accuracy`, `anim.preview_2d`, `mesh.build_4dir`, `export.effectcraft`,
`effectcraft.run_script`, `export.filmcraft`, `export.blender`, `export.character_iteration`,
`scene.farm_school_lineup`, `vision.analyze`, `command.list`.

### MCP surface
- 31 tools, 1:1 with engine commands (id `.` → `_`).
- `resources/list` and `prompts/list` return empty arrays.
- `tools/call` always returns `isError: false`.
- `initialize` advertises only `{ "tools": {} }`.

### CLI subcommands
`info`, `commands [filter]`, `exec <id> [key=value ...]`, `vision [image|character]`,
`run <script.jsonl | ->`, `mcp`, `help`, `--version`.
Live UI control (only on `--bridge <port>`): `ui.state`, `ui.inspect`, `ui.click`,
`ui.vision`, `ui.screenshot`, `ui.set_view`.

### Local sibling CLIs available
- `photocraft-cli` (`~/.local/share/photocraft/0.9.0/`) — raster editor.
- `effectcraft-cli` (`~/.local/share/effectcraft/0.10.0/`) — motion graphics/compositor.
- `filmcraft-cli` (`~/.local/share/filmcraft/0.8.0/`) — video editor.
- `vectorcraft-cli` / `figcraft` / `cadcraft` — vector / design / CAD (adjacent).

---

## Phase 1 — Wire up features that already exist but are not exposed (quick wins)

- [ ] **T1. Register the 3 orphaned export commands. (quick win)**
  `cmd_export_threejs`, `cmd_export_react_motion`, `cmd_export_blender_script` exist in
  `crates/engine/src/commands.rs` but are unregistered (currently `#[allow(dead_code)]`).
  Register as `export.threejs`, `export.remotion`, `export.blender_script` and remove the
  `#[allow(dead_code)]` annotations. Derives from `src/exporters/*.ts` parity.

- [ ] **T2. `mesh.verify_parity` — topology parity check. (integration)**
  Wrap `pipeline/topology_parity.py` (verts / edges / loops = 3×tris / bones) and consume
  `exports/blender53_live_metrics.json`. Complements `compare_html_vs_blender.py`.

- [ ] **T3. Make `vision.analyze` reachable with a real schema.**
  Confirm it is invocable from MCP and CLI with `{"character"|"image"}` and returns the
  pixel/alpha/drift/lift metrics used by the GUI.

- [ ] **T4. `export.transparent_video`. (integration)**
  Wrap FilmCraft alpha export (`file.newTransparentVideo`, `file.exportMedia {alpha:true}`,
  WebM/EXR) for RGBA animation output.

---

## Phase 2 — Missing image & rig operations (parity with PhotoCraft)

- [ ] **T5. `image.crop` / `image.trim`. (integration)**
  Wrap `photocraft-cli` `image.crop` and `image.trim` — needed to trim sheets before slicing.

- [ ] **T6. `image.slice`. (integration)**
  Wrap PhotoCraft `slice.fromGuides` / `slice.divide` to slice sheets into sprite/part tiles.

- [ ] **T7. `image.content_aware_scale` / `image.content_aware_move`. (integration)**
  Wrap `edit.contentAwareScale` and `paint.contentAwareMove` to reposition limbs without
  distorting artwork.

- [ ] **T8. `image.combine_shapes`. (integration)**
  Wrap `layer.combineShapes.{unite,subtractFrontShape,intersectShapeAreas,excludeOverlappingShapes}`
  for boolean mask construction.

- [ ] **T9. `rig.set_joint` / `rig.edit_pivot`. (feature)**
  Add joint/pivot editing. Today there is only visibility/solo (`part.set_state`) and
  `asset.build_cutout_rig`; there is no way to correct an auto-placed pivot.

- [ ] **T10. `skin.strip_costume` / `skin.apply_costume`. (feature)**
  Port the TS decisions in `src/topology/skinning.ts` into Rust engine commands (base
  mannequin ⇄ costume swap), so the engine can toggle costume shells.

---

## Phase 3 — Animation rendering (largest functional gap)

- [ ] **T11. `anim.render_2d`. (feature — highest value)**
  Render a pose-cycle / walk-run-jump loop to a frame sequence or MP4 directly from the
  active `pose_manifest` + `motion_range`. Today `anim.preview_2d` only sets state; there
  is no renderer, so only `export.cutout_mp4` (rig-based) produces motion.

- [ ] **T12. `anim.retime`. (integration)**
  Wrap FilmCraft `clip.setTimeInterpolation` (frameSampling / frameBlending / opticalFlow)
  for smooth slow-motion of rendered clips.

- [ ] **T13. `anim.scene_detect`. (integration)**
  Wrap FilmCraft `clip.sceneEditDetection` to auto-split long pose sheets / rendered clips.

- [ ] **T14. `anim.render_turntable`. (feature)**
  Single-character 4-direction turntable render. A 6-character variant exists via
  `scene.farm_school_lineup`; expose the single-character case.

---

## Phase 4 — EffectCraft integration depth (real cutout animation)

- [ ] **T15. Generate real EffectCraft keyframed compositions. (integration — high value)**
  `export.effectcraft` currently emits a `.jsx` via `guided_cutout_rig_builder.py --jsx-only`.
  Extend it to emit genuine `effectcraft-cli` operations:
  - `layer.setParent` — bone/pivot hierarchy,
  - `prop.addKey` + `keys.set` — position/rotation keys,
  - `keys.interpolation` — bezier/continuous cycles,
  - `prop.setExpression` with `loopOut` — repeating walk/run cycles.
  Goal: a `.ecproj` that actually animates the cutout rig, not just a script stub.

- [ ] **T16. `effectcraft.inspect`. (integration)**
  Wrap `effectcraft-cli doc_inspect` / `get_project` so an agent can verify a generated comp.

- [ ] **T17. `effectcraft.render_preview`. (integration)**
  Wrap `effectcraft-cli render-frame` for a quick comp preview PNG.

---

## Phase 5 — VectorCraft / SVG integration

- [ ] **T18. `export.vector`. (integration)**
  Export limb masks (`parts/*_limb_masks.svg`) to VectorCraft, or import them as editable
  vector paths for precise re-masking.

- [ ] **T19. `image.svg_to_mask`. (integration)**
  Convert an SVG mask into a raster alpha mask via `vectorcraft-cli` for exact cutouts.

---

## Phase 6 — MCP server completeness

- [ ] **T20. `isError: true` when a command returns `Ok({ok:false})`. (quick win — correctness)**
  Today `tools/call` always reports `isError: false`, so agents treat
  `image_break_apart` failures (empty `parts`, `ok:false`) as success.

- [ ] **T21. Expose MCP resources. (feature)**
  Implement `resources/list` + `resources/read` for `animationcraft://project`,
  `animationcraft://assets/{id}`, `animationcraft://commands`; advertise `resources` in
  `initialize` capabilities.

- [ ] **T22. Expose MCP prompts. (feature)**
  Implement `prompts/list` + `prompts/get` for the 3-question intake and the rigging-agent
  prompt (already produced by `export.agent_prompt`).

- [ ] **T23. Generate real `inputSchema` from `CommandSpec.params`. (feature)**
  Replace `{ "type":"object", "additionalProperties": true }` with per-command JSON schemas
  so agents get typed parameters.

- [ ] **T24. Add `ui_*` MCP tools for bridge mode. (feature)**
  Expose `ui_state`, `ui_inspect`, `ui_click`, `ui_screenshot`, `ui_set_view` as MCP tools
  so agents can drive the live GUI over MCP, not only the CLI `--bridge`.

---

## Phase 7 — CLI parity with `filmcraft-cli`

- [ ] **T25. `describe <id>`. (quick win)**
  Print one command as JSON (menu, shortcut, params, enabled).

- [ ] **T26. `inspect [project|asset]`. (feature)**
  Dump the project/asset tree as JSON (`filmcraft-cli inspect` equivalent).

- [ ] **T27. `probe <media>`. (feature)**
  Media/asset metadata as JSON.

- [ ] **T28. `bench`. (feature)**
  Startup/command timing for agents and performance tracking.

- [ ] **T29. Document `ui.*` control commands in `help`. (quick win)**
  They appear only in the bridge section; surface them as a first-class group.

---

## Phase 8 — Robustness & project hygiene

- [ ] **T30. Version the runtime fixture dependency.**
  `exports/templates/exploded_part_sheet_spec.json` (~4 KB) is required by the cutter but is
  untracked/gitignored. Relocate into a tracked `assets/` directory and commit deliberately
  (flagged in the UI-merge handoff as a real runtime dependency, not a test skip).

- [ ] **T31. Fix Python test false-passes (coordinate ownership).**
  `tests/test_pipeline_cli.py` places assertions inside `if local_asset_exists`, producing
  false passes on machines without the assets.

- [ ] **T32. `cargo fmt --check` clean-up. (chore)**
  The baseline is not fmt-clean (e.g. `apps/animationcraft/src/main.rs`). Reformats of
  files shared with concurrent agents must be coordinated.

- [ ] **T33. `project.purge` / cleanup. (feature)**
  Remove generated exports and stale rig plans to keep the working tree tidy.

---

## Suggested priority order

1. **T1, T20, T15** — register orphaned exports; fix MCP error signaling; deepen EffectCraft
   keyframe output. Highest value, lowest risk.
2. **T11, T5, T6** — animation rendering + core image ops.
3. **T21–T24** — MCP resources / prompts / UI tools (largest agent-UX gain).
4. **T2, T9, T10** — parity and rig correctness.
5. **T7, T8, T12–T14, T16–T19, T25–T29** — breadth.
6. **T30–T33** — hygiene.

---

## Validation expectations per task

Every task should land with:
- `cargo check --workspace`
- `cargo test --workspace` (add unit tests for the new command: registration + failure path)
- `cargo clippy --workspace --all-targets -- -D warnings`
- An updated MCP tool + CLI `describe`/`commands` entry where applicable
- One commit per task, on a branch, via PR (no force-push, no reset of shared HEAD)

## Known pre-existing issues (out of scope unless picked up)

- 3 dead export helpers in `commands.rs` (addressed by **T1**).
- Fixture-dependent engine tests require local `exports/pose_and_parts/*` and
  `exports/templates/*` (addressed by **T30**).
- `cargo fmt --check` not currently clean (addressed by **T32**).
