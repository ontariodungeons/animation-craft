# OpenToonz source adapter

Renders an already-authored OpenToonz scene (`.tnz`) to PNG frames with `tcomposer`.
It does not author rigs, bones, Plastic deformation or actions, and it has no scheduler:
the shared bounded animation worker (owned with the Blender worker) runs it and owns
limits, progress, cancellation and receipts. Design background and acceptance gates:
[native animation tools](native-animation-tools.md).

## Commands

```text
mc exec opentoonz.discover                                   # tool availability, no rendering
mc exec opentoonz.discover scene=/abs/project/scenes/a.tnz   # plus the exact render plan
mc exec opentoonz.render scene=/abs/project/scenes/a.tnz out=/abs/job/frames first=1 last=24
```

- `opentoonz.discover` reports the `tcomposer` path and SHA-256, the `stuff` folder,
  supported controls, and (with `scene`) the validated plan: exact argv, one-based
  range, expected frame numbers, FPS, camera resolution and the hashed source bundle.
- `opentoonz.render` is blocking. It requires a new or empty absolute `out` folder,
  runs `tcomposer`, then maps every rendered file to its scene frame. Missing frames,
  extra files, mixed sizes or a camera-size mismatch fail. The receipt carries `backend`,
  tool path/hash, source bundle path/hash plus dependency hashes, range, FPS, actual
  width/height, `frame_mapping`, `first_frame`/`last_frame` and alpha-channel evidence.
  It never writes `blend`.
- `opentoonz.render request=<job>/request.json` is worker mode. It runs only inside the
  shared worker invocation for that job (`blender::require_worker_invocation`, the same
  check `blender.prepare` uses), reads `scene`/`first`/`last`/`step`/`shrink`/`nthreads`
  from the request, renders into `<job>/frames` and writes `<job>/opentoonz-manifest.json`:
  ordered absolute PNG paths, frame mapping, first/last frame, FPS, width/height, alpha
  evidence, tool hash and the frozen source bundle.

## Rules taken from OpenToonz source (`065cc14`)

- `first`/`last` are one-based scene frames; a scene's stored output range is zero-based
  and `0 -1` means the whole scene. Without `first`/`last` the scene's output range is used.
- `tcomposer` has no FPS, width or height override. A request with different values fails;
  change the scene's output settings in OpenToonz. `shrink` (1-16) is the only size control.
- Files are named `frame.NNNN.png` by default; a scene can change padding/separator, so
  both `.` and `_` with any padding are accepted when mapping frames.
- Exit status: 1 bad arguments, 2 out of memory, -2 scene/project not loaded,
  -1 at least one frame failed. A signal usually means an undefined `TOONZROOT`.

## Source bundle

The `.tnz` plus every level its `levelSet` references. `+drawings`-style aliases resolve
against the project folder (the nearest folder with `*_otprj.xml` or `scenes.xml`) using
the default folder names; a project file that renames them is reported as unresolved
rather than guessed. `name..png` sequences and `.tpl` palettes next to `.tlv` levels are
included. Every file is hashed; any missing level fails before rendering.

## Runtime

- `tcomposer`: `OPENTOONZ_TCOMPOSER` (absolute file) or `PATH`. Nothing is installed.
- `stuff` folder: `OPENTOONZ_STUFF`, then `/usr/share/opentoonz/stuff`,
  `/opt/opentoonz/stuff`, `/usr/local/share/opentoonz/stuff`,
  `~/.config/OpenToonz/stuff`; passed as `-TOONZROOT`.
- `QT_QPA_PLATFORM=offscreen` unless already set. Headless startup is unverified until
  OpenToonz is installed and the acceptance checks in the design doc pass.

As of 2026-10-23 neither `opentoonz` nor `tcomposer` is installed on the build machine,
so the adapter reports `available: false` with that reason; no render has run here.

## Shared start / frozen-source clip completion (#23)

`opentoonz.start {scene,bundle?,first?,last?,step?,shrink?,nthreads?,memory_mb?,timeout_seconds?}`
submits to the same `magicstudio-animation-worker.service` as Blender and Synfig.
Use `animation.list/status/cancel`; busy rejects and cancelling remains busy until
exit. `opentoonz.render request=<request.json>` is worker-only; direct unbounded
render invocation is now refused. No fps/width/height override is exposed by start.

Discovery separates executable `available` from `render_capable`/`render_reason`
(tool and stuff resources required). This is configured capability, not a claim
that this machine has passed a native render. As of this slice tcomposer and the
OpenToonz package are absent: install OpenToonz's tcomposer and stuff resources,
or configure OPENTOONZ_TCOMPOSER and OPENTOONZ_STUFF. FFmpeg/libvpx and Python Pillow
are also required by the shared encoder. No install was attempted.

Start defaults bundle to the detected native project root (else scene directory).
The bounded worker copies a self-contained bundle (<=1GiB/4096files, no symlinks),
retaining the native project layout, then copies stuff resources into job-local
storage and forces offscreen Qt. Absolute/escaping resource links are refused;
prepared dependency paths must remain in the frozen bundle. The source bundle is
checked unchanged after rendering. Arbitrary plugins, custom external project
aliases/fonts or scene portability are not claimed. Native validation is pending
installation; failures retain logs, never a completed clip.

The helper's ordered frame manifest is fully decoded as RGBA before shared VP9
encoding; the encoded clip is decoded again for exact frame count and retained
alpha. `step>1` holds each rendered frame through the skipped scene frames (last
hold clipped to requested last), preserving original FPS and requested duration.
`frame_mapping` records held source scene frames. `partial` is true when range
omits any of the saved scene timeline. Size/shrink comes from native output.

Completed receipts use backend/job/state/clip/clip_sha256/clip_seconds/fps,
first_frame/last_frame, canvas/world_rect (output pixel units), source_bundle with
frozen path/hash/dependency manifest and runtime-stuff inputs, tool identity,
selected_range, alpha_evidence, and shared final telemetry. No fake character or
action IDs are required. `video.place_track animation_job=<id>` uses the existing
hash/ffprobe/full-range/frame-aspect checks and frozen provenance; partial clips
are refused. HyperFrames remains the compositor.

Rerender from `source_bundle.path`, passing its frozen bundle directory as `bundle`;
choose the scene's desired range again. Original input paths are retained only as
provenance. This does not overwrite old jobs or original native files.

Frozen rerender root is now explicit: `source_bundle.bundle` is the actual frozen
bundle directory, while `source_bundle.path` is its scene entry point (possibly
nested under scenes/). Pass both verbatim; never infer bundle from path.parent.
The same fields are supplied by the Synfig snapshot receipt.

### Readiness preflight

`render_capable` requires an executable renderer, a readable nonempty stuff directory,
executable adjacent `mc`, FFmpeg on PATH, and a successful Pillow import in the worker's
`/usr/bin/python3`. Invalid explicit renderer/resource overrides never fall back to
another installation. Start uses this same preflight and pins resolved tool paths.
Discovery does not render; `native_render_verified:null` explicitly separates this
prerequisite check from native source/plugin compatibility. `prerequisites` reports
mc/ffmpeg paths and Python/Pillow checked/available/error; Python is unprobed/null
when another prerequisite is already missing. Its import-only probe is limited to
five seconds. Current native jobs remain the authority for actual render success.

### Native loader readiness

Discovery executes `tcomposer -version` with a five-second deadline, no scene, closed
stdin and a nonblocking, 4 KiB capped stderr read. OpenToonz 1.8 returns exit 1
and writes `OpenToonz v...` to stderr; only that identity with exit 0 or 1 is
accepted. A failed launch, unexpected output/exit or timeout prevents
`render_capable`; `available` still describes the executable file. The new
`prerequisites.renderer` reports `checked`, nullable `available`, `error` and
`private_library_dir`, independently of `resources`, FFmpeg and Python/Pillow.
A successful version probe does not claim native rendering or codec verification.

The Arch package's launcher prepends `<prefix>/lib/opentoonz` to
`LD_LIBRARY_PATH`. When that directory contains `libtfarm.so`, discovery and the
render child use that same package layout, preserving inherited library search
paths after it. Only those children receive the environment; Studio, the worker
and the host configuration are not modified. Both children use offscreen Qt.


Native dependency discovery decodes the quoted filename strings written by
OpenToonz (`"+drawings/A B.png"`, including native quote/backslash escapes) before
resolving and hashing them. Source bytes remain unchanged. This metadata support
does not constitute successful native-render acceptance.


Portable native scenes may use `$scenefolder` and backslash separators; both are
resolved against the frozen scene directory. OpenToonz TIStream strings permit
literal ampersands, including stock compression labels. The worker escapes those
only in its XML-parser view; it never rewrites the native source bytes. External
entity/DOCTYPE declarations remain refused. The official `opentoonz_sample`
`tga_paint.tnz` is the nonempty source used for the bounded native acceptance check.
