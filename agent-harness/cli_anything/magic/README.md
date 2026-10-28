# Magic Studio CLI

`magic` opens the GUI. `magic-cli --help` describes terminal commands. `magic-cli mcp` starts the MCP stdio server. No REPL.

Read [the Magic skill](skills/SKILL.md) for command discovery, project/window targeting and MCP configuration, and [the architecture](../../../MAGIC.md) for ownership boundaries.

For a source checkout, build and register Magic itself:

```bash
cargo build -p magicstudio-shell
uv pip install --python .venv/bin/python -e agent-harness
python3 scripts/install_magic.py
```

Run these from the repository root with its virtual environment prepared. The installer uses this checkout's Python environment, copies the built native shell, and installs `magic`, `magic-cli` and a desktop entry under `~/.local`. It does not install ArtCraft or register MCP servers automatically. Installed users should inspect command paths and errors before changing their installation.
