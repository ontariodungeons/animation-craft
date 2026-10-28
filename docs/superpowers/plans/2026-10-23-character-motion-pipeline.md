# Character Motion Pipeline: steps, tools, commands

> **For agentic workers:** every step is an `mc exec` command, scripted and reusable for any character. No hand-made output, no per-character code (see `CLAUDE.md`, memory "fix the pipeline, not outputs"). Run the tests that match the change and report the real result.

**Goal:** a split-layer character (the Hey Ho Little Fish cast) moves the same way in the Studio, in `mc`, in exports and in the film, with bones that bend body and tail, built by commands that work on the next fish without hand edits.

**Evidence (2026-10-23, little_fish_layered, read-only on the production project, renders in scratch):**

- `mc exec export.layered_motion spec=…/little-fish/parts/rig.json tracks=swim-cruise` renders a correct paper-puppet swim: tail −10°…+21° on its rivet, pectoral fin −10°…+17°, body bob ±7 px.
- The Studio shows a different animation. `motion.clips.convert` copied each clip's motion into Theatre keyframes; the rig was reworked afterwards; `ui/theatre.js:122` draws Theatre's stale copy *instead of* the engine motion for every keyed part (tail ±7.3°, eye ±0.4°), so parts nearly freeze and the body slides as one block. Exports and `mc` ignore Theatre (`motion_authoring.rs:562` "unsupported by the shared sampler"). Same stale copies exist for every converted character (14 fish, weeds, coral).
- The fabric scales, flat body base and pearl fibres were made by hand (`parts/fabric-scales-v2/*.pcraft` + `placement-recipe.json`); no command produced them.
- Four tools were installed today (Blender 5.2, Kdenlive/MLT, Synfig 1.4.5, OpenToonz 1.8). Only Blender touched the fish, and its worker (`blender-jobs/*/worker.py`) loads parts as flat planes and bakes the same rigid rotations: no armature, no deformation, and it used the older 6-part rig. `pipeline/blender53_bridge.py:532` builds a fixed humanoid armature for every character. Synfig/OpenToonz only render scenes authored by hand (`docs/workflows/animation-source-links.md`: rig → native project "not provided").

## The process (Jesse, 2026-10-23): one image → turnaround → poses → limbs → animate

This is the classic cut-out animator's order, and every step is an `mc` command. With all three sheets, a character animates with rigid limbs on pegs **and** soft parts that bend (antennae, eyebrows, mouths, fin rays). Rejected: "image generation with more steps" models (See-through, Qwen-Image-Layered, PixelART, LayerEditor) and paid services; image generation is used only to draw the sheets, which are then cut and checked mechanically.

The little fish never went through this as one character: its turnaround became a separate whole-view asset (`little_fish_volume_turn`), its `poses/` folder is empty, and its parts were cut from the family lineup picture, so nothing is drawn complete under a joint and nothing is marked bendable.

| # | Sheet / step | What it holds | Command today | Gap |
|---|---|---|---|---|
| S1 | **Turnaround sheet** from the one image | front, ¾, side, back on one canvas | `assets.agent_sheets kinds=[turnaround]` → `intake.validate slot=turnaround_sheet` → `asset.cut_sheet kind=turnaround` → `turnaround.optimize_accuracy` | must feed the same character as S2/S3, not a separate asset |
| S2 | **Pose sheet** | key poses of the character's actions | `assets.agent_sheets kinds=[poses]` → `asset.cut_sheet kind=action` → `image.register_frame` onto the turnaround canvas | none known; the fish never ran it |
| S3 | **Limbs sheet** | every part drawn **complete, extended under its joint, round at the peg**, laid apart on a flat background, fixed part names; soft parts drawn straight | `asset.cut_sheet kind=exploded` cuts by alpha islands | **new `limbs` kind** for `assets.agent_sheets`; part naming + peg marks |
| S4 | **Rig** | pegs at joint centres, real hierarchy (body → tail → tail fin), each part `rigid` or `bend` | `asset.build_cutout_rig` | **hierarchy is flat; no `bend` parts** |
| S5 | **Animate** | keys from the pose sheet, breakdowns, timing, **overlapping action** (each joint lags its parent), bend channels on soft parts, replacement drawings (blink, mouth shapes) | engine tracks, pose library | **no bend channel; no lag offsets; no replacement slot** |
| S6 | **Playblast** | the Studio preview equals the export | `export.layered_motion`, `validate.motion` | **Task A** (stale Theatre copy) + Task D |

**Where bending lives.** A bend must show the same in the Studio canvas, `mc`, and exports, so it goes into the engine's motion and the layered-motion renderer (a part marked `bend` becomes a short bone chain along its long axis, drawn as a deformed mesh), and the Studio canvas draws the same mesh. Blender stays a renderer that reads the same data; it is not the only place a bend exists (that would be a second truth again, the bug in Task A).

**Run order:** Task A → S3 `limbs` kind + S4 hierarchy/`bend` → S5 bend + lag channels → second fish end to end (S1→S6) through the CLI only → the little fish rebuilt the same way. Tasks B (Blender bones) and C (fabric-scale recipe) follow only if S1–S6 leave a gap.

## Which tool for bones (compared 2026-10-23, by asking each installed tool)

Rule from Jesse: use whatever can be scripted end to end without anyone opening it and looking; not all four tools are needed.

| Tool | Bones by script | Bends the art | Headless render | Wired here | Use for |
|---|---|---|---|---|---|
| Blender 5.2 (`blender` → 5.3 beta) | yes: Python API builds armature, weights, keys | yes: armature + automatic weights on subdivided planes | yes | yes: worker, limits, receipts, accept | **character bones (Task B)** |
| Synfig 1.4.5 | partly: no API, scene XML written by hand | yes: `skeleton`, `skeleton_deformation` layers | yes | render only | fallback; deformation grid needs tuning by eye |
| OpenToonz 1.8 | no: skeletons are built in the GUI; headless is `tcomposer` render only | GUI only | yes | render only | hand-authored scenes only |
| Kdenlive / MLT | no (0 warp/skeleton filters in `melt`) | no | yes | yes (film cuts) | film assembly, not character motion |

## The map

| # | Step | Tool | Command today | Status |
|---|---|---|---|---|
| 1 | Split art into parts (fins, tail, eye, body, scale rows) | PhotoCraft | `asset.cut_sheet`, segmentation checks (silhouette IoU 0.98 for the little fish) | works |
| 2 | Surface detail on a part (fabric scales, fibres) | PhotoCraft | none | **gap: hand-made** |
| 3 | Rig: pivots on rivets, parents, layer order | Magic Studio | `asset.build_cutout_rig`, `rig.json` | works (flat: every part under `body`) |
| 4 | Author motion (paths, part channels, actions) | Magic Studio engine | `motion.path`, `motion.part.set`, `motion.action.set` | works; **the one source of motion** |
| 5 | Retime by hand on a timeline | Theatre.js in the Studio | `motion.theatre.read/save`, `motion.clips.convert` | **broken: stale copy replaces engine motion** |
| 6 | Bones that bend body and tail | Blender armature (or Synfig skeleton deformation) | none | **gap: no rig → native scene** |
| 7 | Render | engine / Blender worker | `export.layered_motion`, `blender.start`, `animation.source.render` | works |
| 8 | Check the clip | ffmpeg + engine | `validate.motion` | works; **gap: no check that the Studio preview equals the export** |
| 9 | Accept and place in the film | Magic Studio, Kdenlive/MLT | `animation.source.accept`, `video.place_track` | works |

## Tasks

### Task A: one motion (fixes "eyes and body move as one block")

- Theatre stores only edits the user made, as **offsets added to the engine motion**, never a copy. `ui/theatre.js` adds the offset instead of replacing the engine rotation.
- The engine sampler applies the same offsets, so `mc`, exports and the Studio agree; the "unsupported by the shared sampler" refusal goes away.
- `motion.clips.convert` creates empty sheets (no sampled copy). A command `motion.theatre.reset_copies` drops keyframe tracks that are a sampled copy (generated by convert, never edited: same key count and spacing as the sampler); it reports what it dropped and is undoable through `motion.edit.undo`.
- Tests: Studio frame = sampler frame for a character with and without offsets; a converted-then-reworked rig no longer freezes.

### Task B: rig → bones (`animation.source.from_rig`)

`mc exec animation.source.from_rig character=<id> track=<clip> backend=blender` writes a native scene from `rig.json`:

- each part as an image plane in rig layer order;
- an armature with one bone per part: head at the part pivot, parent from the rig, length to the child pivot (or the part's far edge for leaves);
- body and tail planes subdivided and weighted to their bones (automatic weights), so a bone rotation bends the felt instead of turning a card;
- the engine clip baked as bone rotations (starts identical to today's swim);
- registered as the character's native source (`animation.source.set`), rendered by the existing worker, gated by `validate.motion`, accepted by `animation.source.accept`.

Blender is the backend: it is the only installed tool that builds bones and weights by script with no inspection, and it is already wired (worker, limits, receipts, accept). Synfig is the fallback; OpenToonz stays for hand-authored scenes; Kdenlive/MLT stays for the film.

Replaces the fixed humanoid in `pipeline/blender53_bridge.py:532` for cutout rigs.

### Task C: surface detail as a command (`part.surface_from_recipe`)

`mc exec part.surface_from_recipe asset=<id> part=<part> recipe=<placement-recipe.json>` rebuilds a part's surface layers (scale rows, fibres) from a PhotoCraft master and a placement recipe through `photocraft-cli`, writes them as rig parts, and records the recipe in `rig.json`. The little fish's `fabric-scales-v2` becomes the first recipe; the same recipe format then works on any fish.

### Task D: the Studio preview equals the export

`validate.motion` gains `compare=studio`: it samples N frames from the running Studio (bridge) and from the exporter for the same clip and fails when they differ beyond a pixel threshold. This is what would have caught Task A's bug.

### Task E: second fish, CLI only

On `older_fish_layered` (same kind of stale Theatre copies, no hand edits): `motion.tracks` → `motion.theatre.reset_copies` → `animation.source.from_rig` → `blender.start` → `validate.motion compare=studio` → `animation.source.accept`. Success = the second fish needs no command that the first fish did not, and the Studio, `mc` and the export show the same bending swim. Screenshots of both fish for Jesse.

## Order

A first (it is what makes the fish look wrong today), then B, then D, then E; C can run alongside B. Production data (`Hey Ho Little Fish.magic`, Theatre state) changes only through these commands, after Jesse's go, with nothing unsaved in the open Studio.
