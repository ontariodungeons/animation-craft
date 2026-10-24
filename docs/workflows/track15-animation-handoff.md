# Track 15 animation: handoff (Rizzo, 2026-10-24)

Project: `/run/media/jesse/Speedy/media/three-times-around/project/Three Times Around.magic`, open in Rizzo's Studio (pid 1280318, `--control 19971`, workspace 12). Everything editable lives in `project/animation/`; the recipe README there lists each step with the command and its output paths.

## Done and seen in pixels

| Scene | Clip (in the video) | Who moves | Check |
|---|---|---|---|
| 2, 17.72 s | `clips/s2-ship-sails/` → `ship-s2` | ship sails across, rides the waves, flag flutters | gate passes; snapshot 25 s |
| 4, 48.32 s | `clips/s4-captain-never-v2/` → `ship-s4` | finger wag + head shake on each "never", ship stopped | gate passes; snapshot 52 s |
| 6–7, 78.5 s | `clips/s6-7-tidepool-dips-v2/` → `cast-s6-7` (63.3 s, to the end) | captain bends at the hip on "dipped"/"sank our heads", pops up, waves; fish dip with him; crab claws; tails swim | gate passes; snapshots 79.7, 81, 88, 139 s |
| 1, 3, 5 | specs done (`specs/s1…`, `s3…`, `s5…`); Beaker is fixing the turn width and the crab height and re-rendering with the canonical `04` recipe | laps + sandbar bump; captain aboard, leans on "slow", waves, ship turns; ship settles on the sandbar, fish peek on "bottom", crab scuttles | old clips still placed until Beaker's render |

The six scene specs are also project assets `scene_s1_ship_laps` … `scene_s6_7_tidepool_dips` and play in the Animation view (Theatre lists every channel).

## Rigs

- `captain_rig`: 5 parts, stacking legs, body, arm_raised, arm_side, head (`rig.from_sheet order=`). Joint backing is still 0 at the raised shoulder and neck: the sheet drew no artwork under those seams, so the swing is kept small (wave 8°, wag 7°). Closing it needs a redrawn sleeve with a shoulder cap (an artwork request, not a script).
- `sea_friends_rig`: 7 parts from the existing sheet; `places=` fixed the blue tail; tails and claws pivot at their bases.
- No face parts or control bindings on any cast member: no blinks or mouths.

## Lyrics

The video's captions are the sung words (`compositions/captions.html`) to 136.74 s. The words mapping stopped at 102.74 s because the sheet prints verse 5 once; `structure_as_sung` (commit 67d41400) adds the sung repeat (verse 5 lines 1–2 at 116.3–136.74 s) without changing printed text. The running Studio predates it; `words.align` covers the ending after its next build.

## Not done / requests

- Final full render waits for Beaker's scene 1/3/5 clips: `hyperframes render <project>/hyperframes -o /run/media/jesse/Speedy/media/three-times-around/renders/three-times-around-v1.mp4`.
- mc adapters requested (Beaker/Fozzie): `video.snapshot at=[s…] out_dir=<dir>` → `{frames:[path]}` and `video.render out=<mp4>` → `{file, duration, receipt}` over the installed HyperFrames CLI.
- Blender and pose-replacement comparisons of the song cast were not run; `animation.source.from_rig` does not exist and the existing Blender bridge builds a fixed humanoid (gap per `track15-animation-sequence.md`).
- Seen in the Studio: setting the Animation object picker over the bridge needed a relaunch once to show the new object; the Layers guard needed the specs inside the project (now moved).
