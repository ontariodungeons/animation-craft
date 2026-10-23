# Blender animation in Magic Studio

Magic Studio owns project identity, storyboard placement and job controls. Blender owns
the saved animation scene. This first migration preserves layered artwork and existing
motion as keyframes; it does not manufacture a volumetric character from flat artwork.

## Run

Open a `.magic` project in one persistent `mc run` session, or use its explicit live bridge.

```text
mc --bridge PORT exec blender.start character=little_fish_layered track=swim-cruise
mc --bridge PORT exec blender.status job=RETURNED_JOB_ID
mc --bridge PORT exec blender.cancel job=RETURNED_JOB_ID
mc --bridge PORT exec blender.list
```

Start returns a job ID immediately after submitting the worker. Status distinguishes
running, completed, failed and cancelled. A submitted job is not a successful render.
Default preview is at 50% resolution and at most 90 frames; `max_frames=18000`
requests up to ten minutes at 30fps. Results explicitly label shortened previews.

The Animation tab's Blender mode selects a rig and authored track, renders, cancels and
plays the result. It restores the most recent matching job from project-local receipts.
Legacy exposes the previous editor and preserved Theatre state during migration.
The first open defaults to Blender. Subsequent explicit backend choices are remembered;
the old subject-mode preference does not silently enable the legacy editor.

Files live under `animation/blender-jobs/JOB_ID/` beside the `.magic` project:

- `animation.blend`: editable scene with packed source textures and baked keyframes.
- `frames/*.png`: transparent rendered frames.
- `preview.webm`: transparent VP9 clip for preview and video placement.
- `job.json`, `request.json`, `progress.json`, `result.json`, `worker.log`: provenance and progress.

To render an edited Blender scene, pass `blend=/absolute/animation.blend` to
`blender.start`, with character and track identifiers to associate the result with the
same selection. This loads the saved scene instead of re-evaluating legacy motion.
The UI's subsequent renders use the last successful saved Blender scene. To explicitly
reimport changed legacy motion, call `blender.start` without `blend`.

Place the completed transparent clip through the existing `video.place_track` command;
keep the storyboard start, duration and actor transform authoritative. Discover its
current arguments with `mc commands video.place_track`. Do not place a shortened preview
as if it covered an entire action.

## Process and memory ownership

Requires Linux user systemd, Blender and FFmpeg with libvpx-vp9. `BLENDER_BIN` selects
Blender; otherwise PATH is used. The Python worker is embedded in mc at build time.
No Blender TCP daemon or generated character-specific Python is required.

Each job runs in its own systemd service with MemoryMax (default 4096MB), MemorySwapMax=0,
CPUQuota=200%, two render threads and RuntimeMaxSec (default 300). Cancellation stops
that service and its child encoder. There is one active job per project. Limits are
per job, not a global cap across separate projects. No fallback silently removes limits.
Blender auto-execution is disabled when opening files; saved scenes requiring Python
drivers may need adaptation to keyframes. Saved jobs and frames remain on disk for review.

The browser receives job metadata and a media URL, not the entire sampled animation.
WebKit previews use one owned blob URL, capped at 64MB while streaming the local file;
navigation aborts loading, releases the video source and revokes that URL.
Entering Blender mode releases the old image/sample references and Theatre iframe
references after checking for unsaved edits. This reduces the active integration surface;
it is not evidence that the historical memory leak has been diagnosed or fixed.

Migration frames the union of the original canvas and measured motion bounds. A path
leaving the original canvas is therefore visible rather than silently cropped. Check
the resulting framing when placing its rendered clip into a scene.

## Tool roles and next migration boundary

Blender is the first working backend. OpenToonz is a candidate for drawn poses and Plastic
deformation; Synfig for 2D bitmap cutout animation; MLT/melt for final clip assembly.
Those integrations are not implemented by this change. Godot is excluded to avoid a
second general scene engine. Keep each editable source owned by one tool and exchange
rendered clips plus provenance, rather than synchronizing every tool's keyframe graphs.

The old `export.blender` archetype path remains a legacy command. This new worker does not
call its generated humanoid script. Removing that command and Theatre entirely requires
migrating their remaining callers and saved editing workflows first.
