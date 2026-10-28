---
name: "magic"
description: "Drive Magic Studio projects, storyboards, character rigs, motion tracks, HyperFrames websites, and live GUI windows with one command hierarchy (`magic` CLI + `magic mcp`)."
---

# `magic` CLI & MCP Skill

Unified CLI-Anything harness (`cli_anything.magic`) for Magic Studio. Every operation uses the same logical command hierarchy whether working headlessly on a `.magic` project file or live inside an open Magic Studio window.

## Installation

```bash
cd /run/media/jesse/Speedy/Work/omhas-animation/agent-harness
pip install -e .
```

## Session Context & Routing Rule

Do **not** choose between a UI route (`--bridge <port>`) and a headless route per operation. Establish context once:

1. **Open or focus a live Studio window**:
   ```bash
   magic studio new
   magic studio "Bumble Bee.magic"
   ```
2. **Or open/create a project**:
   ```bash
   magic project open "/path/to/Song.magic"
   magic project new -o "/path/to/Song.magic" --name "Song"
   ```
3. **Run subsequent commands with the exact same syntax**:
   - If the active project is open in Magic Studio, `magic` automatically routes commands to that window's socket so the visible UI updates immediately.
   - If no window is open for that project, `magic` executes headlessly via `magicstudio-shell --headless` and auto-saves mutations to `<project>.magic` (pass `--dry-run` to suppress saving).
   - If a bound Studio window disconnects, `magic` refuses with an explicit error and **never** silently falls back to another window.

## Command Groups

### `magic studio`
| Command | Description |
|---------|-------------|
| `magic studio new [--animate]` | Launch a new blank Magic Studio window and bind session |
| `magic studio <project>` | Launch or focus `<project>` in Magic Studio and bind session |
| `magic studio list` | List all running Studio windows and current session binding |
| `magic studio status` | Query `ui.feedback` on the bound Studio window |
| `magic studio unpin` | Clear the active Studio/project session binding |
| `magic studio quit` | Guarded quit of the bound Studio window |

### `magic project`
| Command | Description |
|---------|-------------|
| `magic project new -o <path.magic> [--name <n>]` | Create a new `.magic` project and bind session |
| `magic project open <project>` | Open `.magic` project (attaches to live Studio if open, else headless) |
| `magic project list` | List recent projects (`project.catalog`) |
| `magic project save [-o <path>]` | Save active project (flushes pending UI storyboard drafts first) |
| `magic project close` | Close active project and clear session |
| `magic project reveal` | Open project directory in file manager |
| `magic project info [<path>]` | Inspect project assets, storyboard, and HyperFrames website status |
| `magic project profiles` | List project profiles (`hyperframes`, `remotion`, `character`) |
| `magic project add-asset path=<f> character=<c>` | Connect an asset to the project |
| `magic project list-assets` | List connected project assets |

### `magic storyboard`
| Command | Description |
|---------|-------------|
| `magic storyboard create scene=<N> action=<text>` | Create a storyboard scene |
| `magic storyboard read [scene=<N>\|page=<P>]` | Read storyboard scenes and pages |
| `magic storyboard update scene=<N> field=action value=<text>` | Update scene, split/combine pages, or restore history |
| `magic storyboard edit scene <N> "<text>" [field=action]` | Friendly scene text update |
| `magic storyboard delete scene=<N>` | Delete a storyboard scene |
| `magic storyboard open` | Open the project's `.deckcraft` storyboard deck |
| `magic storyboard build` | Build `.deckcraft` deck from scenes (`workshop.build_storyboard`) |
| `magic storyboard render` | Render storyboard pages to `storyboard/.preview/page-*.png` |
| `magic storyboard doctor` | Run section-boundary diagnostics |

### `magic character` & `magic motion`
| Command | Description |
|---------|-------------|
| `magic character list` | List character assets, sheets, angles, and rigs |
| `magic character import character=<id> path=<png>` | Import character artwork |
| `magic character configure character=<id> angle=<a>` | Set active angle or pose |
| `magic character cut-sheet asset=<id>` | Slice turnaround/pose sheet into parts |
| `magic character build-rig asset=<id>` | Build 2D articulated cutout rig |
| `magic motion tracks character=<id>` | List motion tracks and FPS |
| `magic motion clips [--convert] character=<id>` | List Theatre clips and conversion status |
| `magic motion sample character=<id> track=<t> time=<s>` | Sample part transforms at time `s` |
| `magic motion keyframe ...` | Author a Theatre motion keyframe |
| `magic motion undo` | Undo last Theatre keyframe edit (`motion.edit.undo`) |
| `magic motion path ...` | Apply an authored motion path |
| `magic motion undo-path` | Undo last motion path (`motion.undo_path`) |

### `magic video` (HyperFrames Website / Remotion)
A HyperFrames studio project (`hyperframes/index.html`) is a website inside the project directory and works directly in the app without needing a separate project name or `hyperframes.json`.

| Command | Description |
|---------|-------------|
| `magic video init` | Scaffold `hyperframes/index.html` |
| `magic video status` | Check readiness of the project's video studio |
| `magic video snapshot sec=1.0 out=/tmp/frame.png` | Render a single frame PNG |
| `magic video render out=/tmp/video.mp4` | Render full video MP4 |
| `magic video place-track ...` | Place a rendered transparent character track into the composition |

### `magic ui` (Live Window Inspection & Control)
| Command | Description |
|---------|-------------|
| `magic ui state` | Read live window state (project, view, active character/clip/playhead) |
| `magic ui feedback` | Read window readiness (`ui_ready`), dirty state, and DOM ack |
| `magic ui elements [region=<id>]` | List clickable DOM elements and bounding boxes |
| `magic ui components [region=<css>]` | Inspect computed DOM geometry and styles |
| `magic ui controls` | List UI controls and bound engine commands |
| `magic ui click <id>` | Click a visible UI control |
| `magic ui set id=<id> value=<v>` | Set an input or select value |
| `magic ui key key=<k> [id=<id>]` | Press a key in the window |
| `magic ui drag ...` | Drag a canvas or timeline control |
| `magic ui look [click=<id>] [output=<png>]` | Capture window/region PNG + state + elements |
| `magic ui screenshot [output=<png>]` | Save window PNG screenshot |
| `magic ui focus` | Focus the bound Studio window |

### `magic mcp`
Start the stdio MCP server (`2024-11-05`, `2025-03-26`, `2026-07-28`):
```bash
magic mcp
```
