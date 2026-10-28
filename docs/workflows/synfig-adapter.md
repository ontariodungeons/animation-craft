# Synfig native adapter (#23)

This slice renders an already-authored root canvas from `.sif` or gzip `.sifz`.
It does not author rigs or evaluate motion itself. Install `synfig` (Arch package
`synfig`), `python-pillow`, and FFmpeg with libvpx-vp9. `SYNFIG_BIN` may select an
explicit executable. An invalid override is an error, not a silent PATH fallback.
Discovery does not launch the renderer; version is nullable there and recorded
inside the bounded job. Synfig 1.4.5 prints its version with exit status 3.

```sh
mc exec synfig.discover
mc exec animation.list
mc exec synfig.start project=/absolute/Generic.magic \
  source=/absolute/bundle/scene.sif bundle=/absolute/bundle \
  memory_mb=512 timeout_seconds=60
mc exec animation.status project=/absolute/Generic.magic job=<returned-job>
mc exec animation.cancel project=/absolute/Generic.magic job=<returned-job>
```

Optional `first_frame`, `last_frame` are inclusive, nonnegative output frame
numbers. Omitted dimensions, FPS and range use the saved root canvas. Limits:
2–4096 px per axis; FPS (0,120]; at most 18,000 frames and 256 million decoded
pixels. The bundle defaults to the source directory. Keep it separate from the
project job output directory. Bundles are limited to 4096 files / 1 GiB; XML to
16 MiB after decompression. Symlinks, missing or escaping image references,
animated filenames and external canvas links are rejected. `.sfg` archives and
exported subcanvas selection need a future bounded resolver; unpack a native
archive into a self-contained `.sif` bundle for this slice. Fonts and installed
Synfig modules remain external runtime dependencies; they are not claimed as
frozen by the bundle hash. This is not an untrusted-scene sandbox.

## One worker and one receipt protocol

`blender::submit` owns the existing `magicstudio-animation-worker.service`,
nonblocking runtime lock, observed owner correlation and start transaction.
No second scheduler/unit/poller exists. Native jobs use `magicstudio-native-*`
IDs in the historical `animation/blender-jobs` directory; no old paths migrate.
Blender commands remain available. `animation.list/status/cancel` use the same
implementation, including terminal receipt fallback when the user bus is absent.
Start rejects busy, and cancelling remains busy until service exit. Memory caps
are 512–8192 MiB (default 4096), swap zero, CPU 200%, wall timeout 10–3600 seconds
(default 300); whole-cgroup cancellation is asynchronous, stop timeout five seconds.

`native_animation_worker.py` runs Synfig and FFmpeg inside that cgroup. Existing
Blender worker lifecycle helpers validate job/invocation identity and publish the
final terminal telemetry through ExecStopPost. Synfig render completion alone does
not publish success: every expected fresh RGBA PNG is decoded, count/numbering and
dimensions checked, then the VP9 clip is decoded through libvpx to check actual
frame count and retained alpha. `worker_peak_rss_kib` is the Python worker's RSS,
not child or cgroup memory; cgroup metrics remain separate.

The original source is never edited. The job saves an editable source bundle and
`source.json`, `plan.json` (actual argv), `request.json`, `job.json`, `progress.json`,
`rendered.json`, `result.json`, `worker.log`, PNGs and `preview.webm`. Receipts add:

- `backend: "synfig"`, `source_bundle` with frozen entry path/hash, original path,
  dependency relative paths/hashes and digest of the sorted file manifest;
- `tool` path, executable SHA256, version and version exit code;
- inclusive `selected_range`, `authored_range`, FPS, dimensions, `frame_mapping`
  from each native PNG number to source frame, and measured `alpha_evidence`;
- `canvas: [width,height]`, `world_rect: [0,0,width,height]` in output-canvas pixels,
  `clip_seconds`, full lowercase `clip_sha256`; no `.blend` field;
- `partial` when the range omits any saved root-canvas timeline frames.

Placement keeps the same hash, ffprobe, duration/rate and full-frame aspect checks:

```sh
mc exec video.place_track project=/absolute/Generic.magic \
  animation_job=<completed-job> id=native-clip start=0 duration=1 \
  'rect=[0,0,64,48]'
```

The target must already be HyperFrames with an existing composition. `blender_job`
remains an alias; do not combine it with `animation_job` or `path`. Partial clips
are rejected. Placement copies frozen native provenance, never hashes current
source as if it rendered the clip. No Studio UI controls are added in this slice.

## Verification

`pipeline/fixtures/synfig/scene.sif` is a generic 64×48 moving circle, 24 frames
at 24 FPS. Copy its directory to a temporary bundle and create a temporary `.magic`
project for actual native checks. Never use a production song as the fixture.
`python3 -m unittest pipeline/test_native_animation_worker.py` checks source
freezing, dependency escape rejection, fresh sequence completeness and PNG decode.
Run the engine and workspace checks that match the change and report the real result.

Real installed Synfig 1.4.5 verification on 2026-10-23: decoded 24 frames, alpha
0–255 retained in PNGs and clip, first/middle/last circle centroids moved across
frame. Shared service rejected Blender while Synfig was running; cancel returned
in approximately 13 ms, immediately exposed cancelling, then cancelled with worker
and Synfig child PIDs gone. Temporary `video.place_track animation_job` succeeded
with frozen receipt and clip hash. Evidence paths and exact commits are in the
handoff, not production media committed to this repository.

Remaining adapter expansion: `.sfg`, exported canvases, external canvas/font/module
bundling, richer selections, and Studio controls. Statler owns OpenToonz planning
and collection. Its bounded helper still needs explicit native-worker dispatch and
shared encoding integration; this slice does not advertise OpenToonz rendering.

Frozen rerenders use `source_bundle.path` as source and `source_bundle.bundle` as
the actual root, including scenes nested below that root. Do not infer the bundle
from the source parent directory; dependencies may live elsewhere within the root.
