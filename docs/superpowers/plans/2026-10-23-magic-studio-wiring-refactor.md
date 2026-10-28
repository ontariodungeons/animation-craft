# Video Project Wiring Implementation Plan

> **For agentic workers:** Claim paths in the Sam ledger first (single main checkout, serialized commits). Run the tests that match the change and report the real result. Steps use checkbox (`- [ ]`) syntax. Owner: assigned by Jesse.

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

- [x] Reproduced as regression tests in `crates/engine/tests/wiring_state.rs`. See "Observed".

### Task 1: One resolver, legacy migration on open

**Files:** Modify `crates/engine/src/workshop.rs:214,455`, `crates/engine/src/video_target.rs:44,514`, `crates/engine/src/project.rs:208`. Create `crates/engine/tests/wiring_state.rs`.

**Produces:** `Session::video_project(&self) -> Option<&Path>` (stored value only); `workshop.status` returns `video_project_path` and `video_project_exists: bool`, and no longer returns `video_project`.

- [x] Tests in `wiring_state.rs`: `status_reports_only_the_stored_video_project`, `deleted_video_project_reports_missing_not_another_file`, `changing_target_does_not_keep_the_old_targets_project` (persistence/reuse already covered by `video_project_path_persists_across_reopen`).
- [x] `workshop.status` returns `video_project_path` + `video_project_exists` only; `set_target` stores the new target's project; catalog reads `video_project_exists`.
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

On main `9249ae8`:

- `workshop.status` returned two answers at once (`workshop.rs:455-456`): `video_project` = stored, else the active target's files on disk, else the **first `.fcproj` in the folder**; `video_project_path` = stored only. With no stored path and a stray `.fcproj`, they disagree; `app.js:681` reads either.
- `workshop.set_target` scaffolded the new target and **discarded the path** (`let _ = ensure_video_target_project(…)`, `workshop.rs:682`), so the stored path kept pointing at the previous target's project.
- `project_catalog.rs:104` judged "editor ready" from the guessed `video_project`.
- Already covered before this plan (`9aa046d`): persistence across reopen, `video.init` reuse.
- Left as is, deliberately: `target_project_path` / `targets[]` (on-disk view for the target chooser), `song_seconds` and the file listing (FilmCraft transcription artefacts), `video.place_references` default (FilmCraft-specific; changing it would break transcribe→place without `video.init`).
- Legacy migration dropped: an old project shows "missing" and `video.init` reuses its existing scaffold (idempotent), so no hidden write on open is needed.
