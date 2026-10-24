# THE RENDER WARS · Ep. 02: THE HARD WAY — Production Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce the 4:50, 15-chapter all-desktop comedy film from Sol's script as a reusable FilmCraft project whose every credited technique has a native source document and a final timecode, and which visibly beats the HyperFrames champion (299 s, 15 tracks, 25 credited skills).

**Architecture:** One production spec (`production.json`: chapters → shots → dialogue lines → technique IDs) drives everything. Generic pipeline modules in `omhas-animation/pipeline/production/` turn that spec into Voicebox dialogue, an animatic, and the final FilmCraft timeline, and they check the evidence ledger. Each is exposed as an `mc` command. Creative work happens in the native apps (one Blender soundstage `.blend` holds all of the 3D, plus a native document per craft app), and their renders land in `renders/<chapter>/`. Structure comes before polish: the animatic is cut first, and shots replace their animatic placeholders one at a time.

**Tech Stack:** Python 3.12 + pytest (pipeline), Blender 5.3 headless (`blender -b --python`), Voicebox HTTP `127.0.0.1:17600`, filmcraft-cli, effectcraft-cli, photocraft-cli, vectorcraft-cli, soundcraft-cli, lightcraft-cli, cadcraft-cli, designcraft-cli, pdfcraft-cli, gridcraft-cli, deckcraft-cli, wordcraft-cli, figcraft (MCP), mc, HyperFrames CLI (opponent shots only), ffmpeg/ffprobe.

**Spec:** `PROJECT/SCRIPT.md` (Sol's script, copied from `/tmp/claude-1000/.../scratchpad/sol-script-the-hard-way.md`) and `PROJECT/LEDGER_TARGET.md` (copied from `/tmp/codex-roast-review/competition-count.md`).

`PROJECT` = `/home/jesse/Work/merriweather-videos/projects/render-wars-ep02-hard-way`

## Global Constraints

- Episode 02, title "THE RENDER WARS — Episode 02: THE HARD WAY"; 4:50 target, 15 chapters; 1920×1080, 30 fps, 48 kHz.
- Cast: SOL (vain desktop impresario), HYPE (reigning HyperFrames champion, stays strong), REMY (Remotion), JESSE (off-screen). SOL must not lose. The comedy mocks the frameworks, not their dialogue.
- No claims that browsers *can't* do something. Beat their produced result visibly instead.
- Champion shots (HYPE's title, rain, curtain) are real HyperFrames marketplace renders at full quality, never sabotaged.
- Continuity: one gold trophy, one velvet curtain, one overhead water tank. Damage persists across chapters.
- Earned count only: a credit needs (a) a native source doc/setup, (b) a final timecode, and (c) a distinct operation. Opening an app or installing a skill earns 0. Closing card shows **achieved** counts (target 50 techniques · 17 tools · 15 chapters).
- Ownership: Sol owns `/tmp/episode2-sol-assets` (read only for us). We own `PROJECT`. Copy assets in; never write to Sol's dir.
- Pipeline code is generic (no character or episode names in code or tests). Episode content lives only in `PROJECT`.
- Use the craft CLIs, not hand-rolled substitutes. Never hand-patch rendered frames: fix the script/source and re-render.
- Commit at every pause (merriweather: local commits only, **never push**; omhas-animation: commit on a branch).
- Voicebox profiles: HYPE `e254ca33-5d1c-41a8-8351-699248069ccb`, REMY `212d6007-a716-4ac9-9f87-781bcb509aab`, Announcer `9bbf373d-6b4d-4812-9b16-ae11ace60feb`. SOL and JESSE get new or other existing profiles, picked in Task 2 by audition.
- Hardware: GTX 1660 6 GB. Use EEVEE for coverage and Cycles only for the hero shots in chapters 2, 14 and 15. Bake simulations to disk caches inside `PROJECT/blender/cache/`.

## Review Focus

1. **A ledger entry whose source exists but never appears on screen.** `ledger_check` must fail when the timecode falls outside the edited chapter or the shot was cut from the timeline.
2. **Re-generated dialogue changes duration.** The animatic/timeline must re-flow from measured WAV durations, not stale numbers; the test covers a changed duration shifting later shots.
3. **Missing or offline media in FilmCraft.** The timeline build must fail loudly listing missing paths (no `--allow-offline` export of the final).
4. **Sim caches out of sync with the edit.** If `stage.blend` frame ranges change after baking, the render step must refuse to render from a stale cache (cache frame range ≠ shot range → error).
5. **Champion comparison fairness.** Split-screen shots must use HYPE renders at native 1080p. The check fails if an opponent clip is upscaled from less than 1080 lines.

---

### Task 1: Project scaffold, production spec, ledger checker

**Files:**
- Create: `PROJECT/SCRIPT.md`, `PROJECT/LEDGER_TARGET.md` (copies), `PROJECT/production.json`, `PROJECT/README.md`
- Create: `omhas-animation/pipeline/production/__init__.py`, `spec.py`, `ledger.py`
- Test: `omhas-animation/tests/test_production_spec.py`, `tests/test_production_ledger.py`

**Interfaces:**
- Produces: `load_spec(path: str) -> Spec` (dataclasses `Spec{title, fps, width, height, chapters: list[Chapter]}`, `Chapter{id: str, title, lesson, shots: list[Shot]}`, `Shot{id, duration_s: float, kind: "blender"|"app_insert"|"opponent"|"composite", source: str, lines: list[Line], techniques: list[str]}`, `Line{id, speaker, text, wav: str|None, duration_s: float|None}`); `check_ledger(spec: Spec, ledger_csv: str, timeline: dict) -> LedgerReport{earned: int, tools: int, failures: list[str]}`. Ledger CSV columns: `technique_id,technique,app,source,shot_id,timecode,status`.

- [ ] **Step 1:** Create `PROJECT`, copy SCRIPT.md and LEDGER_TARGET.md, and write `production.json` with all 15 chapters, shots, every dialogue line verbatim from SCRIPT.md, and techniques 1–50 assigned to shots as in LEDGER_TARGET.md's table.
- [ ] **Step 2:** Write failing tests:
  - `test_load_spec_counts`: a fixture spec with 2 chapters and 3 shots loads, and `sum(shot.duration_s)` equals the fixture total.
  - `test_ledger_fails_missing_source`
  - `test_ledger_fails_timecode_outside_shot` (Review Focus 1)
  - `test_ledger_fails_cut_shot`: a shot absent from the timeline dict causes a failure.
  - `test_ledger_counts_reuse_once`: the same technique_id on two shots counts as 1.
  - `test_ledger_counts_tools_distinct`
- [ ] **Step 3:** Run `pytest tests/test_production_spec.py tests/test_production_ledger.py -v` and confirm it FAILS (module missing).
- [ ] **Step 4:** Implement `spec.py` and `ledger.py`.
- [ ] **Step 5:** Run the same command and confirm it PASSES. Then run `python -m pipeline.production.spec PROJECT/production.json` and confirm it prints `15 chapters, N shots, 50 techniques, 290±10 s`.
- [ ] **Step 6:** Commit both repos (`feat(production): spec + evidence ledger checker` / `chore(ep02): scaffold project and production spec`).

### Task 2: Voice casting and dialogue generation

**Files:**
- Create: `pipeline/production/voice.py`, `PROJECT/audio/vo/<line_id>.wav`, `PROJECT/audio/vo/casting.md`
- Test: `tests/test_production_voice.py`

**Interfaces:**
- Consumes: `load_spec`.
- Produces: `generate_lines(spec_path: str, out_dir: str, profiles: dict[str,str], only: list[str]|None=None) -> list[Line]`. It writes the WAVs, updates `production.json` with `wav` and measured `duration_s` (from ffprobe), and pads each shot's `duration_s` to at least (sum of its line durations + 0.4 s per line).

- [ ] **Step 1:** Write failing tests with Voicebox mocked by an HTTP stub:
  - `test_generate_writes_wav_and_measures_duration`
  - `test_shot_duration_grows_when_line_longer` (Review Focus 2)
  - `test_unknown_speaker_raises`
- [ ] **Step 2:** Run the tests and confirm they FAIL.
- [ ] **Step 3:** Implement using the POST `/generate` → poll `/history/{id}` → GET `/audio/{id}` flow from `showdown-remotion/scripts/generate_voice2.py`.
- [ ] **Step 4:** Run the tests and confirm they PASS.
- [ ] **Step 5:** Audition SOL candidates. List profiles with `curl 127.0.0.1:17600/profiles`, render SOL's line "I said good. Let's not get married." in 3 candidates and JESSE's "Beat it. Use the whole desktop." in 2, then record the picks in `casting.md`. **Ask Jesse to pick SOL's voice.**
- [ ] **Step 6:** Generate all lines. Verify every line has a WAV with `duration_s > 0`, then listen to 5 punchlines.
- [ ] **Step 7:** Commit.

### Task 3: Animatic in FilmCraft (structure before polish)

**Files:**
- Create: `pipeline/production/timeline.py`, `PROJECT/boards/<shot_id>.png`, `PROJECT/edit/ep02.fcproj`, `PROJECT/edit/animatic.mp4`
- Test: `tests/test_production_timeline.py`

**Interfaces:**
- Consumes: Spec with measured line durations.
- Produces: `build_timeline(spec_path: str, project_out: str, media_for_shot: Callable[[Shot], str]) -> dict` (returns `{shot_id: {"start_s", "end_s", "track"}}`). It runs `filmcraft-cli run <jsonl>`. Named tracks: V1 PLATES, V2 CHARACTERS, V3 APP INSERTS, V4 COMPOSITES, V5 LABELS, C1 CAPTIONS, A1 DIALOGUE, A2 MUSIC, A3 FOLEY, A4 AMBIENCE. There is one sequence marker per chapter, named after the chapter and holding its lesson as the comment. `build_timeline` raises `MissingMedia([paths])` before touching FilmCraft (Review Focus 3).

- [ ] **Step 1:** Write failing tests against a 3-shot fixture spec, with filmcraft-cli replaced by a recorder that captures the JSONL:
  - `test_shots_laid_end_to_end_from_durations`
  - `test_dialogue_on_A1_at_shot_start_plus_offsets`
  - `test_chapter_markers_emitted`
  - `test_missing_media_raises_before_cli`
- [ ] **Step 2:** Run the tests and confirm they FAIL.
- [ ] **Step 3:** Implement. Discover exact command ids with `filmcraft-cli commands marker`, `commands track` and `commands caption` first, and pin those ids in module constants.
- [ ] **Step 4:** Run the tests and confirm they PASS.
- [ ] **Step 5:** Boards. Make one board per shot from Sol's art (copied into `PROJECT/assets/sol/`), composited with photocraft-cli (plate + characters + shot label). Chapters 1, 6 and 14 get boarded together first, per Sol.
- [ ] **Step 6:** Build the real animatic: `python -m pipeline.production.timeline PROJECT/production.json --boards` then `filmcraft-cli --project PROJECT/edit/ep02.fcproj export PROJECT/edit/animatic.mp4 --scale 0.5`. Verify the duration is 290±10 s with ffprobe, and confirm by eye that every punchline has its pause.
- [ ] **Step 7:** **Checkpoint: send `animatic.mp4` to Jesse and Sol for timing notes before any 3D polish.** Commit.

### Task 4: Blender soundstage (one world)

**Files:**
- Create: `PROJECT/blender/build_stage.py`, `PROJECT/blender/stage.blend`, `PROJECT/blender/cameras.json`

**Interfaces:**
- Produces: in `stage.blend`, these objects (exact names, used by later tasks): `SET_Arena_HYPE` (HYPE's recognizable theatrical arena, rebuilt from his stills), `SET_Soundstage`, `PROP_Trophy`, `PROP_Curtain`, `PROP_Tank`, `PROP_ReleaseLever`, `PROP_InsurancePlacard`, `CHAR_SOL`, `CHAR_HYPE`, `CHAR_REMY` (character cards from Sol's cutouts, or meshes once Task 6 lands), `LIGHT_Key`, `LIGHT_Fill`, `LIGHT_Rim`, and one camera `CAM_<shot_id>` per Blender shot with its frame range in `cameras.json`.

- [ ] **Step 1:** Write `build_stage.py`. It is deterministic and idempotent (it rebuilds from scratch) and is applied with the skills blender-director, environment-artist, set-dressing, materials and lighting. The trophy is a beveled gold mesh with a PBR metal material.
- [ ] **Step 2:** Run `blender -b --python PROJECT/blender/build_stage.py -- --out PROJECT/blender/stage.blend`.
- [ ] **Step 3:** Render one EEVEE still per camera at 50%. Review each still and fix the script (not the frames) until the set reads as "lavish practical soundstage around a small theatrical arena".
- [ ] **Step 4:** Chapter 1's dolly, occlusion and focus pull: animate `CAM_01a` with depth of field focused from the card to SOL. Render it to `renders/01/` and swap it into the animatic. **Ledger 1–3.**
- [ ] **Step 5:** Chapter 2's title: build the beveled extruded mesh title plus HYPE's reflection in the curved gold. Make a Cycles hero render. **Ledger 4–6.**
- [ ] **Step 6:** Commit.

### Task 5: Connected physics sequence (chapters 3, 4, 6, 14)

**Files:**
- Create: `PROJECT/blender/sim_fracture.py`, `sim_cloth.py`, `sim_fluid.py`, `render_shots.py`; caches go in `PROJECT/blender/cache/{fracture,cloth,fluid}/`

**Interfaces:**
- Consumes: Task 4 object names.
- Produces: `render_shots.py --shots 04a,14a ... --engine eevee|cycles` renders to `renders/<chapter>/<shot_id>/####.png` plus passes (`_depth`, `_glow`, `_beauty`) for chapter 14. It **errors if any cache's frame range ≠ the shot range** (Review Focus 4).

- [ ] **Step 1:** Fracture (chapter 4) — skills physics-sim, vfx-fx:
  - Voronoi-split `PROP_Trophy` into 12 pieces (12 is a script fact) using geometry nodes or bmesh.
  - Set up rigid-body contacts with the floor, set and placard, and bake.
  - One fragment must strike `PROP_ReleaseLever` and leave it displaced. That end state persists for chapter 6.
  - Render the shot. **Ledger 10–11.**
- [ ] **Step 2:** Rain and wet (chapter 3):
  - Tank-pour fluid (Mantaflow, low-res domain sized to the trophy area), baked.
  - The wet-material shader is animated on HYPE's paper jacket (roughness and darkening driven by a wetness value).
  - Render. **Ledger 7–8.**
- [ ] **Step 3:** Cloth (chapter 6):
  - Velvet curtain with folds and a stitched hem.
  - Collision with the HYPE body proxy and the puddle plane, with the release triggered by the lever state.
  - Bake and render, including the retimed close-up insert of the snag. **Ledger 16–18.**
- [ ] **Step 4:** Finale (chapter 14), one connected bake:
  - Tank tips, water hits the curtain, the curtain bows and funnels water into the broken trophy, and the fragments skitter.
  - Render passes: beauty, depth and glow. Check ten evenly spaced frames: the interaction must be continuous with no cutaway hiding a break.
- [ ] **Step 5:** Commit after each bake that's visually verified (caches tracked with git LFS or excluded: decide by size, and keep `bake.sh` reproducible).

### Task 6: Characters and performance

**Files:**
- Create: `PROJECT/characters/<NAME>/` (cutout rig `.magic` from Sol's turnarounds), `PROJECT/blender/remy_body.py`

**Interfaces:**
- Consumes: Sol's manifest (turnarounds, transparency flags).
- Produces: Magic Studio cutout rigs exported with `mc exec export.layered_motion` for 2D performance, and `CHAR_REMY_Body` (rigged mesh) in `stage.blend`.

- [ ] **Step 1:** Import each character with `mc` (`asset.cut_sheet` → `asset.build_cutout_rig`) and validate it with `assets.assess`. Likeness must match the turnarounds (check with `intake.compare_reference`).
- [ ] **Step 2:** Pose library per character (pose-library-design): neutral, smug, recoil, drenched, conduct, point. Check silhouettes for readability at 25% scale.
- [ ] **Step 3:** HYPE's drenched contact performance (chapter 3), where the weight shift reads. **Ledger 9.**
- [ ] **Step 4:** REMY's body (chapter 5) — skills character-artist, rigging:
  - Articulated hands, with a CHILD_OF constraint to the trophy fragment for the pickup.
  - Anticipation and reaction poses.
  - Render. **Ledger 13–15.**
- [ ] **Step 5:** Check with character-animation-qa (frame samples per shot) and commit.

### Task 7: Native app inserts (chapters 7–13)

Each insert keeps its native document in `PROJECT/apps/<app>/` and its render in `renders/<chapter>/`, and each is verified by viewing the render.

- [ ] **Step 1:** Lighting (chapter 7): key, fill and rim in `stage.blend` plus an atmosphere volume. LightCraft develops HYPE's and REMY's publicity stills to a matched exposure and white balance (`lightcraft-cli run --import ... develop.set ... app.export`). **Ledger 19–21.**
- [ ] **Step 2:** Publicity (chapter 8):
  - Sol-generated poster elements go into a layered `.pcraft` with masks, plus a retouch layer that can be toggled.
  - Export the layers and run a parallax move in EffectCraft.
  - **Ledger 22–25.**
- [ ] **Step 3:** Branding (chapter 9):
  - Exploded-trophy vector diagram in VectorCraft, with its path/stroke draw-on animated in EffectCraft.
  - Souvenir brochure in DesignCraft ("collect all twelve pieces").
  - Review PDF in PDFCraft with the "REMOVE FROM PERFORMER" annotation.
  - **Ledger 26–29.**
- [ ] **Step 4:** CAD (chapter 10): dimensioned plan of the actual stage (dimensions from `stage.blend`), a sightline drawing, and an animated section reveal of the dressing room. **Ledger 30–32.**
- [ ] **Step 5:** Musical (chapter 11):
  - Original cue arranged in SoundCraft, with SOL's sung phrases recorded as separate Voicebox clips and edited to the beat.
  - Panned effects, and music automation ducking under the dialogue.
  - **Ledger 33–37.**
- [ ] **Step 6:** Bureaucracy (chapter 12):
  - GridCraft budget with real formulas (`gridcraft-cli check` passes). Its numbers are measured from the project (shot count, render minutes logged) or explicitly comic.
  - DeckCraft beat board.
  - WordCraft paper edit with tracked changes.
  - **Ledger 38–40.**
- [ ] **Step 7:** Control panel (chapter 13):
  - FigCraft interface (velvet panels, brass faders, the MAKE IT POP button and the two confirmation dialogs) with an interaction sequence exported as frames.
  - Tracked screen replacement in EffectCraft.
  - **Ledger 41–43.**
- [ ] **Step 8:** Commit after each step.

### Task 8: Champion (opponent) shots

**Files:** `PROJECT/opponent/hf/` (a HyperFrames project using marketplace blocks), `PROJECT/opponent/remotion/` (clips cut from `showdown-part2-remy.mp4`)

- [ ] **Step 1:** Install and render HYPE's real marketplace versions at 1080p:
  - `extruded-logo` (chapter 2)
  - `falling-lines` rain (chapter 3)
  - `curtain-reveal` (chapter 6)
  - his arena title card (chapter 1)

  Do this with `npx hyperframes add` and render. Each must look genuinely good: review the stills.
- [ ] **Step 2:** Check fairness with ffprobe: every opponent clip is 1080 lines or more (Review Focus 5). Commit.

### Task 9: Composites (EffectCraft)

- [ ] **Step 1:** Chapter 14 finale composite:
  - The beauty, depth-holdout and selective-glow passes go into `PROJECT/apps/effectcraft/finale.ecproj`.
  - Render it, and check the 10 evenly spaced frames from Task 5 Step 4 again.
  - **Ledger 44–46.**
- [ ] **Step 2:** Chapter labels as one EffectCraft template comp (lesson text set per chapter), rendered with alpha. Commit.

### Task 10: Final FilmCraft edit

- [ ] **Step 1:** Re-run `build_timeline` with real media for every shot. Use `--final` mode, which refuses placeholders and lists any board still in the timeline.
- [ ] **Step 2:** Editorial techniques, each placed deliberately:
  - J-cut for Jesse's off-screen voice in chapter 15
  - motion match cut from chapter 14 to chapter 15
  - impact cut in chapter 4
  - captions on C1 (styled, synced to the WAVs)
  - transitions between chapters

  **Ledger 12, 47–49.**
- [ ] **Step 3:** Mix:
  - music under dialogue, foley (water, glass, cloth, cymbal)
  - loudness normalized to -14 LUFS integrated with -1 dBTP peaks
  - verified with `ffmpeg -af ebur128`

  **Ledger 50.**
- [ ] **Step 4:** Export `PROJECT/renders/render-wars-ep02-hard-way.mp4` (no `--allow-offline`). Verify with ffprobe: 1920×1080, 30 fps, 290±10 s.
- [ ] **Step 5:** Commit.

### Task 11: Evidence ledger, closing card, review page, CLI

**Files:** `PROJECT/ledger.csv`, `PROJECT/review.html`, the commands `production.voice`, `production.timeline` and `production.ledger` in `crates/engine/src/` (wired like `export.layered_motion` in `commands.rs:327`), and Magic Studio `README.md` lines

- [ ] **Step 1:** Fill `ledger.csv` from the actual edit (FilmCraft `sequence.inspect` gives the shot timecodes). Run `mc exec production.ledger spec=PROJECT/production.json ledger=PROJECT/ledger.csv` and fix or remove every failure. The closing card text comes from `LedgerReport` (the achieved counts), and the card is rendered in EffectCraft and placed at the end.
- [ ] **Step 2:** `review.html`:
  - chapter buttons with the editing lesson per chapter
  - side-by-side champion vs desktop clips
  - the ledger table with source links
  - served the way Gemini's was, on a local port
- [ ] **Step 3:** Rust command wrappers, each returning JSON with `next` steps. Add one README example line per command, then run `cargo build --bin mc`. Smoke test: `mc exec production.ledger ...` gives the same counts as Step 1.
- [ ] **Step 4:** Final QA:
  - Watch the full film at 1×.
  - Use the qa-review skill.
  - Ask Sol to review it against the script.
  - Fix the sources and re-render.
- [ ] **Step 5:** Commit and hand to Jesse.
