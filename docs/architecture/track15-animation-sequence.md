# Track 15 reusable animation sequence — implementation plan

> Execution: implement in this session after the plan is reviewed, using the executing-plans skill. This document plans work; it does not claim the animation or UI passes acceptance.

**Goal:** finish Three Times Around / The Alley Alley Oh through repeatable `mc` commands, and compare the same animation methods on a humanoid, insect, animal and prop.

**Architecture:** retain the engine command registry, existing motion data, Craft adapters and approved panel architecture. The CLI and UI invoke the same operations and read the same saved values. Select animation methods from measured results across the subject set; one successful bee or captain is insufficient.

**Tech stack:** Rust engine and `mc`, Tauri/vanilla JS, existing Python pipeline, PhotoCraft, EffectCraft, Blender and the existing HyperFrames song project.

**Spec:** Jesse's recovered workflow in [track15-recovered-workflow.md](track15-recovered-workflow.md), the current request, [panel contract](panel-contract.html), [binding architecture](implementation-plan.md), and the song's `intent.md`. The old agent assignments in the diagram are historical labels, not ownership instructions.

## Constraints

- No invented UI, additional timeline or independent animation evaluator. Use the working Animation left inspector as the visual reference and the architecture diagram as the structural reference.
- Any touched panel or horizontal rail joins the shared configured header/body/status, state, selection and CLI inspection contract. Remove its replaced implementation rather than adding another styling layer.
- Apply Ponytail review to connected code and the rendered UI. Record changed-file line and selector counts; include new code/config in totals. Do not raise lint baselines or compress formatting to claim a reduction.
- Preserve Rizzo's checkout, bridge 19971, project, original artwork, receipts, authored tracks and lyric design. Use an isolated project copy and a task-owned preview during implementation.
- Keep media in the selected media project and planning/evidence in `docs/architecture/`. Explicit project-local output paths are required; legacy default `exports/` paths can land in the source repo.
- Track 15's latest intent requires shallow-water staging: sandbar, lagoon and tide pool. Do not restore the removed underwater/deep-sea backgrounds. Keep artist lyrics unchanged.
- No subject names or guessed anatomy in algorithms. Part IDs, hierarchy, pivots, rigid/bending behavior, expressions and limits are data. Split only the parts needed for the requested action.
- No automatic redraw of existing successful assets, silent source/subject fallback, or repeated registration of an already registered generated image.
- Tests and compile checks follow AGENTS.md: `cargo check --workspace` during code work; run the Rust tests that match the change and report the real result. This documentation-only review did not run those checks.

## What Rizzo actually left

Reviewed 2026-10-24 against main `92a42388`, the `song-lyrics-reader` command catalog/source, the Track 15 project and Rizzo's transcript. Do not assume installed `mc`, current main and Rizzo's binary have identical commands.

| Item | Evidence and status | Next action |
|---|---|---|
| Audio, lyrics, storyboard, target | Present. Seven storyboard spans cover 0–141.8 s; the older artifact scene plan instead has 25 line-level spans. | Read active storyboard mapping, not the older count, when placing clips. |
| Ship | Three full-canvas layers: flag, rigging, hull. Seven WebMs exist; five ship placements and four background placements are in HyperFrames. | Recheck pixels, positioning and joins. Rizzo reported scenes 2/4 high above the water. Existing files are not fresh validation. |
| Captain | Five parts; saved `rest` action moves only `body`. Current `mc ... motion.tracks character=captain_rig` confirms head and arms static. Generated `@wave` rotates body, not an arm. | Repair the parts/control contract before calling this a wave or finger wag. No face layers/control bindings are present. |
| Captain joins | `placement.json` reports exact rest reconstruction but `arm_raised` and head `joint_backing: 0.0`; raised arm has no hidden fill. | Verify shoulder/neck at motion extremes. Rest silhouette agreement cannot establish articulated quality. |
| Sea friends | A seven-part sheet exists, but no registered sea-friends rig or exported friend clips were found. | Validate and reuse usable drawings; rig each fish and crab with separate stable identities. |
| Sheet workflow | Rizzo's `8eed736b` adds `image.generate rig_parts` and `rig.from_sheet`; he explicitly marked it provisional after discovering the existing skeleton/limbs contract. | Merge the useful registration/compositing half into one sheet contract. Do not adopt both templates as separate workflows. |
| Placement helper | `motion.place_section` calls path → export → validation → placement. It exists in Rizzo's binary, not this main catalog. | Review and integrate it before relying on it. It currently assumes `outputs[0]` and only adds provenance where its caller supplies it. |
| Inspection gap | Rizzo used the HyperFrames snapshot CLI directly; no `mc` video snapshot/render command appears in either inspected catalog. | Add a thin engine adapter for the installed HyperFrames CLI, with real output verification. |

Sources: Rizzo transcript `03fc8a53-4d50-42c7-b40d-ba8ef42209a5.jsonl`, especially lines 5000, 5436, 6255 and 6466; `assets/characters/captain_rig/rig/placement.json`; `hyperframes/assets/animation/*.json`. Absolute locations are in the recovered workflow document.

## The sequence and scripts underneath it

Run from the implementation checkout so engine adapters find that checkout's `pipeline/`. Use an explicit project on each invocation, or a single `mc run recipe.jsonl` session starting with `project.open`. A live invocation must explicitly select the task's own bridge.

| Order | CLI operation | Implementation / underlying script | Availability or required work |
|---|---|---|---|
| 0 | `mc commands`, `mc doctor --health`; `project.open`, `project.list_assets`, `storyboard.read`, `motion.tracks` | `apps/mc/src/`, `project.rs`, `storyboard.rs`, `motion.rs` | Read actual binary help before execution; discover tools without stopping other sessions. Snapshot input identities and output paths. |
| 1 | `project.add_asset` only for new references; `project.load_asset` for cutting; `assets.agent_sheets ... run=false` | `sheets.rs`, `.agents/skills/asset-sheets/SKILL.md` | Prepare turnaround → poses → skeleton/complete limbs from one subject and a data-defined part list. Existing contract is biped-oriented; generalize that contract before generating non-bipeds. |
| 2 | `assets.agent_sheets ... run=true`, or native `image.generate reference_asset=...` for each sheet | `sheets.rs`, `image_generation.rs`, existing sheet template | Both drawing routes must consume the same contract/layout and retain native receipts. `image.generate rig_parts` is branch-only and provisional. Use Gaffer as the app's existing role. |
| 3 | `image.image_size`, `intake.validate`, `asset.cut_sheet kind=turnaround|action|exploded layout=... out_dir=...`, `turnaround.optimize_accuracy`, `image.register_frame` | `sheets.rs`, `photocraft.rs` → `studio_pipeline_cli.py` → `pipeline/pose_and_part_cutter.py`; PhotoCraft prepass/matting | Normalize from measured dimensions; preserve layout/crop transforms, names and missing parts. The current cutter cleans cells, so whole-sheet-first cleaning needs to be reconciled here, not added as a second matte implementation. |
| 4 | `rig.from_sheet sheet=... name=...`; `project.add_asset ... kind=cutout_rig if_not_exists=true`; `motion.tracks` | Rizzo's `image_cmds.rs` → `pipeline/rig_from_sheet.py` → existing cut/rig writer | Branch-only. Extend to common sheet metadata, joint markers, limits and explicit expression parts. Today it requires the adjacent generation receipt, cuts internally, and is not a generic consumer of arbitrary previously cut manifests. Refactor it to reuse Step 3 outputs without cutting twice. |
| 5 | `motion.action.set`, `motion.part.set`, `motion.layers.set`, `motion.path`, `motion.sample`, `motion.edit.undo` | `motion_authoring.rs`, `motion.rs` | Existing static part edits, paths and action duration/loop. Missing general term authoring and binding authoring must be exposed through the engine (see controls below). |
| 6 | `export.layered_motion spec=... tracks=... stills=6 video=false`, then `motion.export_track` / `export.layered_motion ... video=true` | `exports.rs`, `motion.rs` → `pipeline/layered_motion.py` → editable EffectCraft project, frames, ffmpeg WebM | Existing primary cutout route. Preview stills before full rendering. Read the returned output paths rather than selecting the newest file on disk. |
| 7 | Optional scripted Blender comparison: `animation.source.from_rig` → `animation.source.set/render` → `animation.status` → `animation.source.accept` | Proposed rig importer; existing `animation_links.rs`, `blender.rs`, `pipeline/blender_animation_worker.py` | `animation.source.from_rig` is proposed, not available. Existing fixed-humanoid bridge is unsuitable for ship/crab/bee. Adapt the existing worker/import path from explicit parts and bindings; do not resurrect a second live renderer or silently use a stock character. |
| 8 | `validate.motion video=... fps=30 loop=... reference=... spec=...`; `motion.sample`; `ui.look` | `pipeline/motion_validate.py`, existing sampler and shell bridge | Validate decoded alpha/frames/timing plus actual action, joins, expression swaps and UI/export agreement. Keep automated checks separate from visual findings. |
| 9 | Reviewed `motion.place_section`, or explicit `motion.path` → `motion.export_track` → `validate.motion` → `video.place_track` | Rizzo's `motion.rs`; `video_target.rs` → `pipeline/place_hyperframes_track.py` | Use the explicit chain for comparisons so candidates stay separate. Production placement uses only the selected verified result, original song timing, background-before-character stack and source provenance. |
| 10 | Proposed `video.snapshot` / `video.render` adapters; existing `words.*` only where lyrics must be connected | Existing HyperFrames CLI through `video_target.rs`; existing words commands | Verify composites at action/scene boundaries before full-song export. Replace duplicate karaoke only when the existing designed lyric track is connected; do not redesign the lyrics UI. |

`asset.build_cutout_rig` is not the default for this job: its existing guessed anatomy/output-location behavior must not turn a ship into a humanoid. `image.break_apart` remains useful for precise existing-art extraction; it cannot invent hidden joint artwork. These are distinct jobs, not competing names for one operation.

### Concrete current commands

These are existing signatures, not a claim that the pending preparation is done:

```bash
MC=/absolute/implementation-checkout/target/debug/mc
P='/absolute/review-copy/Three Times Around.magic'

"$MC" "$P" motion.tracks character=captain_rig
"$MC" "$P" assets.agent_sheets character=captain kinds='["turnaround","poses","skeleton"]' run=false
"$MC" "$P" export.layered_motion spec=/absolute/review-copy/rig.json out_dir=/absolute/review-copy/comparison/effectcraft stills=6 video=false
"$MC" "$P" validate.motion video=/absolute/review-copy/comparison/effectcraft/wag.webm fps=30 loop=true
```

`character` and part identities must be read back from preparation receipts; the examples do not imply the current five-part captain is sufficient. Guarded edits also require `expected_project` and the current `expected_rig_sha256` from `motion.tracks`.

### The eventual short interface

Only after the steps work on all four classes, wrap them as the Grok plan proposed. These commands do **not** exist today:

1. `mc animate prepare <image> project=<file> kind=<humanoid|insect|animal|prop|custom> [-y]`: inspect source, collect/reuse parts/actions, prepare the shared sheet contract, print exact next drawing command. `-y` accepts the known wizard defaults; it cannot manufacture missing anatomy or accept failed checks.
2. Run the printed existing `assets.agent_sheets` or `image.generate` commands to draw sheets.
3. `mc animate cut project=<file> character=<id>`: normalize/clean/cut/register and wire the named parts, invoking Steps 3–4 once; report unresolved joints/parts.
4. `mc animate show project=<file> character=<id> track=<id>`: load the authored action in the existing Animation view, sample/render the same action and return output/check receipts. Placement remains an explicit existing command.

Implement as thin CLI dispatch over registered engine commands (`apps/mc/src/animate.rs` if a module is needed), not a parallel pipeline. Store recipe inputs and receipts under the project; resume successful stages without a redraw. Keep individual commands usable.

## Subject/method comparison

Use the actual Track 15 cast plus the existing bee as a non-song insect fixture. Do not add an insect to Track 15 to satisfy the test matrix.

| Class / subject | Short comparison action | Values that must affect the result |
|---|---|---|
| Humanoid / captain | Finger wag, head shake, blink; tide-pool dip as a separate action | Arm/hand pivot and range, head range, frequency/phase, eyelid replacement timing, dip distance |
| Animal / orange fish | Tail/fin swim and bob | Tail/fin range and phase, body bob, path/speed |
| Animal / blue fish | Same action family using its own parts | Same parameter roles without assuming identical IDs/anatomy |
| Animal / crab | Claw wave and sideways movement | Claw pivots/ranges, alternating phase, travel distance |
| Prop / ship | Three laps, flag motion, gentle settle | Path, turns, duration, scale, bob/tilt and flag amount |
| Insect / existing bee | Wing/antenna motion, blink/mouth replacement | Wing frequency/range, antenna phase/range, expression timing |

Compare three approaches where supported, using the same action specification, reference art, duration (initially 3 seconds), frame rate (30), framing and sampled times:

1. Engine layered cutout → EffectCraft: existing reproducible path; rigid pivots, expressions and opacity replacements. Good baseline; rotating a rigid appendage is not proof of bending.
2. Scripted Blender rig: data-defined hierarchy, bones/deformation only where marked bendable. Preserve the art/camera. The generic rig importer is a real missing step; a rendered fixed-humanoid substitution is a failure, not a result.
3. Registered pose/replacement animation: reuse pose-library drawings with common canvas/pivots and replacement timing. It is an animation technique, not a third independent renderer. Use the same existing layered export path; do not introduce a new canvas renderer.

For every cell record supported/blocked, exact commands, setup/repair time, manual corrections, render time, editable source, decoded checks and visual findings. Measure CPU/GPU/memory only if available; missing is unknown. Compare identity, joint continuity, expression readability, motion quality, edge quality, timing and editability separately. Historical 99.86% bee alpha overlap measures silhouette only, not overall quality or live playback.

Keep methods in separate output directories and show a contact sheet/short comparison per subject. Decide from the full matrix before expanding the chosen method to song-length clips. Unsupported methods remain explicit and block a claim of complete comparison.

## Controls and UI contract

Existing CLI coverage: action duration/loop, part position/scale/rotation/pivot, stacking, visibility/solo, path/size, sampling, export amount and undo. `motion.part.set` currently replaces a property with one key at time zero: it is **not** an oscillation/keyframe editor and can erase that property's previous motion.

Smallest missing authoring capability: extend that existing guarded editor with mutually exclusive `terms` input keyed by `position`, `rotation`, `scale`, `opacity`. It accepts the engine's existing `keys`, `ellipse`, `oscillate`, `follow` structures; validates supported property/kind, finite numbers, durations/times and referenced part IDs before any write; and uses the existing hash guard/history/undo. Legacy static edits retain their behavior. Bindings, parents and limits belong in the common rig contract and need a guarded update path for correcting an existing rig; the same command can carry explicit `binding`, `parent` and `rotation_limits` edits after capability validation. Do not create one command per anatomy or slider.

The UI shows only controls supported by that subject's declared data. Display degrees, pixels, seconds, Hz and percent consistently. CLI edit → readback → engine sample → UI → export must agree; unsupported controls give a reason, not a whole-body imitation of a wave. Saved Theatre edits must compose through the same evaluator or be explicitly blocked; never silently ignored/cleared.

UI work is limited to bringing touched controls onto the existing approved components. Reuse the left Animation inspector's section hierarchy, labels, tooltips, density and progressive disclosure. For right panels and horizontal rails remove duplicated tabs/headers/padding selectors as they migrate to configured shared components. Preserve resizing, collapsed state, keyboard actions, selection and stable automation IDs. No new timeline or screen design.

## Remaining UI work and bounded handoff

The replacement agent and division of work are not assigned yet. Grok is out of credits. Do not assume its availability or dispatch another agent before Jesse chooses the split.

| UI gap, from inspected source | Existing foundation | Smallest next work |
|---|---|---|
| Gaffer is a general prompt/candidate form, not the recovered prepare/draw/cut/rig sequence. | `image-prompt.js`, existing sheet/cut commands | Surface the current stage, next command and failure in the existing panel; no new screen. |
| Part editing supports constants, not adjustable oscillation, replacement timing or bending. | `motion-authoring.js`, `motion.part.set` | Wire the shared validated term/binding editor to existing inspector controls; show unsupported capabilities explicitly. |
| Captain wave moves the body; separate face controls have no authored parts/bindings. | `motion.tracks` readback and existing capability flags | Repair data/bindings before enabling a control; verify actual arm/face pixels. |
| Right Parts tabs and panel padding retain their own selectors. | `.segmented`, configured panel styles, working left inspector | Migrate touched tabs/headers/bodies, then delete duplicate selectors and measure net reduction. |
| Playback controls keep speed, amount and loop-preview values in frontend state. | `animation.js`, bridge automation and engine sample/export parameters | Inventory each control's units, read/set route and persistence; distinguish audition settings from saved action edits. |
| Timeline edits can block engine authoring/export and differ from sampled motion. | Existing Theatre guards | Preserve edits; resolve shared evaluation or expose the exact unsupported case. No replacement timeline. |
| Final composite snapshots/render require leaving mc. | Installed HyperFrames CLI and `video_target.rs` | Thin mc adapters with output validation. |

Measured at planning base: CSS 1,016 lines/741 selectors; JS 4,527 lines. Existing ui-lint passes its baseline, while its report still exceeds historical acceptance counts by 105 CSS lines, 68 selectors and 300 JS lines. A lint pass does not prove reduced complexity. Refactor acceptance requires lower combined touched UI/CSS/config totals and fewer duplicate selectors, with required engine additions reported separately.

Live inspection of bridge 19971 now returns connection refused. This pass did not restart it; current visual findings require a task-owned preview. Prior header clipping measurements are historical, not a fresh current-build result.

### Instructions for the comparison runner

1. Work on a review copy. Run `mc commands` and inspect the existing subject with `mc <project.magic> motion.tracks character=<id>`. Use returned part/track IDs, never guessed names.
2. For the assigned subject/action, render six baseline stills with `mc <project.magic> export.layered_motion spec=<absolute-spec> out_dir=<new-method-folder> tracks=<actual-track> stills=6 video=false`. If successful, export that track as video into the same folder and run `validate.motion` on its returned path.
3. Repeat the *same action* using registered replacement poses, then the existing Blender route where it actually supports that rig. Do not implement a new adapter during the comparison. If a command/source is missing, record the exact gap and exit status for the UI/engine owner.
4. Keep duration, framing, source art and sample times equal. Inspect motion extremes and the final frame. Record whether the requested part moves, joints stay covered, expressions change, and UI/export agree. A body bob is not an arm wave.
5. Return one row per attempted method: exact commands, output/receipt paths, elapsed time, manual interventions, checks, visual findings, editable source and blocker. Preserve all variants. Do not choose a winner from silhouette overlap alone.
6. Run the next subject without subject-specific script edits. Full acceptance includes captain, bee, both fish, crab and ship; the size of the initial cost-limited comparison is a decision for Jesse.

## Implementation tasks and evidence gates

### 1. Consolidate the existing interfaces and source evidence

**Files:** recovered workflow; `README.md`; scoped integration of Rizzo's `commands.rs`, `motion.rs`, `image_generation.rs`, `image_cmds.rs`, `pipeline/rig_from_sheet.py` and template; coordinate against `feature/lyrics-design-timeline` before any overlapping integration.

- [ ] Record branch/base, exact owned files and binary/script versions. Work from a project copy; do not restart Rizzo's app.
- [ ] Review commits `8eed736b` and the `motion.place_section` implementation. Bring only reviewed dependencies into the implementation branch; preserve lyrics work.
- [ ] Check invalid/missing export outputs, failed validation and nonzero tool exits before placement; remove the unchecked first-output assumption. Propagate scene/receipt/source provenance.
- [ ] Read back command catalogs and source resolution from the built CLI. Commit with normal hooks.

### 2. One complete sheets-to-rig path

**Files:** `sheets.rs`, `image_generation.rs`, `image_cmds.rs`, `photocraft.rs`, `pipeline/pose_and_part_cutter.py`, `pipeline/rig_from_sheet.py`, existing sheet template/skill.

- [ ] Unify the skeleton/complete-parts contract. Preserve source pixels, layout transforms, joint markers, hidden overlap, hierarchy, expression slots and limits. Handle the captain, insect, both fish, crab and prop from data.
- [ ] Reuse the existing cutter and compositing/registration logic; delete the competing template/prompt implementation. No hardcoded biped fallback.
- [ ] Verify decoded crops, alpha, part completeness and parent graph; check zero-backed joints under actual rotations. Missing/ambiguous markers stop only dependent motion, with a fixable report.
- [ ] Test exact shared names/coordinates through normalize → cut → rig; duplicate/missing parts, parent cycles, bad markers, oversized/non-square generated sheets, tool failure and retry without overwriting successful sources. Commit through hooks.

### 3. Adjustable acting with the same CLI and UI values

**Files:** `motion_authoring.rs`, `motion.rs`, `commands.rs`, `ui/animation.js`, `ui/motion-authoring.js`, `ui/components.json`, `ui/style.css`, existing motion/bridge tests.

- [ ] Extend guarded part editing as described above; expose binding/hierarchy/limit corrections through the same saved rig contract. Create named actions using those parts and pose/replacement drawings.
- [ ] Test a parameter at two values: actual sampled/displaced pixels and expression timing must change as requested. Test invalid/unsupported terms, stale hashes, last frame, zero/boundary values, undo and save/reopen.
- [ ] Refactor each touched panel/rail to shared architecture. Compare against working left Animation UI, then run Ponytail review over callers and rendering. Report net line/selector change including additions.
- [ ] Verify 700/941/1261 px, panel open/closed, selection sync, tooltips, keyboard and stable IDs, CLI readback and export agreement. Commit with render evidence and normal hooks.

### 4. Compare methods across all subjects

**Files:** existing Blender adapter/worker, `animation_links.rs`, `pipeline/layered_motion.py`, `pipeline/motion_validate.py`; generic project recipes and `docs/architecture/evidence/track15/` report.

- [ ] Render the existing layered and replacement techniques for every subject; keep original art and evaluated action inputs fixed.
- [ ] Replace fixed-humanoid migration only in the cutout import path with a generic rig importer exposed as `animation.source.from_rig`. Exercise rigid and bendable parts, missing tools, bad sources and bounded job failures. No new renderer.
- [ ] Render the same actions through Blender, inspect the editable sources, and populate the comparison matrix. Validate edges/joins, motion extremes, loop boundary and high-frequency aliasing; verify Studio/export agreement for supported controls.
- [ ] Choose methods from the measured complexity/quality tradeoff across the full set; record limitations per subject. Commit evidence, no generated media in source commits.

### 5. Finish Track 15 and expose the short recipe

**Files:** reviewed `motion.place_section`, `video_target.rs`, `pipeline/place_hyperframes_track.py`, `commands.rs`, `apps/mc/src/animate.rs` if needed, `README.md`; existing HyperFrames project through commands only.

- [ ] Add thin `video.snapshot` and `video.render` adapters to the installed official HyperFrames CLI. Validate new outputs and propagate errors; no network install, newest-file fallback or hand-patched composition.
- [ ] Generate selected methods' captain/fish/crab/ship actions for the actual scene spans below. Check all repeated lyrics/long holds, not just first occurrence.
- [ ] Place verified clips with source references, preserve background stacking and shared timing, connect existing designed lyrics and remove duplicate karaoke via the existing pipeline.
- [ ] Snapshot action boundaries and render the complete 141.8-second song. Decode video/audio; verify duration, frame size/rate, audible original track, lyric timing and no missing final frames. Inspect a rendered contact sheet and key acting frames.
- [ ] Add only the thin `mc animate` wrappers after the low-level path passes; wizard defaults/`-y`, receipt-driven next steps and resume are checked on all four classes. Commit through hooks. Prepare a scoped PR only when implementation is complete; no merge without instruction.

## Song placement schedule

| Scene | Start / duration, seconds | Work |
|---|---|---|
| 1 | 0 / 17.72 | Three laps and gentle sandbar settle; retain clear lyric sky. Existing placement is 17.7 s: reconcile the 0.02 s edge. |
| 2 | 17.72 / 15.70 | Ship sailing; correct waterline. |
| 3 | 33.42 / 14.90 | Captain lean/wave aboard ship; coordinate its parent travel rather than sliding independently. |
| 4 | 48.32 / 15.38 | Stop, finger wag and head shake. Existing ship placement is 15.4 s: check overlap at 63.7. |
| 5 | 63.70 / 14.80 | Shallow lagoon settle; fish and crab peek and react. |
| 6 | 78.50 / 58.24 | Repeated shallow tide-pool dips/pop-ups with captain and friends. |
| 7 | 136.74 / 5.06 | Instrumental ending, held/resolving animation; no sudden disappearance. |

The saved storyboard is authoritative; the table is the recovered current schedule and must be reread before execution. Do not promote a script's rounded durations over saved scene boundaries.

## Review focus

1. A convincing rest composite can hide unpainted joints; test every moving joint at both extremes.
2. A generated action can move the wrong part; compare declared bindings, sampled channels and rendered action, including unsupported cases.
3. Successful repeated commands can select stale outputs, duplicate registrations or cut twice; use receipt-correlated files and exact stage inputs.
4. UI, Theatre and exporter can disagree; compare the same saved track/settings/time after CLI edits and save/reopen.
5. Different kinds, scene/line-level maps and cropped/full canvases can silently change identities, coordinates or timing; verify source hashes, parent transforms and scene boundaries end to end.

## Verification status for this plan

Performed: source/catalog review, Rizzo/Grok transcript recovery, project/rig/placement inspection, and successful current-main `mc` captain track readback. No generation, motion editing, live-app restart, comparison render, source-code edit, full-video render or new UI acceptance was performed. The plan is intentionally the next reviewable result.
