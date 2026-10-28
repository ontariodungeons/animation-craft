# Magic Studio CLI-Anything Test Plan & Results (`TEST.md`)

## Part 1: Test Plan

### 1. Test Inventory Plan
- `test_core.py`: 15 unit tests (project creation, opening, saving, HyperFrames website detection without `hyperframes.json`, session locking, live bridge mutation non-overwrite guarantee, `--dry-run` write suppression across headless & live bridge, MCP bridge UI tool discovery & dispatch, closed-studio error handling, export presets, CLI argument parsing, and first-class Click domain coverage with zero legacy prefixes).
- `test_full_e2e.py`: 11 E2E and subprocess tests (real `magicstudio-shell --headless` backend execution, `.magic` project round-trip, HyperFrames website detection via `workshop.status`, one-shot live bridge mutation without overwriting `.magic`, `--dry-run project save -o` write suppression, `magic run` JSONL batch execution across headless and live bridge, save failure non-zero exit propagation, and MCP `initialize` / `tools/list` / `tools/call` over stdio).

### 2. Unit Test Plan (`test_core.py`)
- **`project.py`**:
  - `test_list_profiles`: Verify `hyperframes`, `remotion`, and `character` profiles.
  - `test_create_and_open_project`: Create `.magic` project in temp dir, verify JSON schema and format.
  - `test_create_project_refuses_overwrite`: Verify `FileExistsError` when `overwrite=False`.
  - `test_resolve_project_from_hyperframes_website`: Verify `resolve_project_path` resolves a song project from its `hyperframes/index.html` website without `hyperframes.json`.
  - `test_project_info_detects_hyperframes_website_without_json`: Verify `info()` reports `hyperframes.exists == True` when `hyperframes/index.html` exists without `hyperframes.json`.
- **`session.py` & `mcp_server.py`**:
  - `test_locked_save_json_persists_atomically`: Verify `_locked_save_json` writes valid JSON.
  - `test_session_set_project_and_reload`: Verify session state persists across `Session` instances.
  - `test_live_bridge_mutation_does_not_overwrite_project`: Verify live bridge mutations written to `.magic` by the Rust engine are never overwritten by stale Python session snapshots.
  - `test_dry_run_suppresses_live_and_headless_writes`: Verify `Session.execute(..., dry_run=True)` returns a dry-run receipt without writing files or mutating state across both headless and live bridge targets.
  - `test_mcp_advertises_all_bridge_ui_tools_and_dispatches`: Verify `build_mcp_tools()` advertises all bridge UI tools (`ui_look`, `ui_click`, `ui_set`, `ui_key`, `ui_state`, `ui_elements`, `ui_drag`, `ui_screenshot`, `ui_focus`, `ui_eval`, `ui_unconfigured`, `app_open`, `app_quit`) and dispatches them over a live bridge socket.
  - `test_session_closed_studio_refuses_fallback`: Verify that a bound Studio endpoint whose socket is gone raises `RuntimeError` instead of silently falling back to another window or headless.
- **`export.py` & `magic_cli.py`**:
  - `test_export_presets`: Verify `storyboard_png`, `video_snapshot`, `video_mp4`, `layered_motion_webm`.
  - `test_parse_kv_args`: Verify boolean, integer, float, JSON, and string parsing.
  - `test_preprocess_argv_studio_shorthand`: Verify `magic studio MySong.magic` normalizes to `magic studio open MySong.magic`.
  - `test_cli_domain_groups_and_no_legacy_prefixes`: Verify all 19 first-class Click domain groups and ensure zero legacy `backstage`/`booth`/`center`/`tui`/`sprocket` commands exist.

### 3. E2E & Subprocess Test Plan (`test_full_e2e.py`)
- **Real Backend Integration & Subprocess E2E (`TestCLISubprocess`)**:
  - `test_help`: Verify `magic --help`.
  - `test_project_new_and_info_json`: Create a `.magic` project and inspect `magic --json project info`.
  - `test_dry_run_suppresses_project_save`: Verify `magic --dry-run project new -o <path>` writes no file.
  - `test_headless_engine_hyperframes_website_roundtrip`: Verify `magic video status` detects `hyperframes/index.html` without `hyperframes.json`.
  - `test_headless_mutation_auto_saves_to_magic_file`: Verify headless mutations persist to `<project>.magic`.
  - `test_commands_discovery`: Verify `magic --json commands storyboard`.
  - `test_mcp_stdio_protocol_and_tool_call`: Verify MCP JSON-RPC `initialize`, `tools/list` (including `ui_click`, `ui_set`, `ui_key`, `ui_state`, `ui_look`), and `tools/call` over stdio.
  - `test_cli_live_bridge_mutation_does_not_overwrite_project_file`: Verify one-shot `magic --bridge unix:<sock> project add-asset ...` preserves the bridge's `.magic` file changes on disk.
  - `test_dry_run_project_save_o_does_not_create_file`: Verify `magic --dry-run project save -o <temp-path>` does not create `<temp-path>`.
  - `test_magic_run_headless_and_live_bridge`: Verify `magic run -` executes JSONL command batches in both headless mode and over a live Studio bridge.
  - `test_headless_save_error_propagates_nonzero`: Verify read-only `.magic` write failures propagate and exit non-zero.

## Part 2: Test Results

```text
============================= test session starts ==============================
platform linux -- Python 3.15.2, pytest-9.2.0, pluggy-1.7.0 -- /home/jesse/.local/share/mise/installs/python/latest/bin/python3
cachedir: .pytest_cache
rootdir: /run/media/jesse/Speedy/Work/omhas-animation/agent-harness
collecting ... [_resolve_cli] Using installed command: /home/jesse/.local/share/mise/installs/python/latest/bin/magic
collected 26 items

agent-harness/cli_anything/magic/tests/test_core.py::test_list_profiles PASSED
agent-harness/cli_anything/magic/tests/test_core.py::test_create_and_open_project PASSED
agent-harness/cli_anything/magic/tests/test_core.py::test_create_project_refuses_overwrite PASSED
agent-harness/cli_anything/magic/tests/test_core.py::test_resolve_project_from_hyperframes_website PASSED
agent-harness/cli_anything/magic/tests/test_project_info_detects_hyperframes_website_without_json PASSED
agent-harness/cli_anything/magic/tests/test_core.py::test_locked_save_json_persists_atomically PASSED
agent-harness/cli_anything/magic/tests/test_core.py::test_session_set_project_and_reload PASSED
agent-harness/cli_anything/magic/tests/test_core.py::test_live_bridge_mutation_does_not_overwrite_project PASSED
agent-harness/cli_anything/magic/tests/test_core.py::test_dry_run_suppresses_live_and_headless_writes PASSED
agent-harness/cli_anything/magic/tests/test_core.py::test_mcp_advertises_all_bridge_ui_tools_and_dispatches PASSED
agent-harness/cli_anything/magic/tests/test_core.py::test_session_closed_studio_refuses_fallback PASSED
agent-harness/cli_anything/magic/tests/test_core.py::test_export_presets PASSED
agent-harness/cli_anything/magic/tests/test_core.py::test_parse_kv_args PASSED
agent-harness/cli_anything/magic/tests/test_core.py::test_preprocess_argv_studio_shorthand PASSED
agent-harness/cli_anything/magic/tests/test_core.py::test_cli_domain_groups_and_no_legacy_prefixes PASSED
agent-harness/cli_anything/magic/tests/test_full_e2e.py::TestCLISubprocess::test_help PASSED
agent-harness/cli_anything/magic/tests/test_full_e2e.py::TestCLISubprocess::test_project_new_and_info_json PASSED
agent-harness/cli_anything/magic/tests/test_full_e2e.py::TestCLISubprocess::test_dry_run_suppresses_project_save PASSED
agent-harness/cli_anything/magic/tests/test_full_e2e.py::TestCLISubprocess::test_headless_engine_hyperframes_website_roundtrip PASSED
agent-harness/cli_anything/magic/tests/test_full_e2e.py::TestCLISubprocess::test_headless_mutation_auto_saves_to_magic_file PASSED
agent-harness/cli_anything/magic/tests/test_full_e2e.py::TestCLISubprocess::test_commands_discovery PASSED
agent-harness/cli_anything/magic/tests/test_full_e2e.py::TestCLISubprocess::test_mcp_stdio_protocol_and_tool_call PASSED
agent-harness/cli_anything/magic/tests/test_full_e2e.py::TestCLISubprocess::test_cli_live_bridge_mutation_does_not_overwrite_project_file PASSED
agent-harness/cli_anything/magic/tests/test_full_e2e.py::TestCLISubprocess::test_dry_run_project_save_o_does_not_create_file PASSED
agent-harness/cli_anything/magic/tests/test_full_e2e.py::TestCLISubprocess::test_magic_run_headless_and_live_bridge PASSED
agent-harness/cli_anything/magic/tests/test_full_e2e.py::TestCLISubprocess::test_headless_save_error_propagates_nonzero PASSED

============================== 26 passed in 1.48s ==============================
```

### Summary Statistics
- Total Tests: 26
- Passed: 26 (100%)
- Execution Time: 1.48s
