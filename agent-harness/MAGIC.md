# Agent Harness: Magic Studio (`magic` / `magic`)

## 1. Architecture Analysis

Magic Studio (`magicstudio-shell`) separates its native Tauri presentation from its Rust command registry (`magicstudio-engine`).
- **Backend Engine**: `magicstudio-engine` (`Session::execute(id, params)`), exposed both:
  - **Headlessly** via `magicstudio-shell --headless <command_id> <params_json>`
  - **Live in an open Studio window** via the window's private Unix domain socket (`$XDG_RUNTIME_DIR/magicstudio-studio/<pid>-<nonce>.sock`) and optional `--control <port>` TCP JSON-lines bridge.
- **Data Model**:
  - `.magic` project files (`format: "magicstudio.project"`, `version: 1`)
  - `.deckcraft` native storyboard decks (`storyboard/*.deckcraft`) and rendered page PNGs (`storyboard/.preview/page-*.png`)
  - `assets/characters/<id>/character.magic` + `rig.json` + `parts/*.png`
  - `hyperframes/index.html` — a website inside the project directory that works directly in the Studio app without requiring a separate project name or `hyperframes.json`.
- **Undo / Redo System (Domain-Local Only — No Cross-Domain Ledger)**:
  - Each domain maintains its own truthful undo stack:
    - `motion.edit.undo` (Theatre keyframe edits)
    - `motion.undo_path` (authored motion paths)
    - `part.undo_image` (part raster replacements)
    - `storyboard.update restore=<entry>` (storyboard history entries)
    - `skills.undo` (skill changes)

## 2. Unified Session & Context Routing

The agent never chooses between a UI route and a headless route per operation:
1. **`magic studio new` / `magic studio <project>`**:
   - Opens or focuses the Magic Studio window (`magicstudio-shell`), waits for `ui_ready == true`, and pins the live Studio endpoint (`unix:<socket>`) in the active session file.
2. **`magic project open <project>` / `magic project new <path>`**:
   - If an open Studio window already owns that `.magic` project, binds to that window's `unix:<socket>` and focuses it.
   - Otherwise, binds the canonical `.magic` path in the session file (`_locked_save_json`) and runs headlessly with auto-save (`--dry-run` suppresses saving).
3. **Subsequent Commands (`magic storyboard ...`, `magic character ...`, `magic motion ...`, `magic video ...`, `magic ui ...`, `magic exec ...`)**:
   - Automatically check if the bound `.magic` project is open in a live Studio window; if so, dispatch over that window's socket so the visible UI updates immediately.
   - If bound to a live Studio window that has closed or switched projects, refuse with an explicit error — **never** silently fall back to another window or headless state.
   - Otherwise execute headlessly against the `.magic` file and auto-save mutations.

## 3. Command Map

| Group | Commands | Engine / UI Mapping |
|-------|----------|---------------------|
| `studio` | `new`, `open`, `list`, `status`, `unpin`, `quit` | `studio_sessions::open`, `ui.feedback`, `app.quit` |
| `project` | `new`, `open`, `list`, `save`, `close`, `reveal`, `info`, `profiles`, `add-asset`, `list-assets` | `project.new`, `project.open`, `project.catalog`, `project.save`, `project.close`, `workshop.status`, `project.add_asset`, `project.list_assets` |
| `storyboard` | `create`, `read`, `update`, `delete`, `edit`, `open`, `build`, `render`, `doctor` | `storyboard.create/read/update/delete`, `workshop.storyboard`, `workshop.build_storyboard`, `storyboard.render`, `storyboard.doctor` |
| `character` | `list`, `import`, `configure`, `cut-sheet`, `build-rig` | `character.assets`, `character.import`, `character.configure`, `asset.cut_sheet`, `asset.build_cutout_rig` |
| `motion` | `tracks`, `clips`, `sample`, `keyframe`, `undo`, `path`, `undo-path`, `recipe` | `motion.tracks`, `motion.clips`, `motion.sample`, `motion.edit.keyframe`, `motion.edit.undo`, `motion.path`, `motion.undo_path`, `motion.recipe` |
| `video` | `init`, `status`, `snapshot`, `render`, `place-track` | `video.init`, `workshop.status`, `video.snapshot`, `video.render`, `video.place_track` |
| `ui` | `state`, `feedback`, `elements`, `components`, `controls`, `click`, `set`, `key`, `drag`, `look`, `screenshot`, `focus`, `eval` | `ui.*` live window controls |
| `session` | `status`, `undo`, `clear` | Session state inspection and domain-specific undo |
| `export` | `presets`, `render` | `export.layered_motion`, `video.render`, `storyboard.render` |
| `mcp` | `stdio` (default) | Built-in MCP stdio server (`2024-11-05`, `2025-03-26`, `2026-07-28`) |
| `settings` | `root` | `project.root` |

## 4. Rendering & Preview Assessment

- **Headless Previews**:
  - Storyboard pages render via DeckCraft to `storyboard/.preview/page-*.png` (`storyboard.render`).
  - Cutout rigs and motion clips render via EffectCraft (`export.layered_motion`) and HyperFrames (`video.snapshot`, `video.render`).
  - HyperFrames studio projects work as a website (`hyperframes/index.html`) inside the project folder without needing `hyperframes.json` or a separate project name.
- **Live Window Previews**:
  - `magic ui look` and `magic ui screenshot` capture the actual WebKit viewport PNG and DOM element layout from the open `magicstudio-shell` window.
  - `magic project save` flushes any pending UI storyboard text drafts (`saveStoryboard()`) before persisting `<project>.magic`.
