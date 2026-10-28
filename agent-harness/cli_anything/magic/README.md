# Magic Studio CLI (`magic` / `magic`)

Unified CLI-Anything harness and MCP server for Magic Studio (`magicstudio-shell`), driving headless `.magic` projects and live GUI windows with one command hierarchy.

## Prerequisites

- Magic Studio native app (`magicstudio-shell`):
  `CARGO_BUILD_JOBS=1 cargo build -p magicstudio-shell`
- Python 3.10+ with `click` and `prompt-toolkit`

## Installation

```bash
cd /run/media/jesse/Speedy/Work/omhas-animation/agent-harness
pip install -e .
```

This installs both `magic` and `magic` into your `PATH`.

## Usage

### Interactive REPL (Default)

```bash
magic
```

### Launching or Attaching to Magic Studio GUI

```bash
# Launch a new blank Studio window and bind the session
magic studio new

# Open or focus an existing project in Magic Studio and bind the session
magic studio "Bumble Bee.magic"

# List open Studio windows
magic studio list
```

### Unified Headless & Live Project Operations

Once a project or Studio window is selected (`magic studio <project>` or `magic project open <project>`), all subsequent commands automatically target the live Studio window if the project is open in Magic Studio, or run headlessly and auto-save to `<project>.magic`:

```bash
# Create or open a project
magic project new -o /tmp/demo/Song.magic --name "Song"
magic project open /tmp/demo/Song.magic
magic project info
magic project save
magic project reveal
magic project close

# Storyboard operations
magic storyboard read
magic storyboard edit scene 1 "Hero enters stage left"
magic storyboard open
magic storyboard render

# Character & Motion operations
magic character list
magic motion tracks character=hopper
magic motion clips character=hopper

# Video Studio (HyperFrames website / Remotion)
magic video status
magic video snapshot sec=1.0 out=/tmp/frame.png

# Live Window UI Controls (CLI-Anything)
magic ui state
magic ui feedback
magic ui elements
magic ui click id=tab-studio
magic ui look output=/tmp/studio-look.png
magic app quit
```

### MCP Server (`magic mcp`)

Register `magic` with args `["mcp"]` in your MCP client configuration:

```json
{
  "mcpServers": {
    "magic": {
      "command": "magic",
      "args": ["mcp"]
    }
  }
}
```
