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
- Full export is slow (several minutes for ~600 frames at 1080p); build and check with stills first.
- Outputs: `<track>.webm` (VP9 with alpha, canvas size, spec fps), `stills/<track>/NN.png`, and `work/` (generated JSX and project).

## Spec (JSON)

`canvas` `[w,h]`, `fps`, optional `motionBlur` `{shutterAngle, samples}`, `layers`, `tracks`.

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

- EffectCraft keeps a child's world position at the moment of linking, so parent links are made before keyframes (done by the module).
- A baked clip cannot be re-timed per part in the editor; change the spec and re-export, or export parts as separate tracks.
