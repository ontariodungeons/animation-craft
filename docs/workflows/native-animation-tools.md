# Native animation source adapters

Proposal only, researched 2026-10-23 against Blender baseline `4a29f11`.
Render already-authored native scenes; do not build a rig evaluator, synchronize
keyframe graphs, or duplicate Godot. Gonzo owns TUI/CLI integration. The planned
Blender actions discovery and global bounded worker slot are dependencies, not
features present in this baseline.

## Evidence and executable availability

Read-only inspection used `command -v synfig opentoonz tcomposer melt`,
`pacman -Q mlt synfig opentoonz`, `pacman -Si synfig opentoonz mlt`, and
`melt -query consumers`. No package database refresh or installation occurred.

| Tool | Observed here | Validation boundary |
|---|---|---|
| Synfig | No PATH executable or installed package; cached repository metadata: `1.4.5-8` | Arguments checked against upstream `v1.4.5`; no local render |
| OpenToonz / tcomposer | Neither executable on PATH; OpenToonz package absent; cached candidate `1.8.0-3` | Arguments checked against upstream `v1.8.0`; package contents and headless startup unverified |
| melt | `/usr/bin/melt`, installed `mlt 7.42.0-1` | Consumer discovery exited 0; no render validated |

Cached package metadata is not a current repository availability guarantee. Record
the actual executable path, package/build version and binary hash at deployment.
Upstream tags inspected resolve to Synfig `f4b8d0455cd7c4186c02275fed69085f60605f67`
and OpenToonz `065cc1404ba43019b22a2577d529134c3e01931b`.

## Source-verified render invocations — untested locally

These are native CLI examples, not implemented `mc` commands. Paths are illustrative;
create a fresh output directory, preserve the source layout, and pass an argv array
rather than interpolating shell input in an adapter.

```sh
synfig -i /bundle/scene.sfg -t png -o /job/frames/frame.png \
  --fps 24 --begin-time 0f --end-time 23f -w 1280 -h 720 -T 2

tcomposer /bundle/scenes/scene.tnz -o /job/frames/frame.png \
  -range 1 24 -step 1 -shrink 1 -nthreads 2
```

Synfig's parser supports these flags, frame-qualified times, `--time` for a single
sample, `--canvas ID` for an exported canvas, and `--canvases` / `--canvas-info`
for discovery. FPS is applied before time parsing. Do not use `--extract-alpha`
as a transparency switch: it extracts the alpha channel. The native renderer owns
sequence filenames; verify numbering, endpoint count and RGBA pixels before mapping
to the common frame manifest. [Synfig parser and render-description handling](https://github.com/synfig/synfig/blob/f4b8d0455cd7c4186c02275fed69085f60605f67/synfig-core/src/tool/optionsprocessor.cpp).

`tcomposer` accepts `-range FROM TO` or `-frame N`; scene frame selection is
one-based. `-nthreads` accepts positive integers or `single`, `half`, `all`.
`-shrink` reduces camera resolution; there are no FPS or width/height override
qualifiers in the inspected parser. Use the saved scene's FPS, camera and output
settings; reject unsupported requested overrides. The implementation uses
`QApplication` and requires a readable stuff directory. `-TOONZROOT /path/to/stuff`
is supported when explicit runtime configuration is needed; isolate its writable
logs/settings per job. A display-free Qt platform and plugin compatibility still
need a real deployment check; do not advertise headless readiness from syntax alone.
[Composer](https://github.com/opentoonz/opentoonz/blob/065cc1404ba43019b22a2577d529134c3e01931b/toonz/sources/tcomposer/tcomposer.cpp),
[range parser](https://github.com/opentoonz/opentoonz/blob/065cc1404ba43019b22a2577d529134c3e01931b/toonz/sources/common/tapptools/tcli.cpp).

## Source bundles and supported controls

- **Synfig:** `.sif` XML or gzip `.sifz` plus referenced assets at resolvable paths;
  prefer `.sfg`, the native archive of scene and assets. Include required fonts and
  identify external modules; an archive does not guarantee runtime dependencies.
  [Native formats](https://wiki.synfig.org/File_Formats).
- **OpenToonz:** `.tnz` plus referenced drawing/image levels, palettes, audio,
  sub-scenes and project folder definitions. Preserve relative paths and project
  aliases (`+drawings`, etc.), with the required stuff/library resources. A lone
  `.tnz` is not a portable bundle. [Project organization](https://opentoonz.readthedocs.io/en/latest/managing_projects.html).

Both adapters render existing authored motion. Neither proposed adapter exposes
bone transforms, IK, Plastic rig authoring, expressions, or arbitrary action edits.
Synfig exported canvases can supply selections; OpenToonz starts with one scene and
explicit named ranges supplied in the handoff. Do not pretend these are Blender
Actions or infer action names from drawings. ToonzScript is documented through the
GUI script menu/console, not a verified headless authoring API.
[ToonzScript](https://opentoonz.readthedocs.io/en/latest/toonzscript.html).

## Minimal shared worker contract

Extend the existing Blender job boundary with backend-specific source validation,
argv construction and frame enumeration. Reuse status, cancellation and artifact
publication; do not introduce a separate scheduler or backend-specific UI protocol.

1. Lightweight discovery reports availability/version, supported controls and
   authored selections without rendering. Unavailable metadata stays unknown.
2. Validate the source bundle and bounded range, then acquire the forthcoming
   **global** worker slot across projects/backends/processes. Report busy rather
   than inventing a queue. Hold the slot through rendering, encoding and teardown.
   Baseline Blender currently limits only one active job per project.
3. Run renderer and existing FFmpeg preview encoding inside the same bounded
   systemd job. Preserve memory/CPU/time limits; cancel the whole process group,
   wait for termination, then release the slot. Startup failure also releases it.
4. Keep `job.json`, `request.json`, `progress.json`, `result.json`, `worker.log`,
   PNG frames and `preview.webm` conventions. Preserve `job`, `state`, `frames`,
   `clip`, `first_frame`, `last_frame`, `fps`; add backend and native source bundle
   path/hash, dependency hashes, tool version, selected range/canvas, actual pixel
   dimensions, frame mapping, alpha evidence and shortened-preview provenance.
   Keep `blend` specific to Blender; do not disguise another source as `.blend`.
5. Publish completion only after successful exit and decoding fresh expected
   frames/clip. Missing assets, gaps, stale output, failed encoding and timeout
   remain failures; cancellation retains partial artifacts without claiming success.
   Reuse existing `video.place_track` only for a validated clip, preserving scene
   placement and distinguishing a shortened preview from a complete action.

Keep originals unchanged; hash a snapshot of the complete dependency bundle so an
in-flight edit cannot silently alter the job. Save native sources for later editing.
The shared lifecycle changes belong with the Blender owner before adapters land.

## MLT findings and licensing caveats

`melt -query consumers` listed `avformat` and `xml`, but emitted duplicate frei0r
registrations and failed to load:

| Module | Missing library |
|---|---|
| `libmltmovit.so` | `libmovit.so.8` |
| `libmltrtaudio.so` | `librtaudio.so.7` |
| `libmltsox.so` | `libsox_ng.so.3` |

These optional-module failures do not prove avformat is broken, nor does discovery
prove it works. Defer MLT to clip/audio assembly after a real decode/render check;
XML serialization is not itself a rendered artifact. Keep existing FFmpeg preview
encoding for the first adapters. [melt CLI](https://www.mltframework.org/docs/melt/),
[MLT XML model](https://www.mltframework.org/docs/mltxml/).

Pin executable versions and retain notices before distributing binaries. Synfig's
cached Arch metadata and parser header say GPL-2.0-or-later, while the tagged
[core LICENSE](https://github.com/synfig/synfig/blob/f4b8d0455cd7c4186c02275fed69085f60605f67/synfig-core/LICENSE)
contains GPLv3: resolve the exact build's licensing rather than assigning one blanket
label. OpenToonz's [BSD-3-Clause license](https://github.com/opentoonz/opentoonz/blob/065cc1404ba43019b22a2577d529134c3e01931b/LICENSE.txt)
explicitly excludes third-party code and brush resources from that blanket grant.
MLT's [core license](https://github.com/mltframework/mlt/blob/v7.42.0/COPYING)
is LGPLv2.1; modules and linked codecs require their own review. Separate-process
integration is an implementation boundary, not a licensing exemption.

## Acceptance before enabling either adapter

Once installation is separately authorized, render a tiny native scene with visible
motion and transparency; inspect first/middle/last frames, decoded count, dimensions,
alpha and playback. Repeat from the saved bundle. Exercise missing dependencies,
invalid controls, cancellation, timeout and competing jobs across projects. Confirm
no stale artifact can produce completion. Perform these checks directly with the
native tools and existing inspection tools; add no custom video-test scripts here.
