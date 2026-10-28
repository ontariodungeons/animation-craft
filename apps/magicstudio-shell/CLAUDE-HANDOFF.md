# Claude takeover — 2026-10-21

User explicitly asked to hand back to Claude and share work. Claude in Herdr `wD:pV` now owns
`feat/production-pipeline` and the live shell on **19897**. Codex is stopping implementation after
this checkpoint. Read HANDOFF.md and its ordered skills before edits; this file is the latest steering.

## Immediate task: fix the mouse's clipped hands

User's last defect: **“the mouse hands cut clipped”**. This is real, inherited from the source I
chose, not a render failure. I should have rejected that cutout before exporting. Do NOT present
current Mouse as production-ready.

Current connected Mouse:
`/run/media/jesse/Speedy/media/merriweather/nelly/assets/characters/mouse/Mouse-reference.png`
It was PhotoCraft `image.trim` of this old grouped-cast layer:
`/run/media/jesse/Speedy/Work/merriweather-videos/projects/kathy-cd-track-01/revision-03/opening/remotion/public/animated/grounded-cast/parts/layer_0002_mouse-body.png`
That layer clips both arms/hands at vertical crop edges, has edge contamination and stray alpha.
Trimming whitespace does not repair it. Preserve this source as history, replace the connected
artwork with a COMPLETE matching mouse, then regenerate the same jump track while preserving the
user's scene placement. Do not silently redesign the character or use a different vintage.

Better existing assets found (not assembled yet):
- Source root `…/kathy-cd-track-01/revision-03/opening/remotion/`
- `public/full/puppets/mouse/arm-left-soft.png` and `arm-right-soft.png`: inspected; both have complete
  paws. Also head/torso/legs/tail/front/back assets.
- `src/puppet-assets.json`: nine parts per mouse/bear/fox, all files exist; dimensions, joint/end UVs,
  source crops and source sheets. `src/Puppet.tsx` shows the existing assembly transforms.
- `public/corrections/friends-ready-v2.png`: exact four-character group reference.
- `renders/nelly-friends-round-checkpoint-v1.mp4` (2.453s): all four turning in a circle, inspected.
- `renders/nelly-storybook-full-v1.mp4`: all four at24s, inspected.
- Nelly currently connected is revised `public/storybook/nelly-walk-v3.png`, not the older front puppet.
- Inventory: Nelly `.agent/recovered-cast.json`. Revision02 dance proof is a HUMAN Snow rig, not this cast.

## User's workflow / priorities (latest, supersedes long-range planning)

“You can animate, I can design.” One character at a time, **start with the harder mouse, not bunny**.
User drags static assets onto editable storyboard scenes, scales via EDGE/CORNER HANDLES (no slider
needed), deletes them, saves a NEW version, then points at scene numbers to direct animation.
User actually placed/resized Mouse in **scene14** and saved. Desired action: Mouse jumps up/down
alongside Nelly in that scene. Scene14 is26.80–29.38s (2.58s), page17 of the new deck.
Do not lose their placement, drafts, or saved layout when restarting.

Get a COMPLETE playable animatic quickly: song + lyrics + storyboard stills/transitions. Improve it
in short passes: place → animate ONE transparent track → lay into HyperFrames → return result/context
to deck. Do not wait to build every control or perfect every character. Every UI action uses shared
engine/CLI/MCP commands. Keep the preparation chain to very few commands; save checkpoints.
Minimax is an operational validator; do not give it complex creative orchestration. If Codex model
needed, user permits Sol6.1 only. No paid model/provider switches made.

## Live state / outputs

Project `/run/media/jesse/Speedy/media/merriweather/nelly/Nelly.magic`.
Shell on19897, Animation view, `mouse`, `@jump`, amount35%, speed2/2.58, paused at end.
HyperFrames managed server3058. Protected other servers3002/3021–3023 untouched.
Backlot read-only reference server4750 was started from source repo `.venv/bin/python -m backlot serve`.
Old egui19898 is closed. Do not maintain or delete it.
In-app OpenCode session `ses_ea28376feffeRZ1TMZxBJQw0Hw`; MiniMax M3.1 Flash Free. Last assigned
read-only validation of exported jump, placement record and HF check. May retrieve result via API.
Do not use the user's composer to coordinate; saved draft was `y` most recently.

### Current jump — technically valid but ARTWORK DEFECT

`tracks/mouse/jump-1792572002094/jump.webm` under Nelly. 344x1032,77frames at30fps (~2.567s),
alpha=true, no empty frames/clipping/teleports/flicker. Placed for2.58s so final ~13ms holds final frame.
`motion.json`, `review/jump.png`, and EffectCraft project are beside it.
This is whole-cutout anticipation/jump/settle, not an articulated mouse rig. Amount35%.
`motion.export_track` bakes existing Rust samples into exporter keys; **no new evaluator**.
Evidence `.agent/mouse-jump-export.jsonl`; parity of this new exporter conversion has not had a
separate multi-part regression test yet. Single-image export visibly reviewed and validated.

HF insertion `mouse-scene-14-jump`, track index4, start26.8/duration2.58.
`hyperframes/assets/animation/mouse-scene-14-jump.json` has media, placement rectangle and backup.
`video.place_track` copies media, inserts a timed native video clip, preserves source/HTML backups.
HyperFrames check passed0errors/35existing warnings; snapshots in `.agent/looks/mouse-in-studio/`.
**Base composition is still the OLD `assets/video/nelly-blender-full.mp4`**, with the older bunny.
The inserted track is reusable; don't call this matching finished scene14. Replace base with the
iterative animatic/current artwork as next production work. Current fixed z-index10 also overlaps
some captions in snapshot; fix compositing order. No final full-video render made.

## Implemented this checkpoint

- Animate card now uses real project asset registry rather than stacking all PNGs in character folders
  (that had stacked old and new Nelly). Assets panel below, Add asset through `project.add_asset`.
- Static asset cards draggable onto storyboard pages; engine `storyboard.place_asset` stores page-
  coordinate rectangles (960x540) and scene IDs. Drag to move; eight edge/corner handles preserve ratio.
- Remove button on hover via `storyboard.remove_placement`.
- `storyboard.save_version`: existing DeckCraft CLI insert.picture into a NEW deck/PDF/revision;
  headless end-to-end test under `.agent/design-command-check` succeeded, slide08 visually inspected.
- Saved placements stay structured: new `design_base` preserves original deck/previews, and the viewer
  shows base + editable placements rather than baking them twice. Most recent saved-edit/delete fix
  COMPILES and is in live binary but needs focused save→reopen→resize/delete→save regression check.
- `storyboard.read scene=14` now returns correct deck page, preview, timing and saved/draft placement
  records. `draft_placements` currently contains effective saved placements too (naming should improve).
- Agent messages capture view/scene/page/deck/placements/animation context at send time.
- Animation Export track button uses `motion.export_track` (engine, CLI, MCP).
- `video.place_track` is the generic HyperFrames placement CLI/MCP command.

Commands (live or headless after project.open):
```
mc --bridge 19897 exec storyboard.read scene=14
mc --bridge 19897 exec storyboard.design
mc --bridge 19897 exec storyboard.place_asset asset=mouse scene=14 x=160 y=105 width=96
# existing placement id updates instead of adding
mc --bridge 19897 exec storyboard.place_asset asset=mouse scene=14 placement=<id> x=160 y=105 width=96
mc --bridge 19897 exec storyboard.remove_placement placement=<id>
mc --bridge 19897 exec storyboard.save_version
mc --bridge 19897 exec motion.export_track character=mouse track=@jump amount=35 duration=2.58
mc --bridge 19897 exec video.place_track id=<stable-id> path=<webm> scene=14 start=26.8 duration=2.58 rect='[x,y,w,h]'
```
MCP names replace dots with underscores. `video.place_track` rect is VIDEO pixels; deck placement is
SLIDE points. The caller currently maps them using template art rectangle [32,102,620,349] and export
margin. `.agent/mouse-track-placement.json` records the exact call. This mapping should become one
combined publish command, not repeated LLM arithmetic. No automatic “back to deck motion preview” yet.

## Prior completed work

- Project/Animation/Studio; Split removed. Studio/agent dock fit via GTK Overlay. ui.look composites
  both native webviews. Avoid reintroducing vertically packed webviews. Preserve outer window size.
- Storyboard landscape doc generated with `workshop.build_storyboard template=magic-studio source=<source>`:
  38slides,35real scenes,actual revision03 references,approved lyrics,script acting cues. Editable
  DeckCraft + linked PDF (70links verified), continuous scroll, clickable contents, collapsible project
  details. Source script `pipeline/movie_board.py`. New builds preserve old revisions.
- `agent.new_chat` is shared engine/CLI/MCP, plus header+ button; preserves history/model, sends app
  agent.md to fresh session. First fresh session learned correctly but verbose boot message is visible.
- Imported approved `artifacts/lyric-verification/lyrics.words.json` + `assets/subtitles.srt`. Raw
  transcript was wrong spelling; alternative ASR stops45.31s. Never use those as authoritative.
- Chat flicker/draft bugs fixed earlier; browser regression25/25 passed again this checkpoint.

## Validation and boundaries

Latest source compiled successfully. 25agent-drawer browser checks pass. New command tests:
headless place+save produced separate deck/PDF; real UI drop handler worked; user's own drag/scale/save
worked; current scene read confirms placement. Alpha exporter and HF checks passed as above.
Run cargo check and the tests that match the change, and report the real result. No dependency upgrades in this checkpoint.
Many unrelated UNTRACKED files were present before our work; leave them alone. Do not stage all.
Permanent app knowledge `agent.md` must NOT mention an earlier UI. Migration rules stay HANDOFF.

Suggested sharing: Claude owns artwork correction/creative animation and UI decisions; use the
in-app agent for narrow operational checks with exact commands. Codex has stopped branch edits.
