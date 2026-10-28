"""MCP adapter: SDK-owned protocol, registry-owned command vocabulary."""
from __future__ import annotations

import asyncio
import base64
import json
import tempfile
from pathlib import Path
from typing import Any

from mcp.server.fastmcp import FastMCP
from mcp.types import CallToolResult, ImageContent, TextContent

from cli_anything.magic.core.session import Session
from cli_anything.magic.utils import magic_backend as backend


def run_mcp_stdio(session: Session, bridge: str | None = None, dry_run: bool = False) -> None:
    server = FastMCP(
        "Magic Studio",
        instructions="Discover Magic operations with command_list, then call command_run. "
        "Select a project using project.open with params.path, or studio_open for a visible window. "
        "Operations retain this session's project/window target. Other apps use their own CLIs.",
    )
    # The engine session is sequential. Do not let concurrent MCP calls race selection.
    lock = asyncio.Lock()

    def result(value: Any) -> CallToolResult:
        return CallToolResult(content=[TextContent(type="text", text=json.dumps(value, indent=2))])

    @server.tool()
    async def command_list(filter: str = "") -> CallToolResult:
        """Discover Magic registry IDs and engine-defined parameter documentation."""
        async with lock:
            value = await asyncio.to_thread(session.execute, "command.list", {"filter": filter}, bridge)
            return result(value)

    @server.tool()
    async def command_run(id: str, params: dict[str, Any] | None = None) -> CallToolResult:
        """Execute one Magic registry command. Use command_list for IDs and parameters."""
        async with lock:
            supplied = dict(params or {})
            supplied["_origin"] = "MCP"
            try:
                if id == "ui.look" and not dry_run and supplied.get("_dry_run") is not True:
                    with tempfile.TemporaryDirectory(prefix="magic-look-") as folder:
                        supplied.setdefault("output", str(Path(folder) / "window.png"))
                        value = await asyncio.to_thread(session.execute, id, supplied, bridge, dry_run)
                        image = base64.b64encode(Path(value["output"]).read_bytes()).decode("ascii")
                        return CallToolResult(content=[ImageContent(type="image", data=image, mimeType="image/png"),
                                                       TextContent(type="text", text=json.dumps(value))])
                return result(await asyncio.to_thread(session.execute, id, supplied, bridge, dry_run))
            except Exception as exc:
                return CallToolResult(isError=True, content=[TextContent(type="text", text=str(exc))])

    @server.tool()
    async def studio_open(project: str | None = None) -> CallToolResult:
        """Open/focus a Magic project window and bind this session; omit project for a blank window."""
        async with lock:
            if dry_run:
                return result({"dry_run": True, "project": project})
            from cli_anything.magic.core.project import resolve_project_path
            target = str(resolve_project_path(project)) if project else None
            live = await asyncio.to_thread(backend.open_studio_window, target)
            session.bind_studio(live)
            return result(live)

    server.run(transport="stdio")
