---
name: asset-sheets
description: Use whenever an agent must produce Magic Studio asset sheets — a turnaround, a skeleton and limbs sheet, or a pose sheet — for an imported character. The exact canvas, layout, naming and background rules so every sheet is cut, registered and rigged the same way.
---

# Asset sheets (turnaround · skeleton & limbs · poses)

One imported, whole-body reference image goes in; three sheets come out. They are drawn by an
image agent (Codex in Herdr, or the app's configured agent) and are then cut by PhotoCraft,
registered and rigged without re-layout.

Every sheet follows **the same rules**. A sheet that breaks them is rejected by
`pipeline/import_validator.py` (3–5 views, a side profile, stable height) and must be drawn again.

## Before the first sheet

Establish the roster from evidence, not from memory: read the project's registered assets and any
`assets/sheets/` already on disk, then state which characters are past the drawing step and which
are not. The user picks the characters and the order. Do not assume a roster, a count, or that a
named preset means one specific character — preset tables map names to literal filenames and go
stale the moment a file is renamed. Report what exists and confirm before drawing anything.

## Shared rules (all sheets)

- Canvas **2048 × 2048 px**, PNG, sRGB.
- Background: plain, flat **#ffffff**; no gradients, shadows, textures, frames or labels. No text
  anywhere on the sheet — the prompt is saved beside it as `<sheet>.prompt.md` instead.
- The character is drawn **once per cell**, whole: nothing touches the canvas edge, no crop, no
  overlap between cells. Minimum 64 px of clear background around any drawing.
- Same character, palette, line weight and proportions as the reference. Do not redesign.
- Leave **48 px gutters** between cells.

## 1. Turnaround sheet — `<character>-turnaround.png`

- Four views in **one row**, equal height, same scale, same baseline:
  `front`, `right profile`, `back`, `left profile`.
- Each view 448 px wide at most, 1600 px tall, feet on a shared baseline at y = 1856.
- Front view faces the viewer; profile views keep the nose pointing right.
- No skeleton, no pose variation, no shading differences between views.

## 2. Skeleton & limbs sheet — `<character>-skeleton.png`

- **Two columns**: left = whole front view (1200 px tall) with joint markers; right = the same
  character's separated limbs, one under another.
- Joint markers: solid circles, radius **12 px**, colour `#ff2d55`, at neck, both shoulders, both
  elbows, both wrists, hips, both knees, both ankles.
- Separated limbs (right column, top to bottom): head, torso, upper arm L/R, forearm L/R, hand L/R,
  thigh L/R, shin L/R, foot L/R, tail or any extra appendage. Each piece is a **whole closed
  shape** with the 48 px gutter, drawn in the same style and colours as the front view.
- No text, no arrows, no measurement lines.

## 3. Pose sheet — `<character>-poses.png`

- **3 columns × 2 rows**: `idle`, `walk`, `jump`, `wave`, `talk`, `cheer` in that order.
- Each cell **600 × 900 px**, 48 px gutters between cells; the grid (1896 × 1848) is centred on
  the 2048 canvas (76 px side margins, 100 px top/bottom).
- The character is **820 px tall**, feet on the cell baseline 40 px above the cell's bottom edge,
  horizontally centred.
- Poses differ only in limb and body attitude; scale, palette and camera stay identical.
- No motion blur, no props, no text.

## Template (copy this, fill the brackets)

```text
Draw one asset sheet for the character in the attached reference image.

Reference: <reference.png>   (match its design exactly: palette, line weight, proportions)
Sheet: <turnaround|skeleton|poses>   (follow the layout rules above for this sheet)

Rules: 2048x2048 is the target canvas. If the image tool cannot produce exactly that size,
produce the largest square it can (>= 1024) and print the size it returned; the sheet is then
normalised to 2048x2048 with the app's CLI — never by cropping by hand.
Plain #ffffff background, no text, no shadows, nothing touching the edge (>= 64 px margin),
>= 48 px between drawings. Draw the character once per cell, whole and identical across cells.

Write the sheet to <project>/assets/sheets/<character>-<sheet>.png and this exact prompt to
<project>/assets/sheets/<character>-<sheet>.prompt.md.
```

## Normalise, matte and connect — through `mc` only

```bash
# 1. fixed contract size, whatever the image tool returned
mc exec image.image_size source=assets/sheets/<character>-<sheet>.png \
  out=assets/sheets/<character>-<sheet>-2048.png width=2048 height=2048

# 2. cut the sheet into frames/parts and remove the background (PhotoCraft under the hood)
mc exec asset.cut_sheet asset=<character>

# 3. connect the prepared artwork to the animated subject
mc exec project.add_asset path=assets/sheets/<character>-turnaround-2048.png \
  kind=image character=<character> load=true

# 4. judge it, then rig
mc exec assets.assess path=assets/sheets/<character>-turnaround-2048.png
mc exec asset.build_cutout_rig
```

Run the steps that have a command through `mc`, and read each command's JSON before the next.
Iterating on a static sheet is expected work: when a stage must be re-run with variations to get
the result right, a small throwaway driver is an acceptable tool, and no sheet is finished before
the user has seen it. A step missing from `mc commands` is a gap to report and get filled — not a
reason to stall or to quietly substitute a different deliverable.

## Naming and prompts

```text
<project>/assets/sheets/<character>-turnaround.png
<project>/assets/sheets/<character>-skeleton.png
<project>/assets/sheets/<character>-poses.png
<project>/assets/sheets/<sheet>.prompt.md     the exact prompt that produced the sheet
```

## After the sheets exist — the agent runs this chain, in order

```bash
mc exec intake.validate image=<sheet.png> slot=turnaround_sheet attach_to_active=true
mc exec asset.cut_sheet asset=<id>          # PhotoCraft cuts frames/parts
mc exec assets.assess path=<sheet.png>      # clean or recreate, with reasons
mc exec asset.build_cutout_rig              # parts + pivots from the cut sheet
```

The handoff between those commands is one field: intake records its recommendation's
`selectedParts`, and `asset.build_cutout_rig` passes exactly that list as `--parts` to the rig
builder. Run intake first, or the builder guesses which parts to cut. Save the project after the
rig builds — the rig attaches to the in-session asset and is gone on reload until it is saved.

## Pitfalls

- The rig attaches to the in-session asset only; `project.save` is what makes parts survive a reload.
- `rig.from_sheet` refuses a sheet that has no `generation.json` beside it (only
  `image.generate rig_parts=` writes one). Send image-agent sheets to `asset.build_cutout_rig` instead.
- The app serves asset files only from its configured roots (home and removable-media trees). A
  project anywhere else shows as `Could not load <name>` in the Animation pane — that reads like a
  broken sheet and is actually a path problem, so check the project location before redrawing art.

## Reject and redraw when

- a view is cropped, touches an edge, or the count is not four;
- the profile is missing or faces left;
- character height drifts more than 4 % between views;
- cells overlap, text appears, or the background is not flat;
- limbs are drawn detached on the turnaround or poses sheets (only the skeleton sheet separates them).

Use `mc` for all of it: it is the harness wrapping the *craft CLIs (see
`~/.agents/skills/cli-anything/SKILL.md`). Never call `photocraft-cli` directly for these steps, and
read each command's JSON before running the next. Do not report success from a file existing alone.
