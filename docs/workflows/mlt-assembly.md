# Bounded MLT clip/audio assembly (#23)

`mlt.discover {}` reports melt availability/path, FFmpeg/ffprobe paths and configured
status. Native consumer/codec capability is deliberately unknown until the bounded
job checks and uses it; availability is not render success. `MLT_MELT` may choose
an executable. Install MLT (`melt`), FFmpeg/ffprobe with H264/AAC support.

`mlt.start {timeline:"/absolute/timeline.json",memory_mb:4096,timeout_seconds:300}`
returns the shared submitted job. Existing `animation.list/status/cancel`, global
owner correlation, lock, resource limits, terminal telemetry and cancellation apply.
No parallel scheduler/editor is introduced. HyperFrames remains the film compositor.

Minimal generic input (all in/out values are inclusive timeline-profile frames):

```json
{
  "width": 64,
  "height": 48,
  "fps": [24, 1],
  "clips": [
    {"path": "red.mp4", "in": 0, "out": 11},
    {"path": "blue.mp4", "in": 0, "out": 11}
  ],
  "audio_mode": "replace",
  "audio": {"path": "tone.wav", "in": 0, "out": 23, "start": 0}
}
```

Clips concatenate with hard cuts, in given order. Local paths resolve relative to
the timeline file. Input video FPS must match the rational timeline profile; no
implicit rate conversion. Without `audio`, embedded clip audio is retained. With
`audio`, explicit `audio_mode:"replace"` is required: one replacement bed, optionally
starting after a blank; embedded clip audio is disabled. No implicit trimming past
measured media duration or the assembled timeline is permitted. A bed may be shorter
than the picture. No mixing/gain automation, transitions, arbitrary XML input,
captions, multilayer composition or alpha export is advertised by this slice.

Bounds: <=1MiB JSON,1–256clips, even dimensions2–4096,1–120fps,18000output frames,
256million output pixels,<=1GiB total local source media. Scene plugins/network
resources cannot be supplied through the timeline schema. Every referenced file is
copied and hashed inside the bounded worker; original media/timeline remain unchanged.

The worker writes `source-bundle/original-timeline.json`, frozen media, rerunnable
`source-bundle/timeline.json`, a native editable `timeline.mlt`, explicit `profile`,
`plan.json` with exact argv, and `assembly.mp4`. MLT's avformat consumer produces
H264/yuv420p picture and AAC audio. Completion requires native exit0, exact decoded
frame count/dimensions/rate, bounded duration agreement, requested audio presence,
full FFmpeg decoding, and unchanged frozen input hashes.

Receipt: backend/job/state/clip/clip_sha256/clip_seconds/fps/fps_fraction,
first_frame0/last_frame, canvas/world_rect in output pixels, partial:false,
source_bundle.{path,bundle,original_path,sha256,dependencies}, native_project and
its hash, tool path/hash/version, audio_mode, probe evidence, explicit opaque
alpha_evidence, plus shared final telemetry. Rerender with source_bundle.path; the
frozen JSON references its exact copied media. No original source file is rehashed
as if it rendered a later clip.

Existing `video.place_track animation_job=<job>` verifies/copies the picture and
frozen receipt. That placement remains MUTED picture placement: it does not route
the assembled audio into HyperFrames. Fozzie/Bunsen's film-composition integration
must handle audio deliberately; this adapter does not change the current film audio.

## Actual installed verification

On2026-10-23, melt7.42.0-1 listed avformat but warned about missing optional
libmovit.so.8, librtaudio.so.7, libsox_ng.so.3 and duplicate frei0r registrations.
Those warnings are retained in job logs; they did not prevent this avformat export.
A single real shared-worker job assembled two synthetic12-frame video clips and a
440Hz audio bed at64x48/24fps. Independently decoded24frames showed red through11,
blue from12; decoded audio RMS0.088 (non-silent). Full clip hash checked. Cgroup peak
137822208bytes under512MiB, swap0, CPU200%, timeout60s; service exited successfully
and the shared worker became idle. Evidence `/tmp/waldorf-mlt-native/`.
No production project, live port, film, transcript or media was touched. No new
MLT-specific cancellation/timeout/OOM stress test was run; shared lifecycle retained.

Native format and consumer references:
[MLT XML](https://www.mltframework.org/docs/mltxml/),
[avformat consumer](https://www.mltframework.org/plugins/ConsumerAvformat/).
Generic compiler regression tests run with
`python3 -m unittest pipeline/test_mlt_animation_worker.py`. Run the Rust tests that
match the change and report the real result. This is a reusable assembly/export path, not a replacement editor.
