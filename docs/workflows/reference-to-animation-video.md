# Reference artwork → pose sheet → animation → storyboard → video

Use the shared engine commands through `mc`, MCP or the app. Image drawing uses the app's **native Codex image generator**, separate from chat-agent selection. No chat message or provider switch is involved.

## Draw an action from an existing character

```bash
mc exec image.generate project='/path/Song.acproj' \
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

For a turn, profile → three-quarter → front → opposite three-quarter → opposite profile provides actual shape changes. Use a short anticipation, head-led turn, delayed tail/fins and a settling hold. A flat plane rotated in Three.js still has no thickness. Mesh deformation and drawing substitutions can be combined; Remotion can schedule/interpolate those poses but cannot invent the missing views.

Once a named track exists, inspect `motion.tracks`, sample with `motion.sample`, convert/save native Theatre clips with `motion.clips.convert` / `motion.theatre.save`, and export with `motion.export_track`. That exporter uses the engine's sampled motion. Do not assume an unrelated HTML performance or arbitrary Theatre-only edits are already part of that export.

## Place a rendered animation and retain its source

```bash
mc --bridge PORT exec video.place_track \
  id=older-fish-first-turn path=/path/to/rendered/turn.webm \
  start=22.30 duration=1.10 'rect=[1180,340,520,390]' scene=2 \
  generation_receipt=assets/generated-images/REQUEST/generation.json
```

`generation_receipt` is optional for existing callers. When supplied, it must be a successful native generation receipt inside this project, and the generated artwork must still match its SHA-256. Placement retains the generated asset, pose order and animation context in `hyperframes/assets/animation/CLIP_ID.json`, together with video timing, rectangle, copied media and the composition backup. This is an explicit source association; it does not falsely certify that a particular renderer used those pixels.

For an existing procedural HTML character (as in the fish film), keep its separate PNG parts and renderer. Record the same generation receipt in its project mapping instead of inserting a duplicate rendered fish. The current project demonstrates this path in `workflow/fish-storyboard-video-map.json`.

## What is automatic and what still needs authoring

Automatic: resolve attached references; request ordered drawings through native image generation; register the returned PNG; retain provenance; save storyboard placements; insert a rendered video with its source association.

Still authored/reviewed: silhouette consistency, frame registration, occlusion and thickness during a turn, part rigging, mapping native clips into a procedural video, and any final film approval. Replace this manual step only when the shared CLI gains a verified implementation; do not report it as an existing automatic handoff.

Technique references: [Spine image meshes](https://esotericsoftware.com/spine-meshes), [Spine attachments](https://esotericsoftware.com/spine-attachments), [Three.js planes](https://threejs.org/docs/pages/PlaneGeometry.html), [Remotion interpolation](https://www.remotion.dev/docs/interpolate).
