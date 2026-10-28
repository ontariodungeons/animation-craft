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

The video's captions today are the heard words in batches of five (`compositions/captions.html`), e.g. "We sank our heads" where the sheet prints "Then we dipped our heads". Two commits fix this without changing printed text:
- 67d41400 `structure_as_sung` (written into the sheet): the sung repeat of verse 5 lines 1–2 at 116.3–136.74 s, so the mapping no longer stops at 102.74 s.
- e5bc1512 `words.captions to=video`: rewrites the caption track from the printed lines with sung timing, one group per line (long lines in halves), keeps the old source as `captions.v1.html`.

The running Studio (pid 1280318) predates both. After it runs this build: `mc --bridge 19971 exec words.captions to=video`, then snapshot 20, 52, 120 s.

## Not done / requests

- Final full render waits for Beaker's scene 1/3/5 clips: `hyperframes render <project>/hyperframes -o /run/media/jesse/Speedy/media/three-times-around/renders/three-times-around-v1.mp4`.
- mc adapters requested (Beaker/Fozzie): `video.snapshot at=[s…] out_dir=<dir>` → `{frames:[path]}` and `video.render out=<mp4>` → `{file, duration, receipt}` over the installed HyperFrames CLI.
- Blender and pose-replacement comparisons of the song cast were not run; `animation.source.from_rig` does not exist and the existing Blender bridge builds a fixed humanoid (gap per `track15-animation-sequence.md`).
- Seen in the Studio: setting the Animation object picker over the bridge needed a relaunch once to show the new object; the Layers guard needed the specs inside the project (now moved).

## Lyrics panel (Rizzo owns words.js, Words CSS, words engine)

Committed, not yet seen in the combined Studio:
- 9937ce4c: the left column shows each sung line in its own lettering (the line design's font, weight, slant, colour and size) on paper; click a word to pick it, Shift-click adds or drops words in that line; Look edits (lettering, colour, outline, size, tilt) apply to every picked word. Stage about two thirds of the width; film roll wider. Text stays the sheet's. Test: `apps/magicstudio-shell/tests/words-lettering.mjs`.
- 67d41400 + sheet edit: the sung ending repeat is its own section (verse 5, second time, lines 1–2), so it gets its own design and words in the panel.
- e5bc1512: `words.captions to=video` puts the printed lines in the video's caption track.

When the combined Studio is up: open Design, check the column on Three Times Around (seven sections now), style a few words, Save, then `mc --bridge 19971 exec words.captions to=video` and snapshot 20, 52, 120 s.

## Lyrics designed (Jesse's task)

`mc exec words.design` designed and used all 26 sung lines (24 printed + the 2-line ending repeat) from `project/lyrics-design.json`; recipe `animation/recipe/06-design-lyrics.jsonl`. The caption track was removed (every section designed), so each line shows once. Checked in snapshots at 3.5, 14.6, 32, 52, 65, 80, 100.5, 118 and 130 s (`animation/preview/lyrics-check-2/sheet.jpg`). The video index before this is kept in `animation/recipe/history/index.before-words-design.html`. Change the art direction in the recipe and run the command again; adjust single lines in Design.

## Kinetic lyrics and the Layers panel (2026-10-24 evening)

- Settings file: `pipeline/templates/kinetic-library.json` (14 acts, 6 entrances, 6 layouts, 8 overlays: registry blocks with blend and opacity). The song's choices: the `kinetic` block in `project/lyrics-design.json` (meaning word → act, the layouts, entrances and overlays to vary through, lettering).
- Commands: `words.track` (the lyric line: one anchor clip per sung word), `words.design seed= layout= entrance= overlay= section=` (kinetic sections), `words.save kinetic=…`, `words.apply` (mounts the section and its overlay).
- Design page: a kinetic section shows Layers under the stage (Layout, Words arrive, Meaning words with acts, Overlay), Shuffle and Static lines.
- Track 15: all seven sections kinetic (frames in `animation/preview/kinetic-check-1/`); overlays were mid-run when the Studio on 19971 (shared-target build, pid 1721943) took the project. With this branch (1660df5f) in that build: `mc --bridge 19971 exec words.track`, then `mc --bridge 19971 exec words.design seed=0`, then snapshot.
