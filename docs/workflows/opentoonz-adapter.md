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
