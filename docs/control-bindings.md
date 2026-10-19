# Native animation control bindings

A cutout rig plan or layered-motion spec may declare `control_bindings`. Bindings
name actual parts, not assumed anatomy or file naming conventions. Loading rejects
unknown controls, missing part IDs, non-finite coordinates and missing landmark files.

```json
{
  "control_bindings": {
    "face": {"part": "surface_17", "landmarks": "face-landmarks.json"},
    "joints": [
      {"part": "surface_17", "control": "head_pitch", "rotation_deg": 1.0},
      {"part": "pupil_a", "control": "eye_gaze", "translation": [0.15, 0.0]}
    ],
    "pins": [
      {"part": "lid_a", "src": [8, 8]},
      {"part": "lid_a", "src": [28, 8]},
      {"part": "lid_a", "src": [18, 18], "control": "eye_blink", "delta": [0, 0.08]}
    ]
  }
}
```

These are illustrative coordinates; author pins on the actual artwork and inspect
the result. Source pins must lie on visible pixels; targets must stay within the
cropped texture. Fixed pins have no `control` and anchor the region. Use sufficient
anchors to hold surrounding artwork still. Eye gaze should bind to a separate pupil
layer or explicitly isolated region; moving a whole face is not a gaze implementation.
Blink requires authored eyelid geometry or pins; detected eye centres alone do not
supply eyelids.

Joint `rotation_deg` and `translation` and pin `delta` are gains per raw slider unit.
Missing gains default to zero. Rotation limits declared by each part are applied.
The accepted control names match native state:
`eye_blink`, `eye_gaze`, `smile_open`, `head_yaw`, `head_pitch`, `ear_splay`,
`ear_pitch`, `arm_spread`, `elbow_bend`, `knee_bend`.
A 2D joint rotation can implement tilt, but cannot invent an unseen turned face.

Face landmarks use the facial preview script's `rect` and five `landmarks` fields,
in cropped-texture pixel coordinates. Paths are relative to the rig plan. Omitting
`landmarks` requests PhotoCraft detection and requires exactly one detected face.
For older plans without a `control_bindings` field, one explicitly declared
`kind: "head"` preserves facial-preview support. Names and spatial positions alone
never establish a binding. Explicit empty bindings disable that compatibility path.

The native renderer runs per-part PhotoCraft jobs for changed pins and mouth
controls. Original textures remain unchanged. Mouth deformation uses the existing
bounded Liquify worker; eyes are controlled by bindings, not a whole-head rotation
or guessed eye-region shift. `ControlBindings::supports(control)` lets the UI report
which controls have a declared mapping. This is capability, not visual QA.

## Export contract

UI and live-bridge current-rig exports sample the same evaluator as the native
viewport. They snapshot selected track, speed, range, manual joint offsets,
visibility/solo, jump settings and ready deformation textures. Animated facial
frames use the same phase selection. Pending or failed deformations refuse export
rather than substituting the unmodified source. Preview the requested settings first.
Parts separation is an inspection aid and must be reset before export.

An explicitly supplied different layered spec is exported as supplied, without
applying the active viewport's state. The headless engine has no UI state; its
explicit spec commands retain their existing behavior. No export edits the source
rig or original image. Rendering uses EffectCraft; MP4 conversion uses ffmpeg.
