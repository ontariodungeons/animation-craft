# Magic Studio completion plan

**Status:** implementation plan, not a completion claim. Reviewed 2026-10-24 against committed revision `5ab274ba` and the explicitly identified working files.

**Goal:** unify the existing architecture and styling across every panel, reduce duplicated code, and get the two existing animated SVG characters working through that shared implementation. Verify existing intake/static assembly afterward; do not expand the product to accomplish this.

**Architecture:** Rust owns project operations, persisted evidence and production eligibility. The shell renders those results and publishes bounded panel observations through existing `ui.feedback`. CLI and TUI read the same component configuration and observations; neither invents workflow success.

**Stack:** existing Rust engine, Tauri shell, vanilla JavaScript, Ratatui, native storyboard tools and HyperFrames. Reuse the existing tool adapters.

**Sources:** [recovered diagram](panel-contract.html), [panel contract](../superpowers/plans/2026-10-23-unified-panel-reporting.md), [video wiring plan](../superpowers/plans/2026-10-23-magic-studio-wiring-refactor.md), and Jesse's six steps below.

## Binding architecture — updated from Jesse's corrections

These requirements supersede conflicting details in the recovered diagram and the older plan. This file is the execution checklist; further user corrections are incorporated here before dependent implementation.

1. **Every UI component registers.** Static markup and dynamically created UI use the same registration path. This includes containers, right-hand panels, controls and their children. Registration is independent of whether a visible light or pet is shown.
2. **One inspectable record.** Each registered component exposes its stable ID where authored, numeric instance index, parent ID/index, component type, configuration reference, screen rectangle `(x, y, width, height)`, visibility, enabled/selected state, relevant data, available actions and mapped engine commands. Runtime-generated indices are scoped to the Studio instance and snapshot; they must not be presented as persistent IDs.
3. **Complete CLI discovery and control.** CLI inspection pages through all registered records, includes total/count/revision and explicitly reports unavailable, unconfigured or truncated observations. Each actionable component exposes the existing `ui.click`, `ui.set` or `ui.key` route; engine-command mappings are reported only when verified. Secrets and private composer drafts are excluded from telemetry.
4. **UI and TUI share the records.** The UI publishes state through the existing feedback transport. TUI reads that same state; it can request attention at a registered component in the explicitly selected project. No independent TUI state guesses or pet-specific command mapping.
5. **One component/style configuration.** `components.json` declares component/look/density, indicator visibility, canonical state target, rail membership and pet destinations. `style.css` supplies common component rules and palette tokens. The right-hand chat, artwork and assets panels must use the same panel construction and styling as the rest of the app. Remove duplicate implementations and measure actual net code reduction, including new files/configuration.
6. **Indicator visibility is optional.** Every component has readable state even when its indicator is hidden. Idle is 💤; active/ready is green, working adds busy/pulse, attention is yellow, error is red. A hidden indicator cannot hide state from CLI/TUI.
7. **Pets enforce the architecture.** Their only inputs are shared registered state and configured rails. No manually positioned overlays, fixed right-panel coordinates, separate command watcher or per-panel pet code. An unregistered/unconfigured destination must report a contract violation rather than accepting a special-case placement.
8. **Dog:** follows bugs/errors or the next action needing attention. **Cat:** follows the last relevant chat action, or joins the dog for what's next. With no activity/attention, each returns to its configured AI box. The TUI can request attention through the shared component record. Clearing a request never clears a real engine error.
9. **Artwork follows the architecture.** The honey-coloured dog and orange cat are starter assets. Do not activate them until the registered right-hand panels, shared styling and generic rails pass their gates. No grey dog replacement work is needed from Jesse.
10. **Ownership:** Codex implements UI/engine/CLI and prepares the documented telemetry interface. Jesse will supply the TUI agent; send that agent a committed handoff when identified. Do not create a competing TUI implementation. Claude owns lyrics behavior (`ui/words.js` and lyrics engine work); coordinate shared `app.js`/`style.css`/`components.json` integration and preserve his changes.
11. **Location:** this plan, the diagram and handoff stay in `docs/architecture/`. Evidence stays in `docs/architecture/evidence/`; song media stays in the song project. No `.codex/sparks`, temporary-document destinations or directory hunting.
12. **Reuse the working animation system.** Preserve the existing fish-and-bee animation workflow. Do not replace its engine, evaluator or timeline. Extend the existing CLI/engine adapters only where commands are missing. Blender owns 3D; verify the existing 2D breakout/rig tools and OpenToonz capability before choosing an adapter. Shared Rust packages are candidates for reuse, not permission to rewrite the UI or vendor an entire app.

## Current execution status

- Base: `5ab274ba`; branch: `fix/panel-unification`; worktree: `.worktrees/panel-unification` inside this project.
- Registration/telemetry foundation committed as **775c6a02** on `fix/panel-unification`; normal commit hooks and workspace tests passed. This is a partial implementation, not whole-app completion. Starter SVG assets remain uncommitted and inactive.
- Focused panel/companion tests and a workspace compile check have passed during development. They do **not** establish completion of the architecture or full telemetry.
- A live preview exposed the wrong order: pets were displayed on the old right-hand panel structure. That preview has been closed and its process exit verified. The SVG manifest activation has been removed from the working changes.
- The next work is the registration contract and shared right-hand panel structure. No more artwork preview or cosmetic pet positioning before those gates pass.

## Review: what is actually present

This is a source review. No fresh app build, live screenshot, production edit or end-to-end acceptance run was performed for this plan. Existing tests are evidence of coverage intent, not a claim that they passed today.

| Area | Finding and source | Remaining work |
|---|---|---|
| Shared panel registry | `ui/companion-boxes.js` emits sorted items, `signal`, `busy`, revision and unconfigured entries; `feedback.js` carries it. | Verify project/view transitions and complete reporting when more than 64 controls exist. |
| Rust contract | `crates/engine/src/panels.rs` defines `Config`, `PanelReport`, `Signal`, reasons. | Config validation is absent from `Config::load`; malformed feedback collapses to unavailable. Make failures diagnosable. |
| CLI | `apps/mc/src/main.rs` has no panels module; `apps/mc/src/panels.rs` is absent. | Implement `mc panels`, JSON output and a failing `--check`. |
| TUI | `apps/mc/src/center/panels.rs` loads config and builds panel/health rows. Cache key is revision plus availability. | Handle a different Studio/project with the same revision; share config loading with CLI; expose component/look in inspection. |
| Studio discovery | `apps/mc/src/studios.rs` still returns JSON rows. | Finish the planned typed Studio representation without changing existing command output. |
| Companion timers | `ui/companions.js` already has per-pet timers and 1500 ms cooldown. | Retain and verify existing regression coverage; do not rebuild it. |
| Style migration | Config and lint exist. The working baseline records 906 px outside `:root`, inline writes and missing config entries. These are stored baseline numbers, not fresh measurements. | Complete migration and enforce final acceptance. `ui-lint.mjs` currently prints the size bar explicitly without enforcing it. |
| TUI theme | `apps/mc/src/theme.rs` is absent; `center.rs` and `logo.rs` construct colours/styles directly. | Read the Studio palette through one theme module. |
| Intake | `command_center.rs::intake` invokes audio analysis/import, storyboard init and video init; returns steps, problems, readiness and `studio_verified:false`. | Verify decoded artifacts and consistent failure presentation in every caller. `ok:true` alone must never mean intake complete. |
| Storyboard editing | `storyboard_design.rs` implements placement, saved versions and PDF operations. | Verify drag → save → reopen pixels, scene identity and failure preservation. |
| Native animation acceptance | `animation_links.rs::accept` validates a native job and records an accepted render. | This is not storyboard approval. Bind proof and approval to specific source/scene revisions. |
| Production placement | `video_target.rs::cmd_video_place_track` accepts files/jobs and optional source provenance. | It does not require an approved storyboard proof. Enforce approval for production promotion. |
| Storyboard approval | `storyboard.rs::cmd_storyboard_approve` writes a status field. | Replace status-only production eligibility with an immutable, revision-bound approval receipt. |
| Lyrics | Recent commits added lyric timing, effects and styling. Working `index.html` says Lyrics, but config and accessibility text still say Design. | Finish the name consistently and verify the view against the panel contract. |

Today's recorded work includes `4d1d4b6f` (lyrics preview), `1b6466b3` (effects per sung line), `b19b7830` (video lyric timing), `01a29db6` (section styling), and `5ab274ba` (configured component classes). Those commits do not establish completion of the architecture.

## Canonical user workflow

1. **Start project.** Create and save the selected `.magic` project with a stable project identity.
2. **Upload audio.** Register the actual audio file and verify it decodes.
3. **Prepare the project.** Transcribe audio unless valid supplied timing already exists; create the Studio video project and editable storyboard. Show actual failed steps and retry the failed work without replacing successful authored outputs.
4. **Fill and assemble.** Map required assets into every storyboard scene and assemble those same assets in Studio. A static composition is valid; motion is optional. Missing requirements remain visible and block readiness.
5. **Edit the storyboard.** Drag registered project assets onto PDF scenes, save a version, and reopen the saved result with the changes intact.
6. **Validate and promote.** Compare an animation with its scene reference, save its linked proof over that storyboard image, record explicit approval, then place the approved clip in the production video. Later source or storyboard edits invalidate that approval.

## Execution sequence — architecture gates

Run these gates in order. Update this checklist with commit SHAs and evidence as work passes. No gate is satisfied by a plan, a screenshot alone, a tool success flag, or a per-panel workaround.

### 1. Register and expose every UI component

**Owners/files:** Codex; common constructor/static initialization in `ui/app.js`, registry in `ui/companion-boxes.js`, `ui/bridge.js`, Rust panel types, shell bridge and CLI discovery.

- [ ] Define one record schema covering every field in binding requirement 2 and document it in `docs/architecture/tui-contract.md`.
- [ ] Route static and dynamic components through the same registration function; preserve authored IDs and assign instance indices to unnamed nodes.
- [ ] Export complete paginated records and totals through CLI, with exact existing action routes, component/parent references and positions. Invalid/stale targets must fail explicitly.
- [ ] Publish shared panel state and requests for attention without depending on the companion controller being loaded or enabled.
- [ ] Test nested components, unnamed controls, dynamic replacement, project switching, hidden/disabled controls, missing registry/config, stale indices, more than 64 components and missing command mappings.
- [x] Commit registration/telemetry foundation: `775c6a02`; normal pre-commit checks and workspace tests passed. Handoff contract: [tui-contract.md](tui-contract.md). TUI recipient has not yet been supplied; no message sent.

### 2. Unify all panel construction and styling, including the right-hand UI

**Owners/files:** Codex; `ui/components.json`, `ui/app.js`, static `ui/index.html`, shared `ui/style.css`, affected view constructors and lint tests.

- [ ] Put chat, artwork and assets on the same panel component and configured looks as the other UI panels. Preserve working controls, resizing, selection and saved settings.
- [ ] Define rails and slots in the same configuration and mount them through that shared component. Remove hardcoded dock coordinates and special-case pet placement.
- [ ] Make visible indicators optional while preserving shared state. Use 💤 for idle.
- [ ] Remove old duplicated panel builders, selectors, inline visual styling and special-case overrides as each component migrates. Geometry belongs in custom properties.
- [ ] Record net code/selector reduction including shared code and configuration; do not count moved code as deleted code or compress formatting to game totals.
- [ ] Meet the existing style-contract thresholds without raising its baseline to pass. Verify normal/narrow layouts, expanded panels, dialogs, tooltips and selection synchronization.
- [ ] Commit with rendered evidence showing that the right-hand UI actually uses the shared component and style definitions.

### 3. Prove the architecture with the pets

**Owners/files:** Codex; generic companion controller, shared rails, artwork manifest and regression tests.

- [ ] Make the controller consume the same registered state read by CLI/TUI. Move command observation into the shared registry/transport, not the pet controller.
- [ ] Verify dog→error/next action, cat→last chat action/next action, both→configured homes when idle. Verify TUI-originated attention and its clear operation through CLI.
- [ ] Refuse unconfigured destinations visibly in telemetry. No special offsets or bespoke positioning to rescue a nonconforming panel.
- [ ] Verify state transitions, stale completion, project changes, reduced motion, disabling/re-enabling and shared destinations.
- [ ] Only after Gates 1–2 pass, activate the new SVG starter artwork and verify it in the app. Artwork can later be exchanged through the manifest without changing behavior.

### 4. Verify existing intake/static assembly and animation controls

- [ ] Use existing project intake, storyboard placement/save and assembly commands; fix their connections rather than adding a new workflow or approval system.
- [ ] Verify decoded audio, transcription contents/timing, a real Studio project, saved editable storyboard and static asset assembly. Motion remains optional.
- [ ] Exercise every existing applicable animation control through shared command/state: selection, play/pause, seek, loop, transforms, layer order, action/pose selection, save/reopen, reset/undo and export. Unsupported capability remains explicit.
- [ ] Use a new song as the clean acceptance project when its audio is selected. This choice does not block Gates 1–3.

### 5. Delivery

- [ ] Run `cargo check --workspace` during development and the tests the change requires. Report the real command and result. Do not claim an unrun check passed.
- [ ] Build the actual shell and CLI, verify the gates in a task-owned instance and close that instance cleanly afterward.
- [ ] Save screenshots, measurements, test results and the CLI/TUI contract under `docs/architecture/`; update this plan with actual results and blockers.
- [ ] Send the named TUI agent the branch/worktree, base/new SHAs, exact schema and commands, evidence and PR status. Jesse's authorization covers that handoff once the recipient is supplied.
- [ ] Deliver a scoped PR. Do not claim whole-app unification while gates remain unchecked. Merge only on explicit instruction.

## Ownership and reporting

The current checkout has existing staged/unstaged changes in UI lint, image prompt, index and stylesheet, plus untracked design material. Preserve them. Implementation starts from a committed baseline in a named project-local worktree per AGENTS.md, declares file ownership and coordinates overlapping files before edits. Implementation changes are isolated in the named worktree; the canonical planning document remains at this visible project path.

Progress reports name the completed behavior, commit, checks and remaining blockers. Neither an old checkbox, an output filename, a green panel nor a saved plan counts as implementation completion.

## Combined preview — 2026-10-24

Applied Claude's committed lyrics change `cfabfb7b02856ce08716746cb0aee7f8881a72ac` from `feature/lyrics-design-timeline` to this implementation worktree: exact `words.js` and marked Words CSS block, preserving shared styles. Integration is currently uncommitted; no PR exists.

The rebuilt task-owned preview runs in workspace 17 on bridge 19963 with Hey Ho Little Fish opened from project history. Six lyric sections load; frontend errors and unconfigured diagnostics are empty. Parts is hidden in this view while its component remains in telemetry. `combined-lyrics.png` records the actual app and existing video preview. Visible lyric lines are clipped at the current time-to-height scale: this is an outstanding layout issue, not a passed visual gate.

Missing-state repair now preserves independent engine errors. New Chat now acknowledges its applied engine revision; the live UI and engine both reported revision 2 after the operation. Shared controls passed at widths 700, 941 and 1261 before lyrics integration. The combined build passes workspace compilation, UI handler tests and lint ratchet; strict style-size acceptance remains unmet. ASCII pets now use the same configured rails as sprites and reject zero-width slots. New SVG art remains inactive until gates pass.

### Follow-up evidence

Checkpoint `118f5a7` includes the combined lyrics change and previous contract repairs; normal commit hooks passed. Subsequent work gives lyrics, stage and film-roll panels configured shared headers/rails, reports missing headers as red, removes the 64-state reporting cap (the real song has 279 states), and normalizes the scrubber default to its actual bounded value. `check-lyrics-layout.py` exercised 700/941/1261px with the drawer open and closed: all 12 lyric rows fit, all three rails exist, Parts remains hidden and registered, and all 279 state records are reported without truncation. Screenshots `lyrics-*-open.png` and `lyrics-*-closed.png` show the wide pinned layout and narrow stacked layout. This supersedes the earlier clipped-row finding. New SVG artwork remains inactive; style-size acceptance and remaining architecture verification are still open.

### Companion registration and field consolidation

Checkpoint `4b53c42` passed normal hooks. Follow-up work routes pet containers, sprite holders/images and the companion-state list through the shared element constructor; configured sprite dimensions fit the existing 28px rail slot. Scrolling invalidates rail geometry; zero-size and partially offscreen slots are rejected in regression tests. Duplicate motion slider rules were consolidated without changing the effective properties. Lyrics font/colour/background fields and Save now have stable configured IDs and shared styling; live records expose their indices, config IDs and theme values. Pets remain disabled while the broader style gate is incomplete.

## Scope correction — UI reserved for Claude

Jesse explicitly instructed Codex to continue the refactor and leave the UI alone. From this point Codex changes only Rust registry/CLI/TUI plumbing, its tests and handoff documentation. Do not edit `apps/magicstudio-shell/ui`, rebuild/restart the preview or apply the pending lyric-tag polish. Prior uncommitted stylesheet cleanup and inactive SVG assets remain preserved in this isolated worktree, outside main; they are not part of the backend checkpoint.

Backend follow-up: configuration validation now runs during loading for both CLI and TUI; both consumers resolve aliases through one `PanelReport::component` method. Reports retain Studio/project identity, diagnose malformed observations, reject explicitly stale/not-ready observations and detect count mismatches as incomplete. TUI cache equality includes these records instead of only revision/availability. No screen layout, style or UI behavior was edited for this change.

## Final functional acceptance — shared refactor and two pets

Jesse's latest instruction is to complete the partially implemented refactor and pets while preserving Claude's page design. The pending mechanical stylesheet cleanup was retained: no pixel or hex literals remain outside root tokens; obsolete panel selectors were removed. Live computed-style comparisons covered 1,985 lyrics DOM nodes and 2,156 animation DOM nodes across 22 properties. No displayed component changed; only the deliberately hidden retired part bar lost unused properties. Selector count is 666 versus the original 673 target. CSS/JS source-line targets remain exceeded (1,042/4,524 versus 911/4,227); these measurements must remain visible in delivery and must not be described as met.

The manifest now activates the original honey dog and orange cat, with ten decoded SVG poses. Default-on respects explicit opt-out. Live acceptance proves TUI attention and clearing, actual engine failure and recovery, missing-state red detection, most-recent command tracking, disable/re-enable, separate idle homes, and positions within measured configured slots. Both pets fit a shared slot at 700/941/1261 pixels. Regression coverage additionally proves reduced-motion cancellation, stale project events and errors beyond the first 64 states. `mc panels --check` exits 0 and full indexed inspection reports 295 unique components with finite positions in the live fixture.

Evidence: `live-pets-results.json`, `pet-layout-results.json`, `complete-components.json`, `final-panel-report.json`, `pets-idle-homes.png`, `pets-missing-state-red.png`, `pets-tui-attention.png`, and `pets-700.png`, all in this project's `docs/architecture/evidence/`. Tests use a clearly identified disposable project fixture; no song content or authored animation is fabricated or modified. Existing fish-and-bee motion engines are unchanged by this final activation.

Remaining delivery work at this checkpoint: the checks the change requires, independent final review, and scoped PR. No merge is authorized. The TUI recipient has not been supplied; the durable contract and committed code are the handoff artifact.

### Independent final review

The executing-plans final reviewer found two material registry defects: `submitted` jobs were incorrectly reported ready and their later status polls lacked an initiating component; attention could also reassign an old failed record to a new project. Both are repaired in the common registry. Job IDs now correlate status observations to their initiating component/request, including independent CLI polling; replacement-job stale completions are rejected. New-project attention goes through the existing registration reset and clears old context/receipt. Regression tests failed before these repairs and pass after them. No panel design or engine implementation changed.
