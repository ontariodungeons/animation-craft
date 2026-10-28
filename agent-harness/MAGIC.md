# Magic Studio command architecture

`magic` launches the native Tauri application. `magic-cli` is its CLI-Anything Click adapter; no arguments print help. `magic-cli mcp` runs the official Python MCP SDK stdio server. There is no REPL, install banner, `mc` compatibility executable or aggregate Craft MCP server.

The source of command IDs, labels, parameter documentation, journaling and long-running metadata is `crates/engine/src/commands.rs`. The CLI derives its natural `domain verb` hierarchy from `command.list`; it does not maintain a second dispatch table. MCP exposes `command_list` and `command_run` with engine-defined parameters, plus explicit window opening. The SDK owns the protocol and negotiation; there is no hand-written JSON-RPC loop or fabricated protocol version.

Both adapters use `core/session.py` for project/window binding. A selected live project uses its private Unix socket and expected project/instance guards. An unopened project uses the native shell's internal engine entry point, which attaches it, executes `Session::execute` and saves mutations. The transport is automatic. A pinned window that closes or changes projects is an error, not a fallback to a different writer. Explicit project selection is the only rebind path. Session binding writes are locked; Python does not create or save project documents.

UI project actions enter `apps/magicstudio-shell/src/project_dispatch.rs` directly. It retains the correlation token, pending-operation guard, project-state checks, native ownership, draft flushing, revision acknowledgements and initiating frontend refresh. The former subprocess round trip through the CLI and a TCP port is removed. The socket adapter uses that same guarded application dispatch. Window-specific `ui.*` operations are declared in the application registry and implemented by the shell; they fail honestly without a window.

Other installed apps are independent. Use their documented CLIs directly when their work is needed. Existing Magic domain operations that coordinate project assets remain Magic operations; do not duplicate app capabilities or add generic Craft wrappers.

The former `motion.recipe` engine command only spawned two scripts and required users to pick a port. Run the existing compiler and recipe renderer directly instead:

```bash
python3 scripts/compile_motion_specs.py /absolute/project/animation
python3 scripts/render_motion_recipe.py /absolute/project/animation/recipe/04-render-clips.jsonl --project /absolute/project/Example.magic --place /absolute/project/animation/recipe/05-place-in-video.jsonl
```

The renderer uses installed `magic-cli` and project selection, preserving its receipts, input hashes and output checks. Core render and placement operations remain in the shared registry.

See [the packaged Magic skill](cli_anything/magic/skills/SKILL.md) for executable usage and MCP configuration. The matching repo skill is `.agents/skills/magic/SKILL.md`. Historical architecture/handoff documents describe retired implementations and must not be used to restore them.

References: [CLI-Anything HARNESS](https://github.com/HKUDS/CLI-Anything/blob/main/cli-anything-plugin/HARNESS.md), [MCP architecture](https://modelcontextprotocol.io/docs/learn/architecture), [official Python SDK](https://github.com/modelcontextprotocol/python-sdk/tree/v1.x), [PhotoCraft's discovery/execution adapter](https://github.com/storytold/photocraft/blob/main/crates/automation/src/server.rs). PhotoCraft is a reference only, not a dependency or integration target for this refactor.
