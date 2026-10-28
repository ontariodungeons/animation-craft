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


class Session:
    """Stateful CLI-Anything session for Magic Studio."""

    def __init__(self, session_file: str | Path | None = None) -> None:
        self.session_file = Path(session_file) if session_file else default_session_file()
        self.project_path: str | None = None
        self.studio_endpoint: str | None = None
        self.studio_instance: str | None = None
        self.project_data: dict[str, Any] | None = None
        self._modified: bool = False
        self._undo_stack: list[dict[str, Any]] = []
        self._redo_stack: list[dict[str, Any]] = []
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
        """Persist current session binding and any modified `.magic` project data."""
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
        if self._modified and self.project_path and self.project_data is not None:
            _locked_save_json(self.project_path, self.project_data)
            self._modified = False

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
        self._undo_stack.clear()
        self._redo_stack.clear()
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

    def snapshot(self) -> None:
        """Snapshot in-memory `.magic` JSON state for local session undo/redo."""
        if self.project_data is not None:
            self._undo_stack.append(copy.deepcopy(self.project_data))
            self._redo_stack.clear()
            self._modified = True

    def undo(self, domain: str | None = None, **kwargs: Any) -> dict[str, Any]:
        """Undo within a specific engine domain (no cross-domain ledger) or in-memory snapshot."""
        if domain:
            cmd_map = {
                "motion": "motion.edit.undo",
                "path": "motion.undo_path",
                "part": "part.undo_image",
                "skills": "skills.undo",
            }
            cmd_id = cmd_map.get(domain, domain)
            return self.execute(cmd_id, dict(kwargs))
        if not self._undo_stack:
            raise RuntimeError("Nothing to undo in local project state")
        if self.project_data is not None:
            self._redo_stack.append(copy.deepcopy(self.project_data))
        self.project_data = self._undo_stack.pop()
        self._modified = True
        return {"ok": True, "undone": True, "project": self.project_path}

    def redo(self) -> dict[str, Any]:
        """Redo an in-memory `.magic` project state change."""
        if not self._redo_stack:
            raise RuntimeError("Nothing to redo in local project state")
        if self.project_data is not None:
            self._undo_stack.append(copy.deepcopy(self.project_data))
        self.project_data = self._redo_stack.pop()
        self._modified = True
        return {"ok": True, "redone": True, "project": self.project_path}

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

        # 1. Explicit bridge override
        if bridge:
            return backend.call_bridge(bridge, cmd_id, req)

        # 2. UI-only commands always target a live Studio window
        if cmd_id.startswith("ui.") or cmd_id == "app.quit":
            target = self.studio_endpoint or "auto"
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
            if not Path(sock_path).exists():
                raise RuntimeError(
                    "This agent's assigned Studio is unavailable; no other window was selected. "
                    "Run `magic studio list` or `magic studio <project>` to reconnect."
                )
            return backend.call_bridge(
                self.studio_endpoint,
                cmd_id,
                req,
                expected_project=self.project_path if cmd_id != "project.open" else None,
                expected_instance=self.studio_instance,
            )

        # 5. Headless engine execution
        res = backend.execute_headless(cmd_id, req, dry_run=dry_run)
        if self.project_path and Path(self.project_path).is_file():
            try:
                self.project_data = json.loads(
                    Path(self.project_path).read_text(encoding="utf-8")
                )
            except Exception:
                pass
        return res
