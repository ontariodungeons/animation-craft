# Animation frame implementation plan

**Goal:** Port the complete approved Animation workspace into the live shell, then animate/export and inspect the bee.
**Architecture:** Extract the existing Rust evaluator into engine commands; the canvas draws samples. UI and CLI/MCP share the window session. The inner agent validates operations and Craft connections while the coordinator implements.
**Stack:** Existing Tauri 2.x/Rust, vanilla JS/CSS, Craft CLI pipeline.
**Spec:** `.agents/skills/magicstudio-shell-ui/research.md`, approved by the user on 2026-10-21; HANDOFF is the behavior/source inventory.

## Constraints and review focus

- Work on the assigned `feat/production-pipeline` branch and shell 19897. Reference 19898 only for comparison; close it afterwards. Never edit reference UI.
- Engine motion must match the Python exporter, including alternate opacity, parents, crop pivots, follow and boundaries. No JS evaluator.
- Keep controls left, stage generous and useful after import, timing below. Preserve the user's draft on restarts.
- Every visible slice: rebuild, bridge-drive, open `ui.look`, inner-agent operational validation, explicit-path commit.
- Only touched stable dependencies; no alpha/beta. Rig/matting use Craft chains. Intake, pose-sheet and 3D UI form still need user decisions.
- Verify malformed specs, missing art, non-loop endpoints, stale selection during jobs, and narrow windows with the drawer open.

## Tasks

- [ ] Engine sampling: add `crates/engine/src/motion.rs`, register `motion.tracks` and `motion.sample`; extract `center_motion` helpers with plain inputs and validated spec data. Return metadata and frame layer matrices/opacity. Test exporter parity with bee plus synthetic numeric/parent fixtures, invalid inputs, layer inclusion and rest. Compile/check; commit through hooks.
- [ ] Animation frame: `ui/animation.js`, HTML/CSS/app navigation, Rust engine invoke. Load selected artwork, rest pose, track picker, play/pause/scrub, fit/100%/zoom/center. Bind state to bridge and CLI updates. Verify pixels and selection with inner agent; commit.
- [ ] Layer/pose controls: use existing part commands, engine control bindings and action behavior. Disclose unsupported operations honestly. Add reusable typed commands where missing; expose state alternatives, review/solo/show-all, anchors/labels/separation, valid pose parameters. Verify edits, reload/persistence and undo; commit.
- [ ] Song timing: waveform/cues from actual song analysis, engine-owned source/song time mapping, placed motion instances and persisted edits. Verify seek/frame/loop agreement with audio, no invented beats, clear conflicts; commit.
- [ ] Craft jobs/exports: research and record each option's exact command contract, add visible progress/results, verify EffectCraft layered video and validation, inspect bee export in selected target tool. Never hand-patch exported output; commit.
- [ ] Remaining migration: walk every HANDOFF item, read its source behavior and implement in approved scope. Mark entries only with verified port commit. Ask on the three ❓ groups, preserve them pending answers.

## Execution record

- Design approved; no extra plan approval requested. Existing branch ownership supersedes generic isolated-worktree defaults.
- Inner validator assigned CLI-Anything and Craft contract research; coordinator owns source edits.
