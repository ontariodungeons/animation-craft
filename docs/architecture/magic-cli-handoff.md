> Magic Studio entry-point and command architecture: [current contract](../../agent-harness/MAGIC.md). `magic` opens the GUI; `magic-cli` and `magic-cli mcp` share the application registry. Do not restore retired `mc`, REPL or duplicated routing from historical instructions.

# Magic CLI & MCP Handoff to Bunzen (`mc` Deletion)

- **Branch**: `main`
- **Worktree**: `/run/media/jesse/Speedy/Work/omhas-animation`
- **Base commit**: `5775e1e2c524fd4a988101c1343217c42df9a42f`
- **Implementation commits**: `0c0795d7f9a64e62c9876fc1faa47f33c836a5e8` .. `HEAD`
- **PR URL**: No PR exists yet — committed directly on canonical `main` per Jesse's instruction (*"I want you to begin by implementing magic and when you visually can confirm the new magic mcp and cli works you delegate the deletion to Bunzen"*).

## 1. What Was Implemented & Verified

1. **CLI-Anything `magic` Harness (`agent-harness/cli_anything/magic/`)**:
   - Built with Click + `ReplSkin` per the CLI-Anything methodology (`setup.py` entry point `magic=cli_anything.magic.magic_cli:main`).
   - Installed to `~/.local/bin/magic`, `target/debug/magic`, and `~/.local/share/magic-studio/bin/magic`.
   - Unified session routing (`agent-harness/cli_anything/magic/core/session.py`): `magic studio <project>` or `magic project open <project>` binds the active session context so all subsequent commands (`magic storyboard`, `magic character`, `magic motion`, `magic video`, `magic words`, `magic skills`, `magic crew`, `magic ui`, `magic exec`, `magic run`, `magic doctor`, and `magic mcp`) route to the bound live Studio Unix socket or headless engine (`magicstudio-shell --headless`) without per-call `--bridge` flags.
   - No cross-domain undo ledger; each domain uses its native undo command (`motion.edit.undo`, `motion.undo_path`, `part.undo_image`, `storyboard.update restore=...`, `skills.undo`).
2. **Headless Engine in `magicstudio-shell` (`apps/magicstudio-shell/src/main.rs`)**:
   - Added `magicstudio-shell --headless commands`, `magicstudio-shell --headless exec <id> <params_json> [--dry-run]`, and `magicstudio-shell --headless run`, eliminating any dependency on `apps/mc`.
   - Updated `apps/magicstudio-shell/src/bridge.rs` so `project.save` flushes pending UI storyboard text drafts (`saveStoryboard()`) before saving `<project>.magic`.
3. **HyperFrames Studio Website Recognition (`crates/engine/src/video_target.rs`, `workshop.rs`, `project_catalog.rs`)**:
   - Projects with `hyperframes/index.html` are recognized directly as existing HyperFrames studio websites without requiring a separate project name or `hyperframes/hyperframes.json`.
4. **Analytics Diagram & Excel Inventory**:
   - Generated and validated `apps/magicstudio-shell/ui/analytics-diagrams/magic-cli-architecture.html` via Archify (`finalize architecture --quality showcase` passed `validate`, `deliver`, `check`, and `browser-check`).
   - Updated `apps/magicstudio-shell/ui/analytics.js`, `crates/engine/src/complexity.rs`, `crates/engine/src/doctor.rs`, `docs/functionality-inventory.xlsx` (`gridcraft-cli check` passed with 0 diffs), `docs/magic-cli-proposed.mmd`, and `docs/magic-studio-current.mmd`.

## 2. Test Commands & Preview Evidence

- `CARGO_BUILD_JOBS=1 cargo build -p magicstudio-shell` — **PASSED**
- `CLI_ANYTHING_FORCE_INSTALLED=1 python3 -m pytest /run/media/jesse/Speedy/Work/omhas-animation/agent-harness/cli_anything/magic/tests/ -v --tb=no` — **19/19 PASSED**
- `gridcraft-cli check docs/functionality-inventory.xlsx --json` — **PASSED (0 diffs)**
- `node /home/jesse/.codex/skills/archify/bin/archify.mjs finalize architecture .archify/architecture-magic-cli-20261028-010000/candidate.json .archify/architecture-magic-cli-20261028-010000/magic-cli-architecture.html --quality showcase --json` — **PASSED**
- **Live Studio Visual Verification**:
  - Launched `/tmp/magic-visual-verify/VerifySong.magic` via `magic studio` (PID `181417`, control port `40791`, build `5775e1e2-1793163782` on `main`), verified Project view, Studio view with live HyperFrames website (`http://127.0.0.1:3002/#project/hyperframes`), and Analytics view with the new `Magic CLI & Studio architecture` diagram and `magic exec` command reference via `magic ui look` and `magic mcp`, then cleanly stopped the server and closed the window via `magic app quit`.

## 3. Scope Decision for Bunzen (Answered)

- **Backstage / `apps/mc` TUI (`center`, `tui`, `workshop` TUI)**:
  - Jesse explicitly stated in the initial task scope: *"Backstage is outside this review."* and *"1. I want you to begin by implementing magic and when you visually can confirm the new magic mcp and cli works you delegate the deletion to Bunzen 2. I think that's all the live preview and manipulation I really need. All ui should be driven with /cli-anything"*.
  - Furthermore, `magic run` and `magic doctor` are now present on `magic` (`agent-harness/cli_anything/magic/magic_cli.py`).
  - Therefore: **Do NOT preserve or migrate the old Ratatui `center`/`tui` from `apps/mc` — delete `apps/mc` completely along with its TUI modules.**

## 4. Bunzen's Bounded Task

1. Delete `apps/mc` (`git rm -r apps/mc`) and remove `"apps/mc"` from the root `Cargo.toml` workspace members.
2. Remove the `apps/mc/src` fallback check in `crates/engine/src/complexity.rs` if desired, and clean up any remaining build/workspace references to the `mc` crate.
3. Preserve untracked `docs/image-to-editable-animation.mmd` and `.archify/`.
4. Run `CARGO_BUILD_JOBS=1 cargo check --workspace` and `CLI_ANYTHING_FORCE_INSTALLED=1 python3 -m pytest agent-harness/cli_anything/magic/tests/ -q` and commit the deletion cleanly on `main`.
