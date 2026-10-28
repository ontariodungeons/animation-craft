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


def build_mcp_tools() -> list[dict[str, Any]]:
    """Build the MCP tool list from the live engine command registry."""
    specs = backend.list_engine_commands()
    tools: list[dict[str, Any]] = []
    for spec in specs:
        cmd_id = spec["id"]
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
                "name": cmd_id.replace(".", "_"),
                "description": f"{spec['label']} ({spec.get('params', '{}')})",
                "inputSchema": {
                    "type": "object",
                    "properties": props,
                    "additionalProperties": True,
                },
            }
        )

    tools.append(
        {
            "name": "app_open",
            "description": (
                "Open a Magic Studio project in its own window, or a new blank window when "
                "project is omitted. Attaches this session to the live window."
            ),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Optional existing .magic file",
                    }
                },
                "additionalProperties": False,
            },
        }
    )
    tools.append(
        {
            "name": "ui_look",
            "description": (
                "See the live Magic Studio window in one call: optionally click an element "
                "first, then return a PNG image of the window plus app state and clickable IDs."
            ),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "region": {"type": "string"},
                    "click": {"type": "string"},
                    "max_side": {"type": "number"},
                    "pad": {"type": "number"},
                },
                "additionalProperties": False,
            },
        }
    )
    tools.append(
        {
            "name": "app_quit",
            "description": "Quit the live Magic Studio window bound to this session.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": False,
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
