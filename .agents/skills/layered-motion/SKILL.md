---
name: layered-motion
description: Use when animating any set of transparent PNG layers along paths, oscillations, or keyframes and exporting transparent video tracks via EffectCraft, without assuming what the layers are (character, creature, prop, effect).
---

# Layered Motion

Drives `export.layered_motion` (`pipeline/layered_motion.py`): takes a JSON spec of transparent PNG layers, pivots and per-track motion, builds an EffectCraft project, and renders one transparent WebM per track. Nothing in the module assumes anatomy, part names or counts.

## Command

```bash
mc exec export.layered_motion spec=<spec.json> out_dir=<folder> [tracks=a,b] [stills=4] [video=false] [gpu=true] [crf=24]
```

- `stills=N video=false` renders N evenly spaced transparent PNGs per track in seconds. Use it to review motion before a full export.
- Full export automatically crops sparse full-canvas layers to their tight alpha bounding boxes, renders PNG frames on the discrete GPU (`gpu=false` for CPU), and encodes VP9 with alpha through ffmpeg (`-threads 4`). Build and check with stills first.
- Outputs: `<track>.webm` (VP9 with alpha, spec fps), `review/<track>.png` (frames across the clip, reference first) and `review/<track>_detail.png` (close-ups), `stills/`, and `work/` (generated JSX, cropped layers, and project).
- Every exported clip is measured automatically (see Validation); the result has `validation_ok` and per-track `validation`.

## Validation (no vision needed)

`mc exec validate.motion video=<clip.webm> [frames=N fps=30 loop=true reference=<image> spec=<spec.json>]`
checks frame count and rate, alpha, empty or clipped frames, teleports and flicker, loop closure, colour and size against a
reference image, and lints the spec for oscillations that alias at its frame rate. It proves "no measurable problem", not that
the result looks right: also review `review/*_detail.png` visually against the same reference.

## Spec (JSON)

`canvas` `[w,h]`, `fps`, optional `motionBlur` `{shutterAngle, samples}`, optional `margin` `[left, top, right, bottom]` (enlarges every clip beyond the canvas so wings, paths and edge-touching artwork are never cropped; undo the offset when compositing), optional `reference` (image of the expected look), `layers`, `tracks` (track `loop: true` requires the last frame to match the first).

- layer: `id`, `file` (relative to the spec; must equal the canvas size), `pivot` `[x,y]` or `{"anchor":"top|bottom|left|right|center|topLeft|…","of":"alphaBounds"}`, optional `parent` (layer id), optional `component` N to use only the Nth separate visible piece (left to right) of the image.
- layers are listed bottom to top. Parent links make a layer follow its parent's motion about the parent's pivot.

## Motion terms (summed per property: `position`, `rotation`, `scale`, `opacity`)

All values are offsets from the resting state (`opacity` -100 hides a layer).

- `{"kind":"keys", "keys":[[t, value], ...], "ease":"smooth|linear|in|out"}` — `value` is a number, or `[dx, dy]` for position/scale.
- `{"kind":"ellipse", "centre":[dx,dy], "rx":N, "ry":N, "start":rad, "turns":1, "from":t0, "to":t1, "ease":"smooth"}` — position only; relative to the point on the ellipse at `start` unless `"relativeTo":[x,y]` is set.
- `{"kind":"oscillate", "amplitude":N, "hz":N, "phase":0, "axis":0, "envelope":[[t, mult], ...]}` — written as an EffectCraft expression (`Math.sin(...)`), so high frequencies (e.g. wings) integrate smoothly across the motion-blur shutter instead of aliasing to frame keys.
- `{"kind":"follow", "layer":"body", "gain":-0.04, "limit":18, "smooth":0.15}` — rotation only; leans with a layer's horizontal velocity (including `keys`, `ellipse`, and horizontal `oscillate` terms) with exponential smoothing.

## Verification

Inspect actual frames (stills or decoded video on a plain background), not file existence: check that parts stay attached at their pivots, transparent background is preserved, and frame-to-frame motion is continuous.

## Known behaviour

- EffectCraft keeps a child's world position at the moment of linking, so parent links are made before keyframes and every layer starts at the same offset (done by the module).
- Pieces that must be separate but are not (antennae, a body without them) can be cut with `image.break_apart` regions (`{polygons, keep: "outside"}` clears the polygon instead of keeping it).
- A baked clip cannot be re-timed per part in the editor; change the spec and re-export, or export parts as separate tracks.
