# Guarded animation authoring (#23)

These engine commands edit a saved project-local layered-motion rig. They use the
existing channel/key vocabulary and sampler; they do not evaluate Theatre tracks.
No project-specific scripts or UI files are part of this backend slice.

All rig mutations require `expected_project` (canonical selected `.magic`),
`character` (connected stable asset ID), and `expected_rig_sha256` from a fresh
`motion.tracks`. Part/action edits also require the stable authored `track` ID.
Standard `@...` actions are read-only. A per-rig nonblocking file lock, before-write
hash check and atomic replacement protect these commands against one another.
Other writers must still refresh/participate; this is not a filesystem transaction
with arbitrary external editors.

| Command | Additional parameters | Meaning |
| --- | --- | --- |
| `motion.part.set` | `part`, optional `position:[dx,dy]`, `scale:[x,y]`, `rotation`, `pivot:[x,y]` | Replace each supplied action channel with a constant key at0. Other channels remain untouched. Pivot edits the layer geometry for all actions. |
| `motion.part.reset` | `part`, `fields:["position","scale","rotation","pivot"]` (explicit subset) | Remove exactly the listed action channels, returning identity defaults; pivot restores the value captured before its first authoring edit. |
| `motion.action.set` | optional `label`, `duration`, `loop` | Rename the display label, preserving track ID; change duration/loop. Does not rescale keys or oscillators. Reject duration shorter than existing keys. |
| `motion.edit.undo` | `undo` returned token | Restore exact previous rig bytes; token binds canonical project/rig/character and requires the current after-hash. Undo itself returns another receipt, allowing redo via the same command. |

Position uses canvas-pixel offsets (±65536). Scale is percent (1–1000), with100 as
identity. Rotation is degrees (±3600). Pivot uses canvas coordinates (±65536).
Duration is0.1–600 seconds; labels are nonempty and at most120 UTF-8 bytes; loop is
boolean. The constant replacement semantics are explicit: this is not a keyframe
insert at the UI playhead, nor an additive offset on top of existing animation.
Reset does not restore an old animation channel; use undo for that.

Every successful mutation returns `rig_saved:true`, `project_saved:false`, new
`rig_sha256`, `undo`, `undo_receipt`, `character`, and `spec`. Rig edits are already
persistent; the shared `.magic` is not rewritten. Original bytes live in
`.motion-authoring/<canonical-rig-path-digest>/<token>.json` beside the rig.

`motion.tracks` returns existing rig hash, part IDs/pivots/crop/source size,
action IDs/labels/duration/loop, plus:

- `layers[].base_geometry`: current pivot, crop, canvas;
- `tracks[].static_overrides[part]`: position/scale/rotation. Identity values mean
  an absent channel; null means an animated/nonconstant channel, never pretend0;
- `tracks[].authoring`: `part_set`, `part_reset`, `action_set`, `pose_export`,
  `reason`, scale/position/rotation units, replacement semantics, and
  `pose_requires_theatre_state`. Saved Theatre overrides disable these capabilities.

A pivot edit or rig-wide undo additionally refuses Theatre overrides on any action
of that character. Other action edits refuse relevant legacy/selected-clip Theatre
tracks or static overrides. Generated Theatre rotation seeds also count as saved
tracks: do not clear them silently to enable these commands. Current `.magic`
Theatre state must match the engine snapshot; a stale session is refused.

## Sampled static pose

`motion.pose.export` requires the same project/character/track/hash identity plus
`time` within the action, optional `rest` (default false), optional `amount`
(0–100, default100), and `theatre_state`: the UI's CURRENT native save-file
JSON, or null when it has no Theatre state. Optionally pass `reference_asset`, a
connected reference asset ID. A caller must capture current state at submission;
passing an old saved snapshot would conceal its own unsaved edits.

Native snapshots containing only recognized empty sheets normalize to null, so an
untouched mounted editor can export. Any track/object entry or nonempty override
(including a zero-valued override or generated seed) remains real state and is not
normalized away. Unknown structures are not treated as empty.

The command refuses a current/saved Theatre mismatch and relevant saved Theatre
edits. It never pretends the shared sampler evaluated Theatre. UI must retain its
unsaved-state gate and should explain the refusal. No Theatre conversion/clear or
project save occurs automatically.

The shared `motion.sample` now accepts optional `time` to return exactly one actual
evaluation. Without time it retains the existing full-range behavior. Pose export
passes the captured rest/amount directly to that sampler and records both in the
receipt. It uses those exact matrices, independent opacity, effective hierarchy and visibility,
then rasterizes the cropped layer PNGs onto the saved transparent canvas. Output
uses nearest-neighbor affine sampling (rough proof quality), is clipped to canvas,
and is bounded to4096px per axis,4M output pixels and64M layer-pixel operations.
It is a compositor, not a second animation evaluator.

The output is `assets/generated-poses/pose-<timestamp>/pose.png` plus `pose.json`.
It registers an image asset in the current session and returns `asset_id`, `path`,
`receipt`, `approval:"pending"`, `project_saved:false`, source character/track/time,
reference path/hash, rig/image hashes, exact sample, and layer readback. Registration
is in memory until the user's explicit project Save. The UI may create a pending
`storyboard.place_asset` draft from the asset; this command never approves or places
it, and never rewrites a storyboard PDF.

## Captured visibility targets

Existing `part.set_state` and `part.show_all` accept optional `character` plus
`expected_project` (required when character is supplied). Explicit targeting loads
that asset without changing the active selection, then edits its visibility/solo
state. Without character, legacy active-asset behavior remains. With only
expected_project, the active-asset operation is still project-guarded.

## Verification / limits

Generic integration tests exercise persisted transforms through the shared sampler,
actual exported PNG pixels and registration, stale-hash rejection, exact undo,
project guards, finite bounds, exact-field reset, stable action IDs, metadata,
Theatre capability refusal and selection-independent visibility. Initial missing
commands and current-selection visibility failures were observed before fixes.
Run the Rust tests that match the change and report the real result. No production project is a fixture.

This slice does not implement Theatre evaluation, per-time channel editing, negative
scale mirroring, arbitrary native-format authoring, subpixel-filtered final-quality
pose exports or automatic project saves. A loop toggle does not validate seam quality.

## Guarded planar stacking (#23 completion)

`motion.layers.set` takes common expected_project/character/expected_rig_sha256,
then exactly one of `order:[all part IDs, back to front]` or `part` plus `depth`.
Depth is a zero-based ordinal:0 is back, n-1 is front. IDs must form a complete
unique permutation; ties, unknown IDs and out-of-range/noninteger depths fail.
This reorders the saved layer array only. Transform parents and pivots do not change.
This is stacking depth, not physical Z or perspective; root paths/scales still own
spatial movement. Any saved Theatre edits for this character refuse the mutation.
Common hash/lock/undo receipts apply. Existing saved blends/clips remain frozen;
remigrate an edited rig to apply its new order to Blender.

`motion.tracks` adds top-level `layer_order` and `layer_authoring` (supported/reason,
semantics/back-to-front, hierarchy_changed:false, physical_z:false). Layer `order`
and `depth` both expose the zero-based ordinal. `motion.sample.layer_ids` explicitly
identifies its arrays' order. The loader, sampler, pose compositor and Blender's
existing positive-Z painter stack consume the same saved array; no hierarchy sort
or backend-specific order is introduced. Generic overlapping-color pose/undo checks
and one actual tiny Blender render verify front/back parity.

## Action export preview parity

`motion.export_track` now uses the same relevant saved-Theatre refusal guard as
pose export, before sampling or renderer/output work. Empty/missing track selection
checks the whole character conservatively. Unsupported saved Theatre edits are
never silently replaced with raw rig motion. The UI must require its current
Theatre changes to be saved before submission; this command cannot observe an
unsent browser draft. No Theatre evaluator or fallback is introduced.

Optional `rest` (default false) is documented in the registry. The entire request,
including rest and amount, still goes directly to the existing sampler. Rest bakes
its single sampled resting transform into a static-duration export; it does not
export the moving action. Successful export responses report actual sampler `rest`
and requested `amount`. No additional renderer run is needed for the refusal test.


### Export integration

Transparent action export preserves the sampler's effective parent IDs and layer
order, including actions which exclude a parent. It is registered as a
long-running command so Studio renders from a session snapshot without holding
the shared engine mutex throughout the external render.

Blender's legacy rig migration applies the same relevant saved-Theatre refusal
before reading the rig or submitting a worker. Saved `.blend` rerenders remain
independent of the current rig/Theatre state. Neither path silently converts
Theatre animation into shared-sampler keys.
