---
name: layered-motion
description: Use when animating pre-separated transparent layers (any subject) into transparent video clips with AnimationCraft, for compositing in Remotion or another editor.
---

# Layered motion

Turns a spec of transparent, same-size layer images into one transparent WebM per track. It assumes nothing about the subject: no anatomy, part names, pose counts or character.

## Run it through AnimationCraft

From the checkout that holds `pipeline/` (the command finds the script from the working directory):

```bash
animationcraft-cli exec export.layered_motion spec=<spec.json> out_dir=<folder> \
  [tracks=<id,id>] [stills=<n>] [video=false]
```

- `stills=N video=false` renders N evenly spaced transparent PNGs per track in seconds. Use it to review motion before a full export.
- Full export renders PNG frames on the GPU (`gpu=false` for CPU) and encodes VP9 with alpha through ffmpeg: ~4 min for ~570 frames at 1.5x 1080p on a GTX 1660. Build and check with stills first.
- Outputs: `<track>.webm` (VP9 with alpha, spec fps), `review/<track>.png` (frames across the clip, reference first) and `review/<track>_detail.png` (close-ups), `stills/`, and `work/` (generated JSX and project).
- Every exported clip is measured automatically (see Validation); the result has `validation_ok` and per-track `validation`.

## Validation (no vision needed)

`animationcraft-cli exec validate.motion video=<clip.webm> [frames=N fps=30 loop=true reference=<image> spec=<spec.json>]`
checks frame count and rate, alpha, empty or clipped frames, teleports and flicker, loop closure, colour and size against a
reference image, and lints the spec for oscillations that alias at its frame rate. It proves "no measurable problem", not that
the result looks right: also review `review/*_detail.png` visually against the same reference.

## Spec (JSON)

`canvas` `[w,h]`, `fps`, optional `motionBlur` `{shutterAngle, samples}`, optional `margin` `[left, top, right, bottom]` (enlarges every clip beyond the canvas so wings, paths and edge-touching artwork are never cropped; undo the offset when compositing), optional `reference` (image of the expected look), `layers`, `tracks` (track `loop: true` requires the last frame to match the first).

- layer: `id`, `file` (relative to the spec; must equal the canvas size), `pivot` `[x,y]` or `{"anchor":"top|bottom|left|right|center|topLeft|…","of":"alphaBounds"}`, optional `parent` (layer id), optional `component` N to use only the Nth separate visible piece (left to right) of the image.
- layers are listed bottom to top. Parent links make a layer follow its parent's motion about the parent's pivot.
- track: `id`, `duration` (s), optional `layers` (subset, default all), `motion` `{layerId: {position|rotation|scale|opacity: [terms]}}`.
- Terms add together and are offsets from the resting state (position px from pivot, rotation deg, scale and opacity in percentage points):
  - `keys` `[[t, value], ...]` with `ease` `smooth|linear|in|out` (value is a number, or `[x,y]` for position/scale)
  - `ellipse` `centre, rx, ry, start, turns, from, to, ease, relativeTo`
  - `oscillate` `amplitude, hz, phase, axis, envelope [[t, multiplier]...]` (continuous sine, stays smooth under motion blur)
  - `follow` `layer, gain, limit, smooth` (rotation leaning with a layer's horizontal velocity)

## Checks before using the clips

Inspect actual frames (stills or decoded video on a plain background), not file sizes. Confirm every layer sits where it should relative to its parent, the track length matches the intended window, and the alpha survives import in the editor. Report defects instead of working around them.

## Known behaviour

- EffectCraft keeps a child's world position at the moment of linking, so parent links are made before keyframes and every layer starts at the same offset (done by the module).
- Pieces that must be separate but are not (antennae, a body without them) can be cut with `image.break_apart` regions (`{polygons, keep: "outside"}` clears the polygon instead of keeping it).
- A baked clip cannot be re-timed per part in the editor; change the spec and re-export, or export parts as separate tracks.
