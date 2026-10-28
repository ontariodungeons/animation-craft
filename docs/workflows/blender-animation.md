# Blender animation in Magic Studio

Magic Studio owns project identity, storyboard placement and job controls. Blender owns
the saved animation scene. This first migration preserves layered artwork and existing
motion as keyframes; it does not manufacture a volumetric character from flat artwork.

## Run

Open a `.magic` project in one persistent `mc run` session, or use its explicit live bridge.

```text
mc --bridge PORT exec blender.start character=little_fish_layered track=swim-cruise
mc --bridge PORT exec animation.status job=RETURNED_JOB_ID
mc --bridge PORT exec animation.cancel job=RETURNED_JOB_ID
mc --bridge PORT exec animation.list
```

`animation.status/list/cancel` are the canonical generic job commands (they cover
Blender, Synfig, OpenToonz and MLT). `blender.status/list/cancel` remain aliases for
existing scripts; MCP exposes only the canonical `animation.*` tools.

Start returns a job ID immediately after submitting the worker. Status distinguishes
running, cancelling, completed, failed and cancelled. Cancelling stays busy until the service and its children exit. A submitted job is not a successful render.
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

Place a completed job with `video.place_track blender_job=JOB_ID id=… start=… duration=… rect=[x,y,w,h]`;
the storyboard start, duration and actor rect stay authoritative. `rect` frames the original
canvas (object-fit: contain); placement maps it to the receipt's `world_rect` so the character
keeps its canvas registration. Placement rejects partial previews, a clip whose `clip_sha256`
no longer matches, and a duration longer than `clip_seconds / playback_rate`. The placement
record keeps the provenance frozen at render time; later rig edits do not rewrite it.

## Process and memory ownership

Requires Linux user systemd, Blender and FFmpeg with libvpx-vp9. `BLENDER_BIN` selects
Blender; otherwise PATH is used. The Python worker is embedded in mc at build time.
No Blender TCP daemon or generated character-specific Python is required.

All current jobs share the fixed user service `magicstudio-animation-worker.service`.
A nonblocking lock in `XDG_RUNTIME_DIR` serializes submission and cancellation; systemd
owns the atomic slot across projects and CLI/Studio processes. Busy jobs reject new starts;
there is no queue. This is one slot per Linux user, not a machine-wide limit across users.
Older independently named workers must finish before switching backend versions.
MemoryMax defaults to 4096 MiB and accepts 512–8192 MiB, with MemorySwapMax=0,
CPUQuota=200%, two render threads and RuntimeMaxSec (default 300 seconds, maximum 3600).
No fallback silently removes limits. Cancellation submits `stop --no-block`, with
TimeoutStopSec=5 and KillMode=control-group. `cancelling` remains busy through shutdown
and terminal telemetry collection. A stale cancel checks the job identity under the same
lock and cannot stop a replacement job.

`blender.actions character=ID` reads at most 16 MiB of saved rig JSON and returns
`{character,tracks:[{id,duration,loop,standard:false}],layers:[],canvas,spec}`.
It never loads artwork or samples motion. Start stores a small request; the bounded
Blender worker invokes adjacent `mc exec blender.prepare request=...` headlessly to use
the existing Rust evaluator inside the same cgroup. Preparation requires the matching
service invocation. It freezes the original rig bytes in `rig-snapshot/original-rig.json`,
resolves original artwork references and evaluates a job-local `rig-snapshot/rig.json`.
Derived preview caches are written there, never beside the production rig. Referenced
artwork must remain readable until preparation packs it into the saved scene.
Effective parents follow the evaluator's track-layer filtering. Blender `fps/fps_base`
preserves the sampled fractional frame rate; keyframe times use that same rate.
Only saved Magic Studio migration scenes with frozen provenance are supported for rerender.
Requested character and track must match the scene provenance. Arbitrary external blend
files, GPU rendering, custom compositor graphs and Python-driver behavior are unsupported.
The worker forces Cycles CPU and disables compositor/sequencer use; auto-execution is disabled.
This is resource containment, not a security sandbox for untrusted blend files or GPU drivers.
Saved jobs and frames remain on disk for review; automatic retention and an aggregate disk
budget are not implemented. Runtime/frame limits do not guarantee bounded total disk use.

The browser receives job metadata and a media URL, not the entire sampled animation.
WebKit previews use one owned blob URL, capped at 64MB while streaming the local file;
navigation aborts loading, releases the video source and revokes that URL.
Entering Blender mode releases the old image/sample references and Theatre iframe
references after checking for unsaved edits. This reduces the active integration surface;
it is not evidence that the historical memory leak has been diagnosed or fixed.

Migration frames the union of the original canvas and measured motion bounds with a 2% margin.
The orthographic camera uses HORIZONTAL sensor fit, including portrait canvases. Each completed
receipt records `canvas` and actual full-frame `world_rect:[left,top,right,bottom]` in original
canvas units, accounting for output aspect ratio. Placement must map this extent instead of
assuming one output pixel equals one canvas pixel. Animated camera registration is rejected.

Completed `result.json` keeps absolute `clip`/`blend`, `width`, `height`, `partial`, `fps`,
`first_frame`, `last_frame` and `clip_seconds=(last_frame-first_frame+1)/fps`.
Top-level `character`, `track`, original `spec`, exact original-byte `rig_sha256`, `canvas`
and `source_job` survive saved-scene rerenders via scene custom properties. `source_blend`
and streamed `source_blend_sha256` identify the immediate saved input (copied to job-local
`source.blend` before hashing/loading); `clip_sha256` is the
full lowercase hex SHA-256 of the output clip. Current rig changes never replace the frozen
rendered provenance. `backend:blender` and `artwork` packed-crop hashes identify the baked
artwork dependencies. Rotated/scaled camera registration is unsupported. Placement must reject incomplete/partial jobs and require placement
seconds <= clip_seconds / playback_rate.

Live status and final receipts identify `job`/`job_id`, canonical `project`, `unit`,
`invocation_id` and `updated_at` (Unix milliseconds, decimal string). Live phases are
preparing/loading/rendering/encoding with `frames_completed`/`frames_total` when known.
The `telemetry` object carries `memory_current_bytes`, `memory_peak_bytes`,
`memory_max_bytes`, `swap_current_bytes`, `swap_max_bytes` and cumulative `cpu_usage_ns`.
Unavailable readings are null; measured zero remains zero. `worker_peak_rss_kib` is the
Blender process RSS and remains separate from cgroup peak. `service_result`,
`exec_main_status`, `exit_code`, `exit_status` and `termination_reason` retain termination
evidence (cancel/timeout/oom/failure). A service stop-post hook persists the final receipt
before collection; success is published only after the renderer/encoder exits. Live metrics
require matching unit Description and invocation ID, so an old project cannot display a
replacement job's memory. Missing terminal receipts are failures with unknown usage,
never inferred success. Historical foundation jobs keep their existing receipts.
If the user bus or `systemctl` is unavailable, status can still return a saved terminal
completed/cancelled/failed receipt. That response preserves the receipt timestamp and
measurements, labels `state_source`/`telemetry_source` as `terminal_receipt`, sets
`telemetry_stale:true` and `service_state:null`, and includes `service_error`. These are
historical measurements, not a claim about a live service. Without a terminal receipt,
the service-query error is returned; running/cancelling state is never invented.

`animation.list` also works without an open project (`jobs:[]`) and always returns a global
worker observation. `animation.status job=...` adds the same observation beside the selected
job; it never substitutes the global owner's metrics for that job's metrics.
`worker_owner` is null when no active owner is observed. `worker_owner_status` distinguishes
`idle`, `observed`, and `unavailable`; `worker_owner_error` is null or the actual query/metadata
error. A disconnected user bus reports unavailable, not idle. An observed owner contains
`job`/`job_id`, canonical `project`, `unit`, `invocation_id`, `state` (running/cancelling),
`service_state` and `updated_at`. Identity comes from the actual fixed unit's Description
and invocation, matched to an atomically published runtime owner pointer and job metadata;
it is never inferred from the selected project's latest job. If matching runtime metadata
is unavailable, the observed unit/job remains visible, but project/limits stay null with
an explicit error. Reads reuse one systemd snapshot and add no scheduler or polling service.

New requests, job metadata, live status and final receipts retain a `limits` object with
`memory_max_bytes`, `swap_max_bytes`, `cpu_quota_percent` and `timeout_seconds`, recording
what was submitted (including validated engine defaults). This is distinct from measured
cgroup telemetry; clients must not invent defaults for historical jobs lacking this record.
`submitted_at` is the durable request's elapsed-time origin, `started_at` is the observed
systemd ExecMain start timestamp, and `cancellation_requested_at` comes from the persisted
cancel marker. All are Unix milliseconds as decimal strings or null. The systemd start
observation uses `--timestamp=unix`, whose installed output has second precision (encoded
as milliseconds); it is not the later Python-entry time. The worker persists its start and
invocation in job metadata, and stop-post persists limits/start/cancel timestamps in the
terminal receipt. While stopping, list/status read the atomic cancel marker directly,
avoiding concurrent writes to the worker's job metadata. Missing historical limits or
timestamps remain null, including after a user-bus disconnect.

## Tool roles and next migration boundary

Blender is the first working backend. OpenToonz is a candidate for drawn poses and Plastic
deformation; Synfig for 2D bitmap cutout animation; MLT/melt for final clip assembly.
Those integrations are not implemented by this change. Godot is excluded to avoid a
second general scene engine. Keep each editable source owned by one tool and exchange
rendered clips plus provenance, rather than synchronizing every tool's keyframe graphs.

The old `export.blender` archetype path remains a legacy command. This new worker does not
call its generated humanoid script. Removing that command and Theatre entirely requires
migrating their remaining callers and saved editing workflows first.
