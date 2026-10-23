---
name: craft-app-lifecycle
description: Use when starting or stopping a live Craft desktop app (Magic Studio, PhotoCraft, EffectCraft, VectorCraft, FilmCraft) from an agent session, especially after visual review or screenshots, to close the exact instance cleanly instead of leaving it running or force-killing it.
---

# Craft app lifecycle

A live Craft window is a process the agent started. It does not close when the
agent stops using it, when a screenshot finishes, or when the MCP server's stdin
closes. **You started it, you stop it** before handing off.

Prefer a headless CLI/MCP session for file work. Launch a window only when the
task needs visual review, and then keep the launch PID, port and log path for the
whole session.

## Before launching

- Use a unique `--control <port>` per agent and a per-task config/data directory
  where the app supports one (`PHOTOCRAFT_CONFIG_DIR`, `EFFECTCRAFT_CONFIG_DIR`,
  `VECTORCRAFT_CONFIG_DIR`, `FILMCRAFT_DATA_DIR`).
- Record `pid=$!`, the port and the log file. Never share a port or stop another
  agent's instance.
- If the agent needs MCP tools against the live window, start a **bridge** MCP
  server for that same port. The default `.mcp.json` servers are headless and
  cannot close a window they never launched.

## Stop it with its control channel

Save first. Then send `app.quit` through the app's control surface and wait for
the process to exit and the port to become free.

| App | Close the live window | Notes |
|---|---|---|
| Magic Studio | `mc --bridge PORT exec app.quit` or MCP `app_quit` with `mcp --bridge PORT` | Closes immediately; save the project first. Headless MCP returns an error for `app_quit`. |
| PhotoCraft | `photocraft-cli mcp --bridge 127.0.0.1:PORT` then `control_call {"method":"app.quit","params":{}}` | The control channel needs its token: launch with `--control-token-file <file>` and pass `--control-token-file <file>` to the MCP server, or read the generated token from stderr. `app.quit` sets `allow_close`, so it does **not** prompt; save/close documents first. |
| EffectCraft | `effectcraft-cli --bridge PORT exec app.quit`, or the control line `{"id":1,"method":"app.quit","params":{"force":true}}` | A dirty project cancels a plain `app.quit` and opens the save prompt. Save, answer the prompt, or use `force: true` to discard. |
| VectorCraft | `vectorcraft-cli mcp --connect 127.0.0.1:PORT` then `invoke_menu {"command":"app.quit"}`, or the control line `{"id":1,"method":"app.quit","params":{}}` | A modified document returns `{"pending":"saveChanges"}` and the window stays. Save, or answer the dialog (`ui.dialog.confirm` with `discard: true`) before quitting. |
| FilmCraft | `filmcraft-cli --bridge PORT exec app.quit` or MCP `ui_control {"method":"app.quit"}` | The control method quits without the save prompt; unsaved changes stay in the recovery journal. The **command** `app.quit` asks first. |

For a raw control line, one JSON request per line to `127.0.0.1:PORT`:

```json
{"id": 1, "method": "app.quit", "params": {}}
```

If the MCP server is already bridged, call the app's own tool instead of opening
a socket. Do not add a second headless server for a live app: headless `app.quit`
has no window to close.

## Verify, then fall back honestly

1. The quit reply is not proof of exit. Wait up to 5 s for `kill -0 "$pid"` to
   fail and for the window/port to disappear.
2. If the app returned a save dialog, answer it or force the quit. Do not report
   success while the window is still up.
3. If the control channel is unreachable, `kill "$pid"` (SIGTERM) using the PID
   you recorded. Wait, then `kill -9 "$pid"` only as a last resort. Report a
   forced kill and why.
4. Never use `pkill -f`: the pattern can match the shell issuing it. Do not use
   `hyprctl dispatch closewindow` or `killactive` on Hyprland 0.56; those
   dispatchers no longer exist and fail silently when stderr is hidden. They were
   the cause of the "apps refused to close" report: no close request ever reached
   the apps, and all four quit on the first control `app.quit`.

## MCP server processes

`<app>-cli mcp` exits when stdin closes; a bridge `app_quit` closes the window and
the server after replying. Do not leave background `mcp` processes behind, and do
not treat a still-running MCP server as a still-open document: list the app's
processes and control ports before relaunching.
