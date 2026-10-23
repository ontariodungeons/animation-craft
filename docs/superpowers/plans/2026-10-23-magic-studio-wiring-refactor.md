# Video Project Wiring Implementation Plan

> **For agentic workers:** Claim paths in the Sam ledger first (single main checkout, serialized commits). Tests run only through `.githooks/pre-commit`. Steps use checkbox (`- [ ]`) syntax. Owner: assigned by Jesse.

**Goal:** One stored answer to "where is this song's video project", read the same way by engine, UI, TUI and CLI, and visible as a panel signal when it is missing.

**Architecture:** `Session.video_project_path` already exists, is persisted in `.magic` (`project.rs:29`, `:208`), set by `video.init` (`video_target.rs:400`) and returned by `workshop.status` (`workshop.rs:456`). The remaining fault is that the path is still *derived* in several places, so callers can disagree. This plan makes the stored field the only source, migrates legacy projects once on open, and reports wiring through the panel contract in `2026-10-23-unified-panel-reporting.md` instead of a new UI field.

**Tech Stack:** Rust engine (`magicstudio-engine`), Tauri shell (`magicstudio-shell`), vanilla JS UI, `mc`.

**Spec:** chat with Jesse 2026-10-23; panel contract in `docs/superpowers/plans/2026-10-23-unified-panel-reporting.md`.

## What changed from the earlier draft (and why)

| Earlier draft | Now | Reason |
|---|---|---|
| Phase 0: remove `Cell<u64> project_saves` | dropped | already `AtomicU64` (`session.rs:56`) |
| Phase 1: add `video_project_path` | dropped | exists, persisted, returned |
| Actor pattern / tokio / `mpsc` / `ProjectStore` trait | dropped | no tokio in the workspace; shell uses `Mutex<Session>` (`magicstudio-shell/src/main.rs:36`); a rewrite of session ownership is not needed to fix wiring |
| `animationcraft-shell`, `.acproj` | `magicstudio-shell`, `.magic` | rename `52607fd` |
| Phase 3 UI fields | panel record | one reporting path for TUI/CLI |

## Global Constraints

- The stored `Session.video_project_path` is the only source after open. No module derives it at read time.
- Legacy projects (stored `None`, scaffold on disk) are migrated once on `project.open`, and the project is marked unsaved — never silently saved.
- Facts only: a missing path is reported as missing, never guessed.
- No new UI state field; wiring status is a panel (`signal`) per the panel contract.

## Review Focus

- Project opened with stored path pointing at a deleted directory → `workshop.status` reports it missing (signal 3), does not fall back to a derived path.
- Legacy `.magic` with no `workshop.video_project` but an existing scaffold → migrated once, unsaved flag set.
- `video.init` called twice → same path, no second scaffold (unless `replace_scaffold=true`).
- Video target changed (hyperframes → filmcraft) → path cleared and re-initialised by `video.init`, not left pointing at the old target.
- UI reading an older Studio's `workshop.status` that still has `video_project` only → shows missing, not a crash.

---

### Task 0: Reproduce the reported failure first

The original report: an agent said "no project created", or a created project was not wired into the app.

- [ ] Reproduce with `mc exec project.intake …` then `mc exec workshop.status`; record the exact output in this file under "Observed". If it no longer reproduces on current main, note the SHA and continue with Tasks 1–3 as hardening only.

### Task 1: One resolver, legacy migration on open

**Files:** Modify `crates/engine/src/workshop.rs:214,455`, `crates/engine/src/video_target.rs:44,514`, `crates/engine/src/project.rs:208`. Create `crates/engine/tests/wiring_state.rs`.

**Produces:** `Session::video_project(&self) -> Option<&Path>` (stored value only); `workshop.status` returns `video_project_path` and `video_project_exists: bool`, and no longer returns `video_project`.

- [ ] Tests in `wiring_state.rs`: `intake_then_init_then_reopen_keeps_path`; `init_twice_reuses_scaffold`; `deleted_directory_reports_missing_not_derived`; `legacy_project_migrates_once_and_is_unsaved`; `target_change_clears_path`.
- [ ] Replace read-time derivations with `Session::video_project()`. The scaffold-locating helper (`workshop.rs:214 video_project_path(dir)`) is called only from the `project.open` migration and from `video.init`.
- [ ] Commit `fix(engine): the stored video project is the only answer (#23)`.

### Task 2: UI and CLI read one field

**Consumes:** Task 1 `workshop.status.video_project_path`, `video_project_exists`.
**Files:** Modify `apps/magicstudio-shell/ui/app.js:681,701`; `apps/magicstudio-shell/src/main.rs:133` (already mirrors the field — confirm no derivation).

- [ ] `app.js:681` reads `video_project_path` only (drop the `video_project` fallback).
- [ ] Commit `fix(ui): video tab reads the stored video project only (#23)`.

### Task 3: Wiring is a panel signal

**Consumes:** panel contract Task 2 (`companionBoxes.register`, `flag`), `workshop.status`.
**Files:** Modify `apps/magicstudio-shell/ui/app.js` (video/studio tab registration).

- [ ] Register the video panel; its state is `failed` when `video_target` is set and `video_project_exists` is false, `needs-attention` when no target is set, `ready` when the path exists. The TUI and `mc panels` then show it with no new code.
- [ ] Test in `apps/magicstudio-shell/tests/panels.mjs`: `video panel is red when target set and project missing`.
- [ ] Commit `feat(ui): video wiring reports as a panel (#23)`.

## Out of scope

- Session ownership rewrite (actor/tokio) — revisit only if a measured lock problem appears.
- Replacing HyperFrames/Remotion/FilmCraft.

## Observed

(Task 0 output goes here.)
