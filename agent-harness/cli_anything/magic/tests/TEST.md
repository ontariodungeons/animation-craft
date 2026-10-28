# Magic Studio CLI-Anything Test Plan & Results (`TEST.md`)

## Part 1: Test Plan

### 1. Test Inventory Plan
- `test_core.py`: 12 unit tests planned (project creation, opening, saving, HyperFrames website detection without `hyperframes.json`, session locking, local undo/redo, export presets, CLI argument parsing).
- `test_full_e2e.py`: 8 E2E and subprocess tests planned (real `magicstudio-shell --headless` backend execution, `.magic` project round-trip, HyperFrames website detection via `workshop.status`, MCP `initialize` / `tools/list` / `tools/call` over stdio, and `TestCLISubprocess` via `_resolve_cli("magic")` with `CLI_ANYTHING_FORCE_INSTALLED=1`).

### 2. Unit Test Plan (`test_core.py`)
- **`project.py`**:
  - `test_list_profiles`: Verify `hyperframes`, `remotion`, and `character` profiles.
  - `test_create_and_open_project`: Create `.magic` project in temp dir, verify JSON schema and format.
  - `test_create_project_refuses_overwrite`: Verify `FileExistsError` when `overwrite=False`.
  - `test_resolve_project_from_hyperframes_website`: Verify `resolve_project_path` resolves a song project from its `hyperframes/index.html` website without `hyperframes.json`.
  - `test_project_info_detects_hyperframes_website`: Verify `info()` reports `hyperframes.exists == True` when `hyperframes/index.html` exists without `hyperframes.json`.
- **`session.py`**:
  - `test_locked_save_json_persists_atomically`: Verify `_locked_save_json` writes valid JSON.
  - `test_session_set_project_and_reload`: Verify session state persists across `Session` instances.
  - `test_session_snapshot_undo_redo`: Verify in-memory project snapshot undo and redo.
  - `test_session_closed_studio_refuses_fallback`: Verify that a bound Studio endpoint whose socket is gone raises `RuntimeError` instead of silently falling back to another window or headless.
- **`export.py` & `magic_cli.py`**:
  - `test_export_presets`: Verify `storyboard_png`, `video_snapshot`, `video_mp4`, `layered_motion_webm`.
  - `test_parse_kv_args`: Verify boolean, integer, float, JSON, and string parsing.
  - `test_preprocess_argv_studio_shorthand`: Verify `magic studio MySong.magic` normalizes to `magic studio open MySong.magic`.

### 3. E2E & Subprocess Test Plan (`test_full_e2e.py`)
- **Real Backend Integration**:
  - Invoke `magicstudio-shell --headless` through `magic` to create a project, inspect `workshop.status`, verify `hyperframes/index.html` website detection in the Rust engine, and mutate project state with auto-save.
  - Verify `--dry-run` suppresses saving to `<project>.magic`.
- **MCP Server E2E**:
  - Send `initialize` (both default `2024-11-05` and `2026-07-28`), `notifications/initialized`, `tools/list`, and `tools/call` (`project_identity`, `workshop_status`) over stdio and verify JSON-RPC 2.0 responses.
- **`TestCLISubprocess`**:
  - Use `_resolve_cli("magic")` without setting `cwd`.
  - Test `--help`, `--json project new`, `project info`, `commands`, and `mcp`.


## Part 2: Test Results

```text
============================= test session starts ==============================
platform linux -- Python 3.15.2, pytest-9.2.0, pluggy-1.7.0 -- /home/jesse/.local/share/mise/installs/python/latest/bin/python3
cachedir: .pytest_cache
rootdir: /run/media/jesse/Speedy/Work/omhas-animation/agent-harness
collected 19 items

agent-harness/cli_anything/magic_studio/tests/test_core.py::test_list_profiles PASSED [  5%]
agent-harness/cli_anything/magic_studio/tests/test_core.py::test_create_and_open_project PASSED [ 10%]
agent-harness/cli_anything/magic_studio/tests/test_core.py::test_create_project_refuses_overwrite PASSED [ 15%]
agent-harness/cli_anything/magic_studio/tests/test_core.py::test_resolve_project_from_hyperframes_website PASSED [ 21%]
agent-harness/cli_anything/magic_studio/tests/test_core.py::test_project_info_detects_hyperframes_website_without_json PASSED [ 26%]
agent-harness/cli_anything/magic_studio/tests/test_core.py::test_locked_save_json_persists_atomically PASSED [ 31%]
agent-harness/cli_anything/magic_studio/tests/test_core.py::test_session_set_project_and_reload PASSED [ 36%]
agent-harness/cli_anything/magic_studio/tests/test_core.py::test_session_snapshot_undo_redo PASSED [ 42%]
agent-harness/cli_anything/magic_studio/tests/test_core.py::test_session_closed_studio_refuses_fallback PASSED [ 47%]
agent-harness/cli_anything/magic_studio/tests/test_core.py::test_export_presets PASSED [ 52%]
agent-harness/cli_anything/magic_studio/tests/test_core.py::test_parse_kv_args PASSED [ 57%]
agent-harness/cli_anything/magic_studio/tests/test_core.py::test_preprocess_argv_studio_shorthand PASSED [ 63%]
agent-harness/cli_anything/magic_studio/tests/test_full_e2e.py::TestCLISubprocess::test_help PASSED [ 68%]
agent-harness/cli_anything/magic_studio/tests/test_full_e2e.py::TestCLISubprocess::test_project_new_and_info_json PASSED [ 73%]
agent-harness/cli_anything/magic_studio/tests/test_full_e2e.py::TestCLISubprocess::test_dry_run_suppresses_project_save PASSED [ 78%]
agent-harness/cli_anything/magic_studio/tests/test_full_e2e.py::TestCLISubprocess::test_headless_engine_hyperframes_website_roundtrip PASSED [ 84%]
agent-harness/cli_anything/magic_studio/tests/test_full_e2e.py::TestCLISubprocess::test_headless_mutation_auto_saves_to_magic_file PASSED [ 89%]
agent-harness/cli_anything/magic_studio/tests/test_full_e2e.py::TestCLISubprocess::test_commands_discovery PASSED [ 94%]
agent-harness/cli_anything/magic_studio/tests/test_full_e2e.py::TestCLISubprocess::test_mcp_stdio_protocol_and_tool_call PASSED [100%]

============================== 19 passed in 0.92s ==============================
```

### Summary Statistics
- Total Tests: 19
- Passed: 19 (100%)
- Execution Time: 0.92s
