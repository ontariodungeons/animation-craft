"""Session state and unified execution routing for Magic Studio CLI.

Implements `_locked_save_json` from CLI-Anything `guides/session-locking.md` and
routes operations transparently between:
1. Live Studio windows (`unix:<socket>` or `--bridge <port>`) when the active project
   is open in Magic Studio or a Studio window is bound.
2. Headless engine execution (`magicstudio-shell --headless`) when working headlessly
   on a `.magic` project file.
Never invents a cross-domain undo ledger; undo delegates to the specific domain's
engine command (`motion.edit.undo`, `motion.undo_path`, `part.undo_image`,
`storyboard.update`, `skills.undo`).
"""

from __future__ import annotations

import copy
import json
import os
from pathlib import Path
from typing import Any

from cli_anything.magic.utils import magic_backend as backend


def _locked_save_json(path: str | Path, data: Any, **dump_kwargs: Any) -> None:
    """Atomically write JSON with exclusive file locking."""
    path_str = str(path)
    dump_kwargs.setdefault("indent", 2)
    try:
        f = open(path_str, "r+", encoding="utf-8")
    except FileNotFoundError:
        os.makedirs(os.path.dirname(os.path.abspath(path_str)), exist_ok=True)
        f = open(path_str, "w", encoding="utf-8")
    with f:
        _locked = False
        try:
            import fcntl

            fcntl.flock(f.fileno(), fcntl.LOCK_EX)
            _locked = True
        except (ImportError, OSError):
            pass
        try:
            f.seek(0)
            f.truncate()
            json.dump(data, f, **dump_kwargs)
            f.flush()
        finally:
            if _locked:
                import fcntl

                fcntl.flock(f.fileno(), fcntl.LOCK_UN)


def default_session_file() -> Path:
    """Return the runtime-scoped session file path for this caller/pane."""
    runtime = os.environ.get("XDG_RUNTIME_DIR")
    base = Path(runtime) / "magicstudio" if runtime else Path.home() / ".magic"
    pane = os.environ.get("HERDR_PANE_ID")
    if pane and all(c.isalnum() or c in "-_:" for c in pane):
        return base / "agent-studios" / pane / "session.json"
    return base / "session.json"


def active_studio_pin_file() -> Path | None:
    """Return the legacy `active-studio` pin path for compatibility."""
    runtime = os.environ.get("XDG_RUNTIME_DIR")
    if not runtime:
        return None
    base = Path(runtime) / "magicstudio"
    pane = os.environ.get("HERDR_PANE_ID")
    if pane and all(c.isalnum() or c in "-_:" for c in pane):
        return base / "agent-studios" / pane / "active-studio"
    return base / "active-studio"


READ_ONLY_COMMANDS = {
    "project.open",
    "project.attach",
    "project.identity",
    "project.catalog",
    "project.list_assets",
    "project.search",
    "workshop.status",
    "workshop.files",
    "workshop.storyboard",
    "storyboard.read",
    "storyboard.design",
    "storyboard.backgrounds",
    "storyboard.toc",
    "storyboard.template",
    "storyboard.doctor",
    "character.assets",
    "assets.assess",
    "assets.fixlist",
    "motion.tracks",
    "motion.clips",
    "motion.sample",
    "motion.theatre.read",
    "validate.motion",
    "animation.sources",
    "animation.status",
    "animation.list",
    "blender.actions",
    "blender.status",
    "blender.list",
    "synfig.discover",
    "opentoonz.discover",
    "mlt.discover",
    "song.lyrics",
    "words.align",
    "words.read",
    "words.studio_track",
    "words.effects",
    "words.track",
    "skills.list",
    "skills.info",
    "crew.list",
    "agent.session",
    "agent.models",
    "command.list",
    "studio.cues",
    "vision.analyze",
    "ui.state",
    "ui.feedback",
    "ui.elements",
    "ui.components",
    "ui.controls",
    "ui.unconfigured",
}


def is_read_only_command(cmd_id: str, params: dict[str, Any] | None = None) -> bool:
    """Return True if `cmd_id` with `params` is a pure read-only query."""
    p = params or {}
    if cmd_id in READ_ONLY_COMMANDS:
        return True
    if cmd_id == "project.root":
        return p.get("root") is None
    if cmd_id == "skills.limits":
        return p.get("max_words") is None and p.get("max_steps") is None
    if cmd_id == "skills.diagram":
        return p.get("render") is not True
    if cmd_id == "agent.log":
        return p.get("refresh") is not True
    if cmd_id == "app.analytics":
        return p.get("scan") is not True
    if cmd_id == "app.doctor":
        return p.get("clean") is not True
    return False


class Session:
    """Stateful CLI-Anything session for Magic Studio."""

    def __init__(self, session_file: str | Path | None = None) -> None:
        self.session_file = Path(session_file) if session_file else default_session_file()
        self.project_path: str | None = None
        self.studio_endpoint: str | None = None
        self.studio_instance: str | None = None
        self.project_data: dict[str, Any] | None = None
        self._modified: bool = False
        self.load_session()

    def load_session(self) -> None:
        """Load persisted session binding from disk if present."""
        if self.session_file.is_file():
            try:
                raw = json.loads(self.session_file.read_text(encoding="utf-8"))
                self.project_path = raw.get("project_path")
                self.studio_endpoint = raw.get("studio_endpoint")
                self.studio_instance = raw.get("studio_instance")
            except Exception:
                pass
        if self.project_path and Path(self.project_path).is_file():
            try:
                self.project_data = json.loads(
                    Path(self.project_path).read_text(encoding="utf-8")
                )
            except Exception:
                self.project_data = None

    def save_session(self) -> None:
        """Persist current session binding (`session.json` and `active-studio` pin).

        Never writes `self.project_data` back to the `.magic` file; project files are
        owned exclusively by the Rust engine (`project.save` and headless dispatch).
        """
        state = {
            "project_path": self.project_path,
            "studio_endpoint": self.studio_endpoint,
            "studio_instance": self.studio_instance,
        }
        _locked_save_json(self.session_file, state)
        pin_file = active_studio_pin_file()
        if pin_file:
            try:
                pin_file.parent.mkdir(parents=True, exist_ok=True)
                if self.studio_endpoint:
                    pin_file.write_text(f"{self.studio_endpoint}\n", encoding="utf-8")
                elif self.project_path:
                    pin_file.write_text(f"project:{self.project_path}\n", encoding="utf-8")
            except OSError:
                pass

    def has_project(self) -> bool:
        """Return True if a project path or in-memory project document is bound."""
        return bool(self.project_path or self.project_data is not None)

    def set_project(
        self,
        project_data: dict[str, Any] | None,
        project_path: str | Path | None,
        studio_endpoint: str | None = None,
        studio_instance: str | None = None,
    ) -> None:
        """Bind the session to `project_path` and optional live Studio endpoint."""
        self.project_data = project_data
        self.project_path = str(Path(project_path).resolve()) if project_path else None
        self.studio_endpoint = studio_endpoint
        self.studio_instance = studio_instance
        self._modified = False
        self.save_session()

    def bind_studio(self, live: dict[str, Any]) -> None:
        """Bind the session to a live Studio window receipt."""
        self.studio_endpoint = live.get("endpoint")
        self.studio_instance = live.get("instance")
        active = live.get("active_project")
        if active:
            self.project_path = str(Path(active).resolve())
        self.save_session()

    def clear(self) -> None:
        """Clear the bound session and remove any pin file."""
        self.project_path = None
        self.studio_endpoint = None
        self.studio_instance = None
        self.project_data = None
        self._modified = False
        if self.session_file.is_file():
            try:
                self.session_file.unlink()
            except OSError:
                pass
        pin_file = active_studio_pin_file()
        if pin_file and pin_file.is_file():
            try:
                pin_file.unlink()
            except OSError:
                pass

    def resolve_live_target(self, bridge: str | None = None) -> str | None:
        """Return the live Studio bridge endpoint for this session if one is active, else None."""
        if bridge:
            return bridge
        if self.project_path:
            p = Path(self.project_path)
            if p.exists():
                live = backend.studio_for_project(str(p.resolve()))
                if live:
                    return str(live["endpoint"])
        if self.studio_endpoint:
            sock_path = self.studio_endpoint.removeprefix("unix:")
            if self.studio_endpoint.startswith("unix:") and not Path(sock_path).exists():
                raise RuntimeError(
                    "This agent's assigned Studio is unavailable; no other window was selected. "
                    "Run `magic studio list` or `magic studio <project>` to reconnect."
                )
            return self.studio_endpoint
        return None

    def undo(self, domain: str | None = None, **kwargs: Any) -> dict[str, Any]:
        """Undo within a specific engine domain (no cross-domain ledger)."""
        if not domain:
            raise ValueError(
                "Specify an engine undo domain (--domain motion|path|part|skills|storyboard) "
                "and its receipt token."
            )
        cmd_map = {
            "motion": "motion.edit.undo",
            "path": "motion.undo_path",
            "part": "part.undo_image",
            "skills": "skills.undo",
            "storyboard": "storyboard.update",
        }
        cmd_id = cmd_map.get(domain, domain)
        return self.execute(cmd_id, dict(kwargs))

    def execute(
        self,
        cmd_id: str,
        params: dict[str, Any] | None = None,
        bridge: str | None = None,
        dry_run: bool = False,
    ) -> dict[str, Any]:
        """Execute an operation in the established context (live Studio or headless engine).

        Context resolution order:
        1. Explicit `bridge` argument (`--bridge <port|unix:path|auto>`).
        2. UI-only commands (`ui.*`, `app.quit`) -> bound Studio endpoint or `auto`.
        3. Explicit or bound `project` -> if open in a live Studio window, route to that window's
           Unix socket so changes reflect immediately in the visible UI.
        4. Bound live Studio endpoint (`self.studio_endpoint`) -> route to that window; if the
           window closed, fail loudly without falling back to another window.
        5. Headless engine execution (`magicstudio-shell --headless`).
        """
        req = dict(params or {})
        is_dry = dry_run or bool(req.pop("_dry_run", False))

        # 1. Explicit bridge override
        if bridge:
            if is_dry and not is_read_only_command(cmd_id, req):
                return {
                    "ok": True,
                    "dry_run": True,
                    "route": "bridge",
                    "endpoint": bridge,
                    "command": cmd_id,
                    "params": req,
                }
            return backend.call_bridge(bridge, cmd_id, req)

        # 2. UI-only commands always target a live Studio window
        if cmd_id.startswith("ui.") or cmd_id == "app.quit":
            target = self.studio_endpoint or "auto"
            if is_dry and not is_read_only_command(cmd_id, req):
                return {
                    "ok": True,
                    "dry_run": True,
                    "route": "bridge",
                    "endpoint": target,
                    "command": cmd_id,
                    "params": req,
                }
            return backend.call_bridge(
                target,
                cmd_id,
                req,
                expected_project=self.project_path,
                expected_instance=self.studio_instance,
            )

        # 3. Resolve project path from params or session
        global_cmds = {
            "project.new",
            "project.intake",
            "project.catalog",
            "project.root",
            "project.migrate",
            "project.identity",
            "app.doctor",
            "command.list",
        }
        target_proj = req.get("project") or (
            req.get("path") if cmd_id == "project.open" else None
        )
        if not target_proj and cmd_id not in global_cmds and self.project_path:
            target_proj = self.project_path
            req["project"] = self.project_path

        if target_proj and cmd_id not in {"project.new", "project.intake", "project.migrate"}:
            p = Path(str(target_proj))
            if p.exists():
                canonical = str(p.resolve())
                live = backend.studio_for_project(canonical)
                if live:
                    if is_dry and not is_read_only_command(cmd_id, req):
                        return {
                            "ok": True,
                            "dry_run": True,
                            "route": "bridge",
                            "endpoint": live["endpoint"],
                            "command": cmd_id,
                            "params": req,
                            "project": canonical,
                        }
                    if cmd_id == "project.open":
                        self.bind_studio(live)
                    return backend.call_bridge(
                        live["endpoint"],
                        cmd_id,
                        req,
                        expected_project=canonical if cmd_id != "project.open" else None,
                        expected_instance=live.get("instance"),
                    )

        # 4. Bound Studio endpoint
        if self.studio_endpoint and cmd_id not in {"project.new", "project.intake"}:
            sock_path = self.studio_endpoint.removeprefix("unix:")
            if self.studio_endpoint.startswith("unix:") and not Path(sock_path).exists():
                raise RuntimeError(
                    "This agent's assigned Studio is unavailable; no other window was selected. "
                    "Run `magic studio list` or `magic studio <project>` to reconnect."
                )
            if is_dry and not is_read_only_command(cmd_id, req):
                return {
                    "ok": True,
                    "dry_run": True,
                    "route": "bridge",
                    "endpoint": self.studio_endpoint,
                    "command": cmd_id,
                    "params": req,
                    "project": self.project_path,
                }
            return backend.call_bridge(
                self.studio_endpoint,
                cmd_id,
                req,
                expected_project=self.project_path if cmd_id != "project.open" else None,
                expected_instance=self.studio_instance,
            )

        # 5. Headless engine execution
        if is_dry and not is_read_only_command(cmd_id, req):
            return backend.execute_headless(cmd_id, req, dry_run=True)

        res = backend.execute_headless(cmd_id, req, dry_run=is_dry)
        if self.project_path and Path(self.project_path).is_file():
            try:
                self.project_data = json.loads(
                    Path(self.project_path).read_text(encoding="utf-8")
                )
            except Exception:
                pass
        return res
