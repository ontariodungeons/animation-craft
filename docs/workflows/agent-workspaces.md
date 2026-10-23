# Agent desktop workspaces

Jesse owns workspaces **1 and 2**, including Herdr. Each agent uses its own
workspace for app windows, so Jesse can review them on his own schedule.

| Workspace | Owner |
| --- | --- |
| 1–2 | Jesse / Herdr |
| 3 | Scooter |
| 4 | Waldorf |
| 5 | Beaker |
| 6 | Statler |
| 7 | Bunsen |
| 8 | Sam |
| 9 | Fozzie |

Sam maintains assignments. These are future launch destinations; existing windows
stay where Jesse placed them until their owner coordinates a safe transition.
A workspace assignment does not authorize an unnecessary app launch.

When a task needs a new app window, use the direct executable (not a launcher
that forks or reuses another process), with its own control endpoint:

```sh
python scripts/agent_window.py --workspace 3 -- \
  ./target/debug/magicstudio-shell /absolute/path/Study.magic --control 19943
```

The helper preserves the calling directory, quotes arguments, and supplies
Hyprland's `workspace = "3 silent"` `no_initial_focus = true`, `render_unfocused = true`, and
`focus_on_activate = false` launch rules. Hidden windows keep rendering without
accepting activation requests; the existing background FPS cap remains unchanged.
It rejects workspaces 1/2 and never retries through a foreground launch. Use
`--dry-run` before `--` to inspect the dispatch without launching anything.
A `submitted` reply confirms compositor acceptance only, not app readiness.
Hyprland associates launch rules with the process; applications that fork or
reuse an existing instance need their own exact-instance launch support.

Record the PID, workspace, executable/build, project and control endpoint. Pin
CLI/MCP to that endpoint, verify `ui.feedback` identity, and use it for subsequent
commands; do not depend on the active window, cursor, hover or `--bridge auto`
when multiple Studios exist. Read-only endpoint feedback works across workspaces.
Do not call `ui.focus`, switch workspaces, or use `project.open` as a focus probe.
If control fails, report the exact endpoint/error instead of asking Jesse to hover
or silently launching another instance. Close only the owned instance through
saved/guarded `app.quit` when its task is finished.

Generic verification (no GUI or real compositor called):

```sh
python -m unittest discover -s tests -p test_agent_window.py
```
