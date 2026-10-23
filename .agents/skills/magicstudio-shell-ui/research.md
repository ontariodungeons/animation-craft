# Magic Studio shell research and proposed Animation workspace

Research date: 2026-10-21. **Animation layout approved by the user on 2026-10-21; implementation is in progress.**
Historical source (2026-10-21, before the full rename): user instructions and
`apps/animationcraft-shell/HANDOFF.md` as it existed at `e1cac3a`.
The user subsequently specified: animation inspector on the **left**, useful artwork immediately
after import, and the in-app agent as the app's technical collaborator. Preserve all functionality
in the handoff checklist; progressive disclosure changes placement, not scope.

## Evidence and limits

Studied official documentation, official screenshots, animated UI demonstrations and the Cua
repository's 50-second driver video (sampled at 0, 10, 20, 30 and 40 seconds). Viewed the actual
HyperFrames desktop project image, Spine dopesheet, Moho timeline and Smart Bones animation,
Harmony drawing substitutions, Character Animator performance controls, Cavalry scene tree,
Jitter interface animation, Figma inspector and Linear agent empty chat. These are source visuals,
not hands-on tests of the commercial editors. Rive's current viewport image CDN returned 403;
its current interface/timeline/sidebar documentation was readable. Embedded YouTube playback was
unavailable; do not claim those entire tutorials were watched. Sources vary by version; older
Moho manual pages below establish concepts, not the exact current chrome.

The complete local review was read:
`/run/media/jesse/Speedy/media/merriweather/bumble-bee/.agent/ui-review.md`.
Its 76 findings become individual acceptance criteria below. They are not all fixed today.
The shell itself still has known empty-state and agent rendering defects. “Impossible in the shell”
means release-blocking invariants and verification, not a claim that prose prevents regressions.

## Reference tools

Each entry covers stage, timing, structure/states, parameters/actions, audio, empty state and agent.
“Not established” means the inspected evidence does not support a claim, not that a feature is absent.

### Spine

The stage and skeleton tree share selection. The dopesheet groups keys beneath bones, with a compact
overview and expandable property rows; a graph serves value/easing work separately. The tree exposes
hierarchy and attachment visibility; draw order has its own timing row. Named animations are chosen
independently of the currently selected bone. Audio has waveforms aligned to keyed events. This is
useful timing evidence, not proof of a song-arrangement workflow. Empty-project recovery and a native
AI collaborator were not established in the inspected sources.

Adopt synchronized selection and expandable timing detail. Reject presenting every bone/property
and graph by default: the user should be directing a performance, not configuring an animation IDE.

Sources: [tree](https://en.esotericsoftware.com/spine-tree),
[dopesheet, screenshots and video section](https://en.esotericsoftware.com/spine-dopesheet),
[audio](https://en.esotericsoftware.com/spine-audio-view).

### Rive

Artboards live on a central stage; the inspector follows selection. The sidebar organizes hierarchy,
assets, animations, data and the AI Agent; the agent can also open beside the stage in the viewport.
The timeline provides current time, duration, speed, work area and one-shot/loop/ping-pong modes.
State machines connect animations and expose inputs, rather than being an artwork layer stack.
Audio events trigger sounds from keys, transitions or listeners; the docs describe limitations for
long-form audio control. A file browser handles starting/opening work; missing-rig recovery was not
established. Do not infer that event audio supplies frame-accurate song scrubbing.

Adopt contextual controls and explicit playback modes. Reject an obligatory state-machine graph
for a song performance and do not confuse state-machine layers with visual order.

Sources: [interface](https://rive.app/docs/editor/interface-overview/overview),
[sidebar/agent](https://rive.app/docs/editor/interface-overview/sidebar),
[timeline](https://rive.app/docs/editor/animate-mode/timeline),
[audio](https://rive.app/docs/editor/events/audio-events),
[state machine](https://github.com/rive-app/help-center/blob/master/editor/state-machine.md).

### Moho

The official Smart Bones animation demonstrates a small set of on-canvas controls driving complex
head/eye changes. The timeline follows selected layer channels. Bone layers supply a rig hierarchy;
switch layers display one child at a time, making them a strong model for expression alternatives.
Actions and Smart Bones provide reusable animation/control concepts. Audio and lip-sync support are
documented, including associating a mouth switch layer with audio. Empty-state recovery and a native
agent's location were not established by the inspected material.

Adopt rig-authored controls and mutually exclusive states. Reject controls inferred from anatomy
or filenames; an arbitrary imported object need not have a head, mouth or legs.

Sources: [features and animated demonstrations](https://moho.lostmarble.com/pages/features),
[switch layers](https://www.lostmarble.com/moho/manual/switch_layers.html),
[timeline](https://www.lostmarble.com/moho/manual/timelinewnd.html),
[audio/lip-sync update](https://moho.lostmarble.com/blogs/news/what-is-new-in-the-latest-13-5-3-free-update).

### Toon Boom Harmony

The Camera view displays the performance; Timeline/Xsheet select layers and frames. Drawing
substitutions expose named thumbnail alternatives, including an explicit no-exposure state.
The current-frame choice is visible in the substitution panel. Manual lip-sync couples sound
scrubbing over a waveform to selecting a mouth drawing at that frame. Playback offers ranges,
looping, speed and frame steps. These sources establish frame/pose replacement, not a generic
parameter dashboard or named action library. Empty-project and agent placement were not established.

Adopt visual state choices and sound-aware frame stepping. Reject text-only state selectors and
silently stacking all alternative drawings.

Sources: [manual lip-sync and substitution screenshots](https://docs.toonboom.com/help/harmony-25/premium/sound/manual-lip-sync.html),
[play controls](https://docs.toonboom.com/help/harmony-25/premium/reference/menu/main/play-menu.html).

### Adobe Character Animator

Puppet performance is separated from rig setup. Its Controls panel promotes selected behavior
parameters into sliders/angular controls and triggers into illustrated buttons. Swap sets organize
alternative artwork. Recorded trigger/replay bars provide compact timeline actions. Audio can
drive lip-sync takes and speech-derived movement; triggered audio has documented scrubbing limits.
An empty Controls panel offers generation of controls for the selected puppet. AI appears as
specific animation operations in the inspected documentation, not an established general chat agent.

Adopt named performance controls and thumbnail swaps. Reject a permanently exposed rigging setup
surface or assuming every audio source behaves identically during scrub.

Sources: [controls/triggers and screenshots](https://helpx.adobe.com/adobe-character-animator/desktop/creating-and-controlling-puppets/triggering-and-controlling-puppets.html),
[recording and audio](https://helpx.adobe.com/uk/adobe-character-animator/desktop/recording-and-playback/record-and-playback.html).

### Cavalry

The scene tree displays hierarchy and per-layer visibility alongside editable attributes; selected
animated attributes can be exposed without expanding everything. Its timeline separates time
navigation and markers from time/graph editors. Control Centre promotes chosen attributes with
names, limits and groups into a focused interface. Audio is represented by Sound behavior, with
playback control; song lyric synchronization was not established. AI Studio is a dedicated window
for operations such as background removal, vector generation and image-to-layers, with result
thumbnails. A general empty-scene onboarding flow was not established.

Adopt promoted controls and inspectable results. Reject a full dependency graph or a large technical
attribute tree as the first screen.

Sources: [scene tree](https://cavalry.studio/docs/user-interface/menus/window-menu/scene-window/scene-tree/),
[timeline](https://cavalry.studio/docs/user-interface/menus/window-menu/scene-window/timeline/),
[Control Centre](https://cavalry.studio/docs/user-interface/menus/window-menu/control-centre/),
[AI Studio](https://cavalry.studio/docs/user-interface/menus/window-menu/ai-studio-window/).

### Jitter

The official interface animation shows a central canvas, left layer list, right properties and
bottom event-segment timeline. Layers can be grouped and reordered; selecting an animation exposes
its parameters. Segments can be arranged, resized and grouped. Presets/custom animations offer
actions; this is not evidence of a cutout bone rig or exclusive expression swaps. Audio/waveforms
support synchronization. Templates and AI Brainstorm help start a design. Superagents start from
selected layers/artboards, accept text/images and update the canvas while the user works.

Adopt readable action segments and selection-scoped requests. Reject making every tiny adjustment
a prompt, and retain exact values for timing polish.

Sources: [interface animation and tutorial](https://help.jitter.video/en/articles/12597029-a-quick-tour-of-the-interface),
[timeline](https://help.jitter.video/en/articles/14111802-work-with-the-timeline),
[audio updates](https://jitter.video/changelog/),
[Superagents](https://help.jitter.video/en/articles/15656969-animate-with-ai-superagents),
[first animation](https://help.jitter.video/en/articles/14111550-make-your-first-animation).

### HyperFrames Studio desktop

The official desktop screenshot places agent chat beside the preview and timing below. Select,
draw and comment attach feedback to the picture or time range; pending notes are reviewed before
being sent together. Each agent run has Undo. Desktop setup connects Claude Code or Codex once.
Home supplies a first-video path and inspiration. Studio canvas documentation distinguishes visual
stacking from timeline tracks and keeps canvas/inspector/timeline selection synchronized. Detailed
controls are contextual; this is composition editing, not a character bone/pose system. Audio and
beat landmarks belong to timing; Magic Studio retains its own character motion authoring.

Adopt directing an agent on the frame, queued notes and run-level undo. Reject porting animation
authoring into the video target, or assuming desktop and browser Studio layouts are identical.

Sources: [desktop edit workflow](https://github.com/heygen-com/hyperframes/blob/main/docs/studio-app/edit-with-the-agent.mdx),
[desktop project screenshot](https://raw.githubusercontent.com/heygen-com/hyperframes/main/docs/public/images/studio-app/project.jpg),
[connect agent](https://github.com/heygen-com/hyperframes/blob/main/docs/studio-app/connect-your-agent.mdx),
[home](https://github.com/heygen-com/hyperframes/blob/main/docs/studio-app/index.mdx),
[canvas](https://github.com/heygen-com/hyperframes/blob/main/docs/studio/canvas.mdx),
[timeline](https://github.com/heygen-com/hyperframes/blob/main/docs/studio/timeline.mdx).

### Figma

Canvas selection and layer-list selection feed the same context-sensitive properties panel.
Design and Prototype are distinct contexts; no selection exposes page-level properties. The
inspected documentation does not establish character bones, performance tracks or song-scrubbing
semantics. AI tools are available through Actions, while the current Design agent works on the
canvas and in a rail, starting from a layer. This supports local creative collaboration rather
than a disconnected assistant. Empty-canvas generation is possible, but Magic Studio's imported
artwork needs a more specific preparation path.

Adopt one selection model, inspectable output and contextual properties. Reject hiding essential
controls behind icons without labels/tooltips, or importing a second design system.

Sources: [properties panel and screenshots](https://help.figma.com/hc/en-us/articles/360039832014-Design-prototype-and-explore-layer-properties-in-the-right-sidebar),
[Actions](https://help.figma.com/hc/en-us/articles/23570416033943-Use-the-actions-menu-in-Figma-Design),
[Design agent](https://www.figma.com/blog/the-figma-agent-is-here/).

### Linear

Linear is a collaboration reference, not an animation editor: canvas/bones, pose parameters,
motion tracks and song synchronization are not applicable to the inspected workflow. Its agent
uses workspace context, offers a dedicated conversation and can be invoked in comments. Chat
history and work-in-progress indicators make collaboration revisitable. The official empty-chat
image keeps a prompt and a few useful starting points in a quiet surface. Team pages organize
resources without filling the main area with setup controls.

Adopt contextual conversation, subdued hierarchy and persistent activity. Reject copying an issue
tracker's information density into the stage or treating a completion message as visual validation.

Sources: [agent and screenshots](https://linear.app/docs/linear-agent),
[agent delegation](https://linear.app/docs/agents-in-linear),
[team pages](https://linear.app/docs/default-team-pages).

## Proposed Animation frame

This is a full workspace inside the existing shell, not an external editor or a reduced preview.
The user requested the animation inspector on the **left**. Keep the global 48px bar and its tokens.
Use one context inspector for the selected object/layer; avoid a second permanent technical sidebar.

```text
 Home / Song       Project | Animation | Studio          Agent     Studio light
 +------------------------+--------------------------------------------------+
 | Bee v             ...  | Bee / Rest              [fit][1:1][-][+][center] |
 | [thumbnail] ready light|                                                  |
 | > Action               |                                                  |
 |   Rest / track picker  |              LARGE CHARACTER STAGE               |
 | > Motion & timing      |           click object -> layer details           |
 | > Expression & pose    |           point / draw / comment on frame         |
 | > Layers               |                                                  |
 | > Quality checks       |             soft agent highlight                 |
 | [export icon/menu]     |                                                  |
 +------------------------+--------------------------------------------------+
 | [play][step][loop]  Song 00:08.400   local 00:01.200    [snap][zoom]         |
 | song       ~~~~~~~~~~~~~~~~ waveform ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ |
 | cues       | lyric/scene 3       | lyric/scene 4                           |
 | Bee        [ idle ][ flight / selected instance ][ settle ]                |
 |   detail                    opacity  ----[blink]----                      |
 +--------------------------------------------------------------------------+
 | Agent drawer when open: context chips, conversation, activity + Undo,      |
 | pending frame notes, inline questions/permissions, composer + Send         |
 +--------------------------------------------------------------------------+
```

The drawer is an overlay at the right, not an extra squeeze column. Proposed widths: 260–300px
left inspector, flexible stage, bottom timeline 180–240px. Below 1000px the inspector becomes an
icon-opened overlay. At 700px the stage remains full-width, the timeline retains transport and
song ruler, and layers/controls are disclosed individually. The agent occupies at most 360px;
when open, fit the artwork into the unobscured stage region without moving its world position.
Layer and agent overlays are mutually exclusive at narrow widths. Provide one-click return to
the stage and retain selection. Short windows collapse detailed lanes before shrinking the stage.
These dimensions are proposed starting points requiring live 700/941/1261px review, not fixed art.

### Import and stage

After a successful import, select the new object and show its artwork immediately, centered and
fit to visible content. The selected name in inspector, stage, timeline and agent context is one
identity. A connected object and an actively loaded object must not disagree silently. If loading
fails, retain the artwork and show an inline recovery step with the actual reason.

Rest pose is a distinct engine state: neutral transforms, correct active alternatives, no stacked
blink lids. Keep rest separate from sampling track time zero, which may already contain motion.
When alternative-state metadata is unresolved, show original/reference artwork with the state-review
step; if no such artwork exists, show a state-selection card before assembling alternatives. Never
choose a state by filename or stack all alternatives while awaiting that decision.
The engine resolves pivots, parent transforms, opacity and track membership. Fit respects content
bounds and motion margins; zoom is view-only. Alpha-aware selection should not select the whole
transparent PNG rectangle. Keep art and stage overlays separate so notes never leak into exports.

No import: “Bring in your artwork” + one Import action. Artwork without a rig: show artwork + one
Prepare action. Missing state metadata: show known artwork and a state-review step; never guess
which layers are eyes by name. Bad dimensions: inline object error with a repair action. No tracks:
rest pose plus Create motion. Missing song: local playback stays available, link to Song setup.

### Left inspector and all options

Action is a named picker with the current action evident, including existing idle/walk/run/jump/
wave/talk or other supported actions, rest and authored tracks. Expose only capabilities supported
by the loaded data. Preserve jump height, run lock and WASD roam through a disclosed movement
section; typing in chat or numeric fields never drives movement.

Motion & timing contains speed, motion amount, selected presets and exact range/value labels.
Distinguish preview speed from authored retiming. Expression & pose presents rig-supported head,
eye, mouth, limb and ear controls plus reset; controls carry units, limits and source bindings.
Pose presets display the selected state. Pose audition is reversible and visibly distinct from
writing keys; an explicit key action records at the current local time. No silent auto-key default.

Clicking an object opens its Layers block. Rows show content-cropped thumbnails, names, visibility,
solo and current state. Top row means visually frontmost; the adapter maps this to spec order.
Parent hierarchy is separately disclosed and never changed by visual reordering. State alternatives
use thumbnail choices with one active member, not a visibility checkbox per simultaneously stacked
alternative. Per-layer fix actions open a short menu invoking the actual Craft pipeline.
Keep reviewed state, show-all, anchors, labels and separation preview accessible in this block.

Quality checks group measured results and unknowns consistently. Unknown is not zero or success.
Exports live in one accessible menu/sheet: preserve layered video, cutout MP4, EffectCraft, FilmCraft,
Blender, action reel, rig prompt and lineup workflows. No export controls extend below an unscrollable
panel. A job shows its result and verification, not just “finished”. Intake, pose-sheet viewer and
3D shape controls remain **user decisions: screen or rig/matting CLI chain**, not silently dropped.

### Song timing and track semantics

Use a fixed song waveform/ruler and optional lyric/scene cues from the project's actual analysis.
Missing beat analysis yields no invented beat marks. A named motion track is a reusable source;
a song lane contains placed instances with explicit start, duration, source in/out, rate and loop.
Visual layer order is independent of song-lane order. Selecting an instance synchronizes stage,
left inspector and local detail lanes. Moving an instance preserves duration; trim and time-stretch
are separate operations. Do not silently mix overlapping instances affecting the same property:
show a conflict until the engine supports an explicit composition rule.

Illustrative mapping: instance starts at song 7.2s with source-in 0 and rate 1; song 8.4s corresponds
to local 1.2s. The engine owns that conversion and looping/clamping. The page draws the result.
During audio playback use the audio clock to request/select engine samples; after seeks, frame
steps and loop wraps, verify the picture and audio agree. Space plays when focus is in the stage;
arrow keys frame-step in the timeline. Keep a short work-area loop for polishing a blink against a
lyric. Placement changes persist in project data and participate in undo; audition does not write.

### Engine migration contract (implementation after approval)

Read `crates/ui-egui/src/panels/center_motion.rs` before porting. Move the existing Rust
`evaluate_layered_motion_spec` / `eval_oscillate_term` and required helpers into `crates/engine`,
adapting `UiState` input into plain parameters. Do not invent another evaluator or move evaluation
into JS/Python for the interactive page. Expose track discovery and sampling through the common
registry/CLI/MCP. The exporter `pipeline/layered_motion.py` is authoritative when behavior differs.

The read-through found parity hazards that tests must resolve: unknown easing currently falls
back in Rust, malformed terms may become zero, scale is clamped in the UI, time wraps even for
non-loop intent, and rest fallback hides IDs ending in `closed`. These are investigation findings,
not permission to retain character-name heuristics. Compare keys, oscillation/envelope, ellipse,
follow smoothing, boundaries, track layer inclusion and parent/pivot matrices. Use the real bee
spec and portable fixtures; never conditionally skip assertions because an asset is absent.
Exporter motion blur is a separate rendering property, not proven by matching single-time samples.

## Agent-native operation: implement in this order

The agent is core product. Human controls, CLI and MCP act on the same live session, with the user
able to observe each change. The app owns its UI; background semantic actions need neither global
Wayland input nor a second invisible engine. Cua provides the comparison: exact targets, a separate
agent cursor and readback distinguish a delivered action from confirmed effect. The official demo
shows two app targets operating while a terminal remains foreground. Its platform matrix explicitly
limits generic Wayland background input; copying that fallback is unnecessary inside our app.

Sources: [Cua repository/demo](https://github.com/trycua/cua#cua-driver),
[action/verification model](https://cua.ai/docs/cua-driver/concepts/how-cua-driver-works),
[cursor](https://cua.ai/docs/cua-driver/reference/mcp-tools/pointer-cursor-and-browser),
[platform limits](https://cua.ai/docs/cua-driver/concepts/platform-support).

1. **Persistent live MCP.** Historical observation (2026-10-21, before the full rename): the
   global command was `mc --bridge 19897 mcp`, with no project path. Initially proposed as a user
   configuration change; the user then explicitly authorized moving MCP to the shell. The global
   `animationcraft` command was updated with a backup and
   OpenCode configuration reloaded. An inner-agent read-only task confirmed Bumble Bee in the
   shell after switching from Hey Ho, demonstrating window-session scope. The installed OpenCode
   v2 API normalizes configuration under `mcp.servers` and uses `disabled`; the on-disk legacy entry
   used `enabled`. Preserve the installed schema rather than blindly copying web examples.
   Recheck after a future OpenCode restart; a full restart has not yet been exercised.
   [OpenCode MCP docs](https://opencode.ai/docs/mcp-servers/).
2. **Visible, reversible actions.** Soft token-colored highlight for target/region; compact drawer
   trail with actor, action, target, before/after, time and outcome. Read-only looks are labeled as
   observations, not edits. Changes from direct engine commands must appear too. Use stable IDs and
   a project revision, reject stale targets, read back state, and expose Undo/Redo through the same
   engine. A completed agent run groups its edits; undo must not erase later human work. External
   tool outputs need versioned artifacts and reversible reference changes, not destructive overwrite.
   Current `Session::record` is a result echo, not a complete undo implementation. Treat undo as
   required engineering work before claiming full control. Highlight overlays use pointer-events:none
   and respect reduced motion; no bouncing cursor or focus theft.
3. **Point both ways.** Preserve current element picture+ID. Add frame-space drawings, pinned
   comments and time ranges, grouped as reviewable pending notes with one Send action. Store original
   image dimensions, project/revision, frame/time, stable target IDs and normalized annotation points.
   Resizing must not shift the note's meaning. Agent point-back highlights the named object/layer/time
   and offers a reveal action; it does not interrupt typing or move the user's mouse. HyperFrames'
   desktop workflow is the reference; annotations are not generated artwork.
4. **Shared context.** Capture project/view, object/layer/scene, source track, placed instance,
   song and local time/range, fps and revision at send time for both OpenCode and Herdr. Show concise
   context chips beside the message. A later selection change never retargets an in-flight request.
   “Later” needs an explicit amount or a user-approved relative target; ask inline if unresolved.
5. **Questions and progress.** Retain inline permission/question cards, their IDs and drafts across
   polling. Jobs return IDs and progress events; UI remains usable and offers cancellation. Serialize
   commits, not all rendering behind a long-held session lock. Completed/cancelled/failed states are
   explicit. Never leave “working” after a terminal event. Never let coordinator messages overwrite
   the user's composer: use the session API for technical delegation.
6. **Markdown.** Vendor a maintained stable renderer with exact version/license; sanitize or disable
   raw HTML. Preserve code blocks, lists and links. Incrementally update streaming text without
   replacing all prior message nodes. Long tool activity stays collapsed; detailed errors/logs are
   available on demand. Enter sends, Shift+Enter inserts newline, IME composition does not send.
   Keep drafts/attachments until delivery succeeds and preserve edits made while delivery is pending.

For every step: rebuild if code changed, bridge-drive it, `ui.look`, open the PNG, ask the inner
agent for a real task, and inspect outcome/undo. Example once layer controls exist: hide the blink
alternative, verify stage + layer state + action trail, undo and compare. Test notes at another zoom,
stale context during a running job, cancellation, a failed send, and a typed draft during polling.
Only the connection-check task has been completed so far; do not label the other steps verified.

## 76-finding prevention ledger

All rows are acceptance requirements, **pending implementation verification**. IDs refer exactly to
the local review, not new issue numbering. Exercise relevant rows at 700, 941 and 1261px, with the
agent open/closed, keyboard focus, long names and empty/error data. Pixels and geometry checks are
both required. Planned overlay overlap is excluded from layout collisions; inaccessible underlying
controls must not be reported as clickable visible targets.

| # | Required invariant / acceptance check |
|---|---|
| 1 | No permanent raw command echo in header; activity details belong in drawer/log. |
| 2 | One selected-object picker with caret; same object identity across views. |
| 3 | Project identity visually separated from view navigation. |
| 4 | Agent toggle has visible pressed state and aria-pressed matching drawer state. |
| 5 | DOM/automation IDs unique; duplicate labels cannot silently resolve to first match. |
| 6 | Enabled actions look enabled; disabled actions truly refuse invocation and explain why. |
| 7 | One sans type system; mono only for values, times and paths. |
| 8 | Row title has min-width:0 and no collision with approval/status controls. |
| 9 | Long transcript title cannot overlap brief action. |
| 10 | Trailing action slots stay reachable; title yields width first. |
| 11 | Stage-actions SVG has normal hit area, tooltip and accessible name. |
| 12 | Status uses semantic tokens/dots with text alternative, no emoji badges. |
| 13 | Missing output explicitly distinguished from ready output and unavailable to open. |
| 14 | Input/output relationship stays together or stacks as labeled rows, no orphan arrow. |
| 15 | File references carry type identity; link appearance reserved for actual links. |
| 16 | Explanations wrap or expose full tooltip; meaningful instructions never silently clipped. |
| 17 | No input yields a named empty state; no dangling relation from an em dash. |
| 18 | Header, asset and stage share readiness/selection state; no contradictory badges. |
| 19 | Instructions name the actual destination view, not ambiguous numbered stages. |
| 20 | No duplicate project/mode title in content. Rename updates one source. |
| 21 | Uninitialized target is neutral, not an error; unavailable/broken distinguished. |
| 22 | Target options have consistent affordances and selected state. |
| 23 | Target picker remains one coherent group; use select at narrow width. |
| 24 | Setup/start has one next action with attached state, not competing calls to action. |
| 25 | Section labels do not outrank the user's actual selection. |
| 26 | Import action and hint occupy separate layout slots. |
| 27 | Asset grid has bounded scrolling; cannot paint beneath storyboard. |
| 28 | Thumbnails use engine content bounds; tiny parts remain recognizable. |
| 29 | Assets grouped by object/folder; full names/paths available on hover/focus. |
| 30 | Distinguish artwork, part, background and rig without category colors implying status. |
| 31 | One import path per place, with explicit destination. |
| 32 | Scenes are discrete rows/chips; no run-together flow text. |
| 33 | Brief action is outside scene navigation. |
| 34 | Scene entries expose time/status and full lyric/title; thumbnails when useful. |
| 35 | Storyboard disclosure is a small paired show/hide icon, not a competing primary. |
| 36 | Page zoom/fit permits reading; no compulsory miniature full-page text. |
| 37 | Draft status is evident; Open PDF is secondary. |
| 38 | Artist lyric/title edits are supported without overwriting the raw transcript. |
| 39 | Agent text 12–13px, bounded readable width and natural wrapping. |
| 40 | Composer placeholder is short; shortcut explanation in tooltip/hint. |
| 41 | Point is an explicit pressed mode; Send is the compose area's sole primary. |
| 42 | Agent Markdown rendered by vendored library, not literal markup or regex parser. |
| 43 | Tool calls compact/collapsed, labeled and status-bearing; details on demand. |
| 44 | Human-readable error + log access; parser dumps/terminal commands not permanent transcript chrome. |
| 45 | User/agent roles identified by label or avatar as well as styling. |
| 46 | Session and Model have readable field labels and accessible names. |
| 47 | Object rows left aligned with thumbnail and reserved trailing menu. |
| 48 | Empty references use a complete sentence; no “for none”. |
| 49 | Artifact actions identify their target in tooltip/accessible name and surrounding card. |
| 50 | Counts consistently include units. |
| 51 | Checkboxes/toggles have clear checked/unchecked states and keyboard semantics. |
| 52 | Show all/review actions reflect real availability, not blanket dimming. |
| 53 | Zero reviewed of zero is neutral/unknown, never green success. |
| 54 | Slider label, value, units, endpoints and thumb agree. |
| 55 | Preparation steps are explicitly grouped, not accidental links within source cards. |
| 56 | One accurate hint per empty view; no duplicated or contradictory 2D/3D instructions. |
| 57 | All control icons are SVG; missing font glyphs cannot substitute for icons. |
| 58 | Secondary controls use shared tokens, no one-off olive/brown fill. |
| 59 | Source/reference selection distinct from work modes; one selection indicator per group. |
| 60 | Imported/selected artwork is visible or has a concrete load/preparation action. |
| 61 | Stage claims flexible space; inspector cannot cram while empty columns reserve space. |
| 62 | Actions use one consistent picker/group; no accidental 4+3 wrap or text pretending to be buttons. |
| 63 | Motion preset selection agrees with numeric amount; custom values marked custom. |
| 64 | Playback speed shows true range and value; preview rate distinguished from authored rate. |
| 65 | Measurement section titled once. |
| 66 | All related measurements, including drift, live in that section. |
| 67 | Export labels, accessible names, target and operation agree. |
| 68 | Export descriptions wrap or ellipsize with full tooltip. |
| 69 | Every export reachable in a scrollable menu/sheet at short heights. |
| 70 | Section headings share a color; semantic status alone uses status colors. |
| 71 | Missing views/poses use neutral state, not category-color badges. |
| 72 | Checks share the same treatment for measured, pending and unavailable. |
| 73 | Breakpoints collapse/disclose panels before collisions; test 941px with drawer open. |
| 74 | Agent overlay does not squeeze stage into unusable columns; narrow workspace retains a stage return. |
| 75 | Switching views preserves outer window geometry. |
| 76 | Every truncated title/name has ellipsis and full tooltip/accessibility text. |

Additional live shell finding: `ui.elements` currently reports descendants of collapsed blocks and
offscreen scroll items as visible because nonzero DOM rectangles alone do not establish visibility.
The agent contract needs clipping/occlusion-aware visibility or an explicit reveal operation.

## Port, verify and stop points

For each feature read its mapped implementation in HANDOFF, run the reference on 19898 beside the
shell on 19897, compare behavior and engine calls, and port into the shell's design. Do not edit or
maintain the reference UI. Record the port's commit beside the checklist entry only after verification.
Close the reference after comparison; the user's explicit request to keep both beside each other
during active work takes precedence while that comparison continues. The user alone authorizes deletion.

The screenshot supplied by the user makes the left inspector and useful main canvas explicit.
First approved slice should include rest/selection/import recovery, transport, track selection,
layers/states and song timing. Subsequent slices preserve the rest of the checklist; do not call
the migration complete after that first slice. Animate and export the bee, inspect stills/motion,
validate the clip, then check the imported result in the chosen Craft design/video app. Export
target choice remains a user decision; no target is silently substituted.

Pending approval questions: (1) approve this Animation frame, (2) intake as screen or CLI chain,
(3) pose-sheet viewer as screen or CLI chain, (4) 3D shape controls as screen or CLI chain.
No Animation screen implementation is authorized by this document alone.

## Historical runtime investigation / operational evidence

The following observations describe commit `e1cac3a` and its pre-rename paths. They are not current
build or file-location instructions; use `magicstudio-shell`, `.magic` and `mc` for current work.

- Rebuilt historical commit `e1cac3a` using `cargo build -p animationcraft-shell`; relaunched its
  shell on 19897.
  Opened `ui.look` PNG at Hey Ho's `.agent/looks/look-1792562627379.png`; project-local default works.
- Opened reference on 19898; compositor reported two 1261×1381 tiled windows, reference left and
  shell right. Inspected bee reference capture `.agent/looks/older-ui-comparison.png`.
- Inner agent confirmed via MCP that the shell changed from Hey Ho to Bumble Bee, with Project
  view active. Global MCP config contains no project path. No comparison-app MCP was added.
- Flicker investigation: an unchanged poll replaced the first transcript node while text remained
  identical. A 5.2s live probe observed 3 child-list mutations, 169 fade starts, **zero resize events**,
  stable 1261×1381 viewport. Latest Rust screenshot patch changes output location, not rendering;
  Rust `layout` is called on resize/view/split changes. This establishes a DOM-refresh cause for the
  reproduced message flicker; it does not rule out every possible compositor defect.
- `opencode` Rust command uses blocking process I/O in an async function: a separate responsiveness
  concern to measure, not evidence it causes the observed fade cycle. No speculative Rust patch.
- `Cargo.lock` has Tauri 2.12.4; official [release history](https://tauri.app/release/tauri/all-versions/)
  was checked. No dependency upgrade needed for this documentation/JS work.
- At `e1cac3a`, app agent instructions lived in `apps/animationcraft-shell/agent.md`, intentionally
  free of migration/reference-app instructions. Current instructions live in
  `apps/magicstudio-shell/agent.md`; give those to the current coding agent separately.
- Chat bugfix committed as `1cedbfc`: stable transcript nodes, safe draft delivery, Enter/Shift+Enter
  behavior. The inner agent implemented the focused patch after reading the app/UI/debugging instructions.
  Coordinator reran 25 browser assertions successfully, rebuilt and relaunched the shell, preserved the
  user's draft, and measured zero fade restarts with the same first node over 5.2s at 941×1021.
  Opened live evidence: `.agent/looks/look-1792564555019.png` in Bumble Bee. Markdown remains pending.
- Required skill inventory found no missing SKILL.md entries. CLI-Anything's missing vendored
  references/helpers were restored from `tools/CLI-Anything/cli-anything-plugin`. No official Tauri
  design skill was found in installed crates or official Tauri/tauri-docs trees and organization
  repository inventory; community packages were not mislabeled or installed as official ones.


## Approved refinement: section color and slider taste

The user prefers this shell overall, and requested only the useful section coding and icons from the
reference inspector, not its density. Reviewed actual Rust constants and slider styling and current
[Rive inspector](https://rive.app/docs/editor/interface-overview/inspector) plus
[Adobe Spectrum slider anatomy](https://spectrum.adobe.com/fragments/components/slider/anatomy) and
[usage guidance](https://spectrum.adobe.com/fragments/components/slider/usage-guidelines). Adopt labeled
controls with units, quiet values, small handles, section icons and contextual grouping. Use the
reference blue/purple/gold/green hues as restrained category accents. Teal remains the primary action.
This explicitly supersedes ledger #70's single-heading-color requirement: named categories can have
colors, while state remains a dot plus a label. The older Spectrum /page/slider URL was unavailable;
the current fragment pages above were read instead.

Pen path uses existing engine keys, not a second evaluator. Start/end scale is local scale; source
wing/antenna motion and root rotation remain, so matrix diagonal entries are not scale measurements.
A grid and the path are editor overlays and never enter exports. User intent for the bee is recorded
in the song's intent.md: slow classic layered 2D, continuous frame, gentle scale/parallax for depth.
