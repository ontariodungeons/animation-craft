# Magic entry points — review checkpoint, 2026-10-28

This is a review checkpoint requested by Jesse, not a claim that all historical cleanup and installed configuration migration is complete. Read `agent-harness/MAGIC.md` and the packaged Magic skill first.

## Implemented

- `magic` launches the real Tauri shell; `magic-cli` has a registry-derived Click hierarchy and help, with no REPL.
- `magic-cli mcp` uses official Python SDK 1.x (`mcp>=1.28,<2`; installed/verified 1.30.0). Protocol negotiation belongs to the SDK.
- CLI and MCP share project/window session routing; Python no longer creates or saves Magic project documents.
- Registry descriptions and dry-run decisions are shared in Rust. Window command metadata is in the registry.
- UI project actions directly enter guarded application dispatch. Removed the UI → CLI subprocess → same-window bridge loop.
- Updated internal executable callers, source installer, desktop template, MCP example and Magic skill.
- Removed the `motion.recipe` Rust subprocess wrapper. Its existing compiler/renderer scripts remain, using `magic-cli --project` without user-selected ports. See MAGIC.md for the replacement workflow.

## Observed verification

- `CARGO_BUILD_JOBS=1 cargo build -p magicstudio-shell`: passed.
- `CARGO_BUILD_JOBS=1 cargo test -p magicstudio-engine --lib commands::tests`: 5 existing tests passed. No tests were added/restored.
- `.venv/bin/python -m compileall -q agent-harness/cli_anything/magic`: passed.
- `uv pip install --python .venv/bin/python 'mcp>=1.28,<2' click setuptools`; editable harness registration; `python3 scripts/install_magic.py`: installed Magic only.
- Bare `magic` in a persistent terminal opened the actual Magic Studio home window; inspected a native screenshot. Short-lived background launch attempts did not survive the command runner; the persistent terminal launch did.
- Exercised the real UI `openProject` handler on `/tmp/magic-entry-review/Review.magic`, verifying the new direct dispatch path.
- `magic-cli --session-file /tmp/magic-entry-review/session.json project new path=/tmp/magic-entry-review/Review.magic`, then open, `project add-asset ... image=review-icon`, save, close, reopen and list-assets: the asset persisted. Import without an explicit external-image destination was correctly refused; adding `image=review-icon` succeeded.
- A real `mcp.ClientSession` with `stdio_client` launched installed `magic-cli ... mcp`, initialized, listed tools, called `command_list`, ran `project.update_asset`, saved and read back the file. Negotiated `2025-11-25`; tools were `command_list`, `command_run`, `studio_open`.
- Repeated against the live project. Initially the SDK's sanitized child environment omitted XDG_RUNTIME_DIR; native writer ownership prevented a competing write. Added Linux same-user runtime discovery. Rerun returned the live Unix endpoint and UI revisions 6/7, with `isError=false`; saved file and visible Assets panel both showed **MCP live verified**.
- [Visible final result](evidence/magic-entry-points-live.png). The disposable PNG asset was copied from this app's existing icon, not generated artwork or a fake application.
- Saved and closed only the verification window through `app.quit`; its process exited. Subsequent pinned operations failed instead of falling back to another window or headless writes.
- `git diff --check`: passed.

## Remaining work / review priorities

1. Complete migration of this machine's old `mc` executable and global Codex/OpenCode MCP entries to `magic-cli mcp`. The new local entry points and new desktop entry are installed, but old aliases and the legacy animationcraft desktop entry still need removal or replacement. Do not modify unrelated Craft servers. Cloud agents cannot verify this machine's global configuration.
2. Review all remaining old command text, generated guidance, skill copies, diagrams and executable references. Prefer the shared registry and direct installed app CLIs; do not rebuild generic wrappers or aggregate Craft MCP servers. Some historical registry aliases (`booth`, `center`, `sprocket`) remain visible now that discovery truthfully returns the registry; decide removals with caller evidence.
3. Review the new dynamic Click tree, especially IDs that are also command prefixes, scripts that depended on old friendly aliases, and batch behavior. Parent commands use --params; leaf commands accept key=value or --params. Do not restore a second command table to repair callers.
4. Review session routing and dry-run edges (multiple windows, changed projects, failed open, Save As, explicit bridge overrides), preserving writer leases and pending/dirty UI guards. The exercised round trip is not exhaustive coverage.
5. The motion recipe and background native-worker caller migrations were syntax/build checked, not rendered end-to-end. The full GUI/domain suite was not run. Preserve legacy persisted job data where needed while removing executable fallbacks.
6. Bunzen is investigating backlot/OpenMontage instructions; Shrubs is investigating the TUI. Both have read-only exploratory assignments and must ask Jesse three questions and wait. Do not implement their plans without Jesse's approval and integration review with Grober.

## User-owned working-tree state

Before this refactor, Jesse had deleted root AGENTS.md, the harness tests/TEST.md, test_core.py and test_full_e2e.py, seven pipeline test files, and seven files under tests/. Those deletions were deliberately not staged in this checkpoint, and no deleted file was restored. The untracked `design/animation/controls/gaffer-mouth-liquify-preview.png` is also unrelated and excluded. In a cloud checkout the old tracked files may therefore still exist: their old architectural assertions are not authority to undo this refactor. Ask Jesse before adding tests and present the proposed cases first.
