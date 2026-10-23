# Reference artwork → pose sheet → animation → storyboard → video

Use the shared engine commands through `mc`, MCP or the app. Image drawing uses the app's **native Codex image generator**, separate from chat-agent selection. No chat message or provider switch is involved.

## Draw an action from an existing character

```bash
mc exec image.generate project='/path/Song.magic' \
  reference_asset=older_fish_fabric_swim_v2 \
  character=older_fish_layered scene=2 animation=turn-around \
  poses='["right-facing three-quarter toward viewer","front-facing","left-facing three-quarter toward viewer"]' \
  prompt='Keep this exact fabric fish. Draw a rounded turn: the far eye and fin emerge, the tail passes behind the body, and the belly retains volume. Preserve its woven scales, red stitched fins, brass joints and friendly face.' \
  transparent_background=true name='Older fish turn poses v1' model=gpt-6-luna
```

- `reference_asset` resolves a registered PNG, not a JSON rig. Use a complete-character reference/poster when drawing a turn. Existing `references=[...]` paths still work; at most five images total are attached.
- `poses` requests 2–8 ordered complete-character drawings on one sheet. It requires a reference. The generator receives the actual reference PNG bytes with `--image`, not only a filename in a prompt.
- `character`, `scene` and `animation` retain the intended destination. The character must be registered and the scene must exist in the saved native storyboard. `animation` names the intended action; it does not claim a playable clip has been built.
- The native generation receipt contains reference hashes/IDs, ordered pose descriptions, character identity, saved storyboard version and measured output dimensions. `asset.prompt.md` saves the complete worker instruction. The output PNG is attached to the project as a new image asset. Originals are preserved.
- Layout and dimensions are requests to the image tool, not verified crop boundaries. Review the actual drawings before cutting them or using them in a film.

For a running app, explicitly select its bridge. It must be rebuilt with the updated engine; inspect `mc --bridge PORT exec command.list filter=image.generate` and confirm its parameter description includes `reference_asset` and `poses`. A new CLI driving an older app does not upgrade the app's command handlers.

## Save the storyboard reference

Use the returned `asset_id` with the existing placement/save path, choosing coordinates from `storyboard.read` or `storyboard.design`:

```bash
mc --bridge PORT exec storyboard.place_asset asset=GENERATED_ASSET page=3 x=574 y=890 width=428
mc --bridge PORT exec storyboard.save_version
```

Save source identity and intended clip in the scene's native fields. This is a static pose-sheet reference, not an animated PDF. Do not mark it approved on the user's behalf.

## Turn drawings into motion

Keep the whole-character reference and individual parts. Use the existing `image.image_size`, `intake.validate`, `asset.cut_sheet` and rig commands after visual review, with their current `mc commands` parameter contracts. A generated pose sheet does not automatically become a valid rig or a 3D mesh.

For a sheet whose drawings are not evenly spaced, do not cut equal cells through a nose or tail. Load the generated image, inspect its actual silhouette bounds, then use the existing PhotoCraft-backed polygon separation command:

```bash
mc --bridge PORT exec project.load_asset id=GENERATED_ASSET
mc --bridge PORT exec image.break_apart asset=GENERATED_ASSET \
  out_dir=/path/to/project/assets/characters/CHARACTER/turn/frames \
  'regions={"quarter_right":[[18,55],[752,55],[752,700],[18,700]],"front":[[760,45],[1336,45],[1336,686],[760,686]],"quarter_left":[[1338,60],[2090,60],[2090,700],[1338,700]]}'
```

Those coordinates are an example from the older fish's 2103×748 sheet, not a universal grid. Read `parts`, `parts_produced` and `errors` in the result and inspect every output. This command masks each view on the original canvas, preserving alpha. Record the pivot/translation and common scale that register each body into the existing rig; a shared image size alone does not register the drawings. Forward and reverse turns can reuse the same three views, with timing derived from the existing facing keys.

Register the separated views with the native CLI, preserving their measured body pivot and proportions:

```bash
mc exec image.register_frame \
  source=/path/to/project/assets/characters/CHARACTER/turn/frames/quarter_right.png \
  out=/path/to/project/assets/characters/CHARACTER/turn/registered-v1/quarter_right.png \
  'canvas=[1400,900]' 'source_pivot=[490,389]' 'target_pivot=[700,450]' scale=1.15
```

This uses PhotoCraft Place Embedded on a transparent canvas. It refuses an existing output or a placement that clips any nontransparent source pixel, checks the decoded PNG dimensions/alpha channel, and saves a `.registration.json` with pivots, scale, bounds and source hash. The example measurements belong to the older fish; measure each character. Register the profile and all turn views onto the same canvas and body pivot. A wider canvas allows the profile to face either direction without losing the tail.

The fish proof stores these as five layers (right profile, right quarter, front, left quarter, left profile) with named `turn-left` and `turn-right` opacity-key tracks. Both tracks use the same registered artwork; the left profile is the right profile flipped around the common pivot. Register the spec using `project.add_asset kind=cutout_rig`, then `motion.clips.convert character=...` and `project.save`. The native Animation tab and `export.layered_motion` can now play/render that action. This is a reusable action asset linked to the source character; the original swimming rig stays editable.

For a turn, profile → three-quarter → front → opposite three-quarter → opposite profile provides actual shape changes. Use a short anticipation, head-led turn, delayed tail/fins and a settling hold. A flat plane rotated in Three.js still has no thickness. Mesh deformation and drawing substitutions can be combined; Remotion can schedule/interpolate those poses but cannot invent the missing views.

Once a named track exists, inspect `motion.tracks`, sample with `motion.sample`, convert/save native Theatre clips with `motion.clips.convert` / `motion.theatre.save`, and export with `motion.export_track`. That exporter uses the engine's sampled motion. Do not assume an unrelated HTML performance or arbitrary Theatre-only edits are already part of that export.

## Place a rendered animation and retain its source

```bash
mc --bridge PORT exec video.place_track \
  id=older-fish-first-turn path=/path/to/rendered/turn.webm \
  character=older_fish_volume_turn track=turn-left \
  start=22.30 duration=1.10 'rect=[1180,340,520,390]' scene=2 \
  generation_receipt=assets/generated-images/REQUEST/generation.json
```

`character` and `track` are optional as a pair. When supplied, the engine reads the saved rig without changing the selected part, checks the registered character and saved named clip, then saves the source spec/hash and duration beside the placed video. This connects the native Animation action to its film placement. Runtime-only standard preview tracks must first be saved as named rig tracks if they need this link. Set `playback_rate=source_duration/window_duration` explicitly when a complete action must fit a shorter scene window; the HyperFrames runtime owns the media clock. The fish film uses this for the shorter return-to-parent turn.

`generation_receipt` is optional for existing callers. When supplied, it must be a successful native generation receipt inside this project, and the generated artwork must still match its SHA-256. Placement retains the generated asset, pose order and animation context in `hyperframes/assets/animation/CLIP_ID.json`, together with video timing, rectangle, copied media and the composition backup. This is an explicit source association; it does not falsely certify that a particular renderer used those pixels.

For an existing procedural HTML character, retain its world travel and bank. The fish film now plays the native turn media only during each placed action window, fades the underlying articulated profile out during that window, and resumes its swimming rig afterward. The `data-magicstudio-character` / `data-magicstudio-track` attributes identify the action; HyperFrames owns media time. World-path binding and the registration pivot are still authored explicitly, not inferred by `video.place_track`. The current project maps all nine native turn placements in `workflow/fish-storyboard-video-map.json`.

## What is automatic and what still needs authoring

Automatic: resolve attached references; request ordered drawings through native image generation; register the returned PNG; retain provenance; save storyboard placements; insert a rendered video with its source association.

Still authored/reviewed: silhouette consistency, frame registration, occlusion and thickness during a turn, part rigging, mapping native clips into a procedural video, and any final film approval. Replace this manual step only when the shared CLI gains a verified implementation; do not report it as an existing automatic handoff.

Technique references: [Spine image meshes](https://esotericsoftware.com/spine-meshes), [Spine attachments](https://esotericsoftware.com/spine-attachments), [Three.js planes](https://threejs.org/docs/pages/PlaneGeometry.html), [Remotion interpolation](https://www.remotion.dev/docs/interpolate).
