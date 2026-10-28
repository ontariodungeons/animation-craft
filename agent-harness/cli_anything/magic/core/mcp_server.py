"""MCP stdio server for Magic Studio (`magic mcp`).

Supports protocol versions `2024-11-05`, `2025-03-26`, and `2026-07-28`.
Uses the same unified `Session` routing as the CLI so tool calls automatically
target the open Studio window when the active project is open in Magic Studio,
or execute headlessly with auto-save when no window is open.
"""

from __future__ import annotations

import base64
import json
import sys
import tempfile
from pathlib import Path
from typing import Any

from cli_anything.magic import __version__
from cli_anything.magic.core.session import Session
from cli_anything.magic.utils import magic_backend as backend


MCP_INSTRUCTIONS = (
    "Magic Studio turns still character artwork into animated characters and plans "
    "song videos (storyboard, transcript, HyperFrames studio website). Start with "
    "project_open, project_new, or app_open. All subsequent tool calls automatically "
    "route to the live Studio window if the project is open in Magic Studio, or run "
    "headlessly and persist to the .magic file."
)


def _infer_schema(example: Any) -> dict[str, Any]:
    """Infer a JSON Schema fragment from a command parameter example value."""
    if isinstance(example, bool):
        return {"type": "boolean", "description": f"default {str(example).lower()}"}
    if isinstance(example, (int, float)):
        return {"type": "number", "description": f"default {example}"}
    if isinstance(example, str):
        options = [o.strip() for o in example.split("|")]
        if len(options) > 1 and all(
            o and all(c.isalnum() or c in "_-." for c in o) for o in options
        ):
            return {"type": "string", "enum": options, "description": example}
        return {"type": "string", "description": example}
    if isinstance(example, list):
        item_schema = _infer_schema(example[0]) if example else {}
        return {"type": "array", "items": item_schema}
    if isinstance(example, dict):
        props = {k: _infer_schema(v) for k, v in example.items()}
        return {"type": "object", "properties": props, "additionalProperties": True}
    return {}


BRIDGE_UI_TOOLS: list[dict[str, Any]] = [
    {
        "name": "app_open",
        "cmd_id": "app.open",
        "description": (
            "Open a Magic Studio project in its own window, or a new blank window when "
            "project is omitted. Attaches this session to the live window."
        ),
        "properties": {
            "project": {"type": "string", "description": "Optional existing .magic file"},
        },
    },
    {
        "name": "ui_state",
        "cmd_id": "ui.state",
        "description": "Read live Studio window state (project, view, animation subject, track, playhead).",
        "properties": {},
    },
    {
        "name": "ui_elements",
        "cmd_id": "ui.elements",
        "description": "List visible clickable elements in the live Studio window with IDs, labels, and bounding boxes.",
        "properties": {
            "region": {"type": "string", "description": "Optional CSS selector region filter"},
        },
    },
    {
        "name": "ui_click",
        "cmd_id": "ui.click",
        "description": "Click a visible UI element in the live Studio window by ID or aria label.",
        "properties": {
            "id": {"type": "string", "description": "Element ID or aria-label"},
            "target": {"type": "string", "description": "Alternative target ID"},
            "instance": {"type": "number", "description": "Zero-based index when IDs repeat"},
            "files": {"type": "array", "items": {"type": "string"}, "description": "Optional file paths for file inputs"},
        },
    },
    {
        "name": "ui_set",
        "cmd_id": "ui.set",
        "description": "Set an input, textarea, or select control value in the live Studio window.",
        "properties": {
            "id": {"type": "string", "description": "Element ID"},
            "target": {"type": "string", "description": "Alternative target ID"},
            "value": {"type": "string", "description": "Value to set"},
            "instance": {"type": "number", "description": "Zero-based index when IDs repeat"},
        },
    },
    {
        "name": "ui_key",
        "cmd_id": "ui.key",
        "description": "Send a key press to the live Studio window.",
        "properties": {
            "key": {"type": "string", "description": "Key name (e.g. Enter, Escape, Space)"},
            "id": {"type": "string", "description": "Optional target element ID"},
            "instance": {"type": "number", "description": "Zero-based index when IDs repeat"},
        },
    },
    {
        "name": "ui_drag",
        "cmd_id": "ui.drag",
        "description": "Drag a control or canvas point in the live Studio window.",
        "properties": {
            "id": {"type": "string", "description": "Optional element ID"},
            "from": {"type": "array", "items": {"type": "number"}},
            "to": {"type": "array", "items": {"type": "number"}},
        },
    },
    {
        "name": "ui_look",
        "cmd_id": "ui.look",
        "description": (
            "See the live Magic Studio window in one call: optionally click an element "
            "first, then return a PNG image of the window plus app state and clickable IDs."
        ),
        "properties": {
            "region": {"type": "string"},
            "click": {"type": "string"},
            "max_side": {"type": "number"},
            "pad": {"type": "number"},
        },
    },
    {
        "name": "ui_screenshot",
        "cmd_id": "ui.screenshot",
        "description": "Save a PNG screenshot of the live Studio window.",
        "properties": {
            "output": {"type": "string", "description": "Optional output PNG path"},
        },
    },
    {
        "name": "ui_focus",
        "cmd_id": "ui.focus",
        "description": "Bring the bound Magic Studio window to the front.",
        "properties": {},
    },
    {
        "name": "ui_eval",
        "cmd_id": "ui.eval",
        "description": "Evaluate JavaScript in the project webview.",
        "properties": {
            "js": {"type": "string", "description": "JavaScript code to evaluate"},
        },
    },
    {
        "name": "ui_unconfigured",
        "cmd_id": "ui.unconfigured",
        "description": "List unconfigured UI controls in the live Studio window.",
        "properties": {},
    },
    {
        "name": "app_quit",
        "cmd_id": "app.quit",
        "description": "Quit the live Magic Studio window bound to this session.",
        "properties": {},
    },
]


def build_mcp_tools() -> list[dict[str, Any]]:
    """Build the MCP tool list from the live engine command registry plus bridge UI controls."""
    specs = backend.list_engine_commands()
    tools: list[dict[str, Any]] = []
    seen_names: set[str] = set()
    for spec in specs:
        cmd_id = spec["id"]
        tool_name = cmd_id.replace(".", "_")
        seen_names.add(tool_name)
        props: dict[str, Any] = {}
        try:
            described = json.loads(spec.get("params") or "{}")
            if isinstance(described, dict):
                props = {k: _infer_schema(v) for k, v in described.items()}
        except Exception:
            pass
        props["_client"] = {
            "type": "string",
            "description": "Optional supplied agent/client label",
        }
        props["_request_id"] = {
            "type": "string",
            "description": "Optional request/correlation ID",
        }
        tools.append(
            {
                "name": tool_name,
                "description": f"{spec['label']} ({spec.get('params', '{}')})",
                "inputSchema": {
                    "type": "object",
                    "properties": props,
                    "additionalProperties": True,
                },
            }
        )

    for btool in BRIDGE_UI_TOOLS:
        if btool["name"] in seen_names:
            continue
        seen_names.add(btool["name"])
        tools.append(
            {
                "name": btool["name"],
                "description": btool["description"],
                "inputSchema": {
                    "type": "object",
                    "properties": dict(btool["properties"]),
                    "additionalProperties": True,
                },
            }
        )
    return tools


def _handle_ui_look(session: Session, bridge: str | None, args: dict[str, Any]) -> dict[str, Any]:
    """Capture a window screenshot via `ui.look` and return MCP image + text content."""
    with tempfile.NamedTemporaryFile(prefix="magic-look-", suffix=".png", delete=False) as tmp:
        out_path = Path(tmp.name)
    req_args = dict(args)
    req_args["output"] = str(out_path)
    try:
        state = session.execute("ui.look", req_args, bridge=bridge)
        png_bytes = out_path.read_bytes()
        b64 = base64.b64encode(png_bytes).decode("ascii")
        return {
            "content": [
                {"type": "image", "data": b64, "mimeType": "image/png"},
                {"type": "text", "text": json.dumps(state, indent=2)},
            ]
        }
    except Exception as exc:
        return {
            "content": [{"type": "text", "text": str(exc)}],
            "isError": True,
        }


def run_mcp_stdio(session: Session, bridge: str | None = None) -> None:
    """Run the JSON-RPC 2.0 MCP server over stdin/stdout."""
    specs = backend.list_engine_commands()
    id_lookup = {s["id"].replace(".", "_"): s["id"] for s in specs}
    for s in specs:
        id_lookup[s["id"]] = s["id"]
    for btool in BRIDGE_UI_TOOLS:
        id_lookup[btool["name"]] = btool["cmd_id"]
        id_lookup[btool["cmd_id"]] = btool["cmd_id"]

    stop_after_reply = False
    for line in sys.stdin:
        trimmed = line.strip()
        if not trimmed:
            continue
        try:
            req = json.loads(trimmed)
        except Exception:
            continue
        if "id" not in req:
            # Notification (e.g. notifications/initialized) — never answered
            continue
        req_id = req["id"]
        method = req.get("method", "")

        if method == "initialize":
            client_ver = (req.get("params") or {}).get("protocolVersion")
            negotiated = (
                client_ver
                if client_ver in {"2026-07-28", "2025-03-26", "2024-11-05"}
                else "2024-11-05"
            )
            resp = {
                "jsonrpc": "2.0",
                "id": req_id,
                "result": {
                    "protocolVersion": negotiated,
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "magic", "version": __version__},
                    "instructions": MCP_INSTRUCTIONS,
                },
            }
        elif method == "tools/list":
            resp = {
                "jsonrpc": "2.0",
                "id": req_id,
                "result": {"tools": build_mcp_tools()},
            }
        elif method == "tools/call":
            params = req.get("params") or {}
            raw_name = params.get("name", "")
            args = dict(params.get("arguments") or {})
            args["_origin"] = "MCP"

            if raw_name in {"ui_look", "ui.look"}:
                resp = {
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": _handle_ui_look(session, bridge, args),
                }
            else:
                try:
                    if raw_name in {"app_open", "app.open"}:
                        live = backend.open_studio_window(args.get("project"))
                        session.bind_studio(live)
                        result = {
                            "ok": True,
                            "active_project": live.get("active_project"),
                            "endpoint": live.get("endpoint"),
                            "instance": live.get("instance"),
                            "identity": live.get("identity"),
                            "ui_ready": live.get("ui_ready"),
                        }
                    elif raw_name in {"app_quit", "app.quit"}:
                        result = session.execute("app.quit", args, bridge=bridge)
                        if result.get("ok") is not False:
                            stop_after_reply = True
                    else:
                        cmd_id = id_lookup.get(raw_name, raw_name.replace("_", "."))
                        result = session.execute(cmd_id, args, bridge=bridge)
                        if cmd_id in {"project.open", "project.new", "project.intake"} and result.get("project"):
                            session.project_path = str(Path(result["project"]).resolve())
                            session.save_session()
                    is_err = result.get("ok") is False
                    res_obj: dict[str, Any] = {
                        "content": [{"type": "text", "text": json.dumps(result, indent=2)}]
                    }
                    if is_err:
                        res_obj["isError"] = True
                    resp = {"jsonrpc": "2.0", "id": req_id, "result": res_obj}
                except Exception as exc:
                    resp = {
                        "jsonrpc": "2.0",
                        "id": req_id,
                        "error": {"code": -32000, "message": str(exc)},
                    }
        elif method == "ping":
            resp = {"jsonrpc": "2.0", "id": req_id, "result": {}}
        elif method == "resources/list":
            resp = {"jsonrpc": "2.0", "id": req_id, "result": {"resources": []}}
        elif method == "resources/templates/list":
            resp = {"jsonrpc": "2.0", "id": req_id, "result": {"resourceTemplates": []}}
        elif method == "prompts/list":
            resp = {"jsonrpc": "2.0", "id": req_id, "result": {"prompts": []}}
        else:
            resp = {
                "jsonrpc": "2.0",
                "id": req_id,
                "error": {"code": -32601, "message": f"Method not found: {method}"},
            }

        sys.stdout.write(json.dumps(resp) + "\n")
        sys.stdout.flush()
        if stop_after_reply:
            break
