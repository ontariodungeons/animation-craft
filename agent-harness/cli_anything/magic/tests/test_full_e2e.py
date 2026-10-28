"""End-to-end and subprocess tests for Magic Studio CLI-Anything harness."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any


def _resolve_cli(name: str) -> list[str]:
    """Resolve installed CLI command; falls back to python -m for dev.

    Set env CLI_ANYTHING_FORCE_INSTALLED=1 to require the installed command.
    """
    force = os.environ.get("CLI_ANYTHING_FORCE_INSTALLED", "").strip() == "1"
    path = shutil.which(name)
    if path:
        print(f"[_resolve_cli] Using installed command: {path}")
        return [path]
    if force:
        raise RuntimeError(f"{name} not found in PATH. Install with: pip install -e .")
    pkg = name.removeprefix("cli-anything-").replace("-", "_")
    module = f"cli_anything.{pkg}.{pkg}_cli"
    print(f"[_resolve_cli] Falling back to: {sys.executable} -m {module}")
    return [sys.executable, "-m", module]


class TestCLISubprocess:
    CLI_BASE = _resolve_cli("magic")

    def _run(
        self,
        args: list[str],
        check: bool = True,
        env_extra: dict[str, str] | None = None,
        input_text: str | None = None,
    ) -> subprocess.CompletedProcess[str]:
        env = dict(os.environ)
        if env_extra:
            env.update(env_extra)
        return subprocess.run(
            self.CLI_BASE + args,
            capture_output=True,
            text=True,
            check=check,
            env=env,
            input=input_text,
        )

    def _isolated_env(self, tmp_path: Path) -> dict[str, str]:
        for d in ("config", "state", "runtime"):
            (tmp_path / d).mkdir(exist_ok=True)
        return {
            "XDG_CONFIG_HOME": str(tmp_path / "config"),
            "XDG_STATE_HOME": str(tmp_path / "state"),
            "XDG_RUNTIME_DIR": str(tmp_path / "runtime"),
        }

    def test_help(self) -> None:
        result = self._run(["--help"])
        assert result.returncode == 0
        assert "Magic Studio CLI" in result.stdout

    def test_project_new_and_info_json(self, tmp_path: Path) -> None:
        env = self._isolated_env(tmp_path)
        proj = tmp_path / "SubprocessSong.magic"
        res = self._run(["--json", "project", "new", "-o", str(proj), "--name", "Subprocess Song"], env_extra=env)
        assert res.returncode == 0
        data = json.loads(res.stdout)
        assert data["ok"] is True
        assert Path(data["project"]).is_file()

        info_res = self._run(["--json", "project", "info"], env_extra=env)
        assert info_res.returncode == 0
        info_data = json.loads(info_res.stdout)
        assert info_data["name"] == "Subprocess Song"
        print(f"\n  Project: {proj} ({proj.stat().st_size:,} bytes)")

    def test_dry_run_suppresses_project_save(self, tmp_path: Path) -> None:
        env = self._isolated_env(tmp_path)
        proj = tmp_path / "DryRun.magic"
        res = self._run(["--json", "--dry-run", "project", "new", "-o", str(proj)], env_extra=env)
        assert res.returncode == 0
        assert not proj.exists()

    def test_headless_engine_hyperframes_website_roundtrip(self, tmp_path: Path) -> None:
        env = self._isolated_env(tmp_path)
        proj = tmp_path / "WebsiteApp.magic"
        self._run(["--json", "project", "new", "-o", str(proj), "--name", "WebsiteApp"], env_extra=env)

        # Create a HyperFrames website (index.html only, no hyperframes.json)
        hf_dir = tmp_path / "hyperframes"
        hf_dir.mkdir()
        (hf_dir / "index.html").write_text(
            "<!doctype html><html><body><div id='root' data-composition-id='main' data-fps='30' data-duration='5'></div></body></html>",
            encoding="utf-8",
        )

        # Verify the Rust engine recognizes `hyperframes/index.html` as a ready HyperFrames studio project
        status_res = self._run(["--json", "video", "status"], env_extra=env)
        assert status_res.returncode == 0
        status = json.loads(status_res.stdout)
        hf_target = next(t for t in status["targets"] if t["target"] == "hyperframes")
        assert hf_target["exists"] is True
        assert hf_target["path"].endswith("hyperframes/index.html")

    def test_headless_mutation_auto_saves_to_magic_file(self, tmp_path: Path) -> None:
        env = self._isolated_env(tmp_path)
        proj = tmp_path / "AutoSave.magic"
        self._run(["--json", "project", "new", "-o", str(proj), "--name", "AutoSave"], env_extra=env)

        # Mutate target to remotion via one-shot `exec workshop.set_target`
        mut_res = self._run(["--json", "exec", "workshop.set_target", "target=remotion"], env_extra=env)
        assert mut_res.returncode == 0
        saved_doc = json.loads(proj.read_text(encoding="utf-8"))
        assert saved_doc["workshop"]["target"] == "remotion"

    def test_commands_discovery(self, tmp_path: Path) -> None:
        env = self._isolated_env(tmp_path)
        res = self._run(["--json", "commands", "storyboard"], env_extra=env)
        assert res.returncode == 0
        data = json.loads(res.stdout)
        ids = {c["id"] for c in data["commands"]}
        assert {"storyboard.create", "storyboard.read", "storyboard.update", "storyboard.delete"}.issubset(ids)

    def test_mcp_stdio_protocol_and_tool_call(self, tmp_path: Path) -> None:
        env = self._isolated_env(tmp_path)
        proj = tmp_path / "McpSong.magic"
        self._run(["--json", "project", "new", "-o", str(proj), "--name", "McpSong"], env_extra=env)

        requests: list[dict[str, Any]] = [
            {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2026-07-28"}},
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
            {
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {"name": "workshop_status", "arguments": {}},
            },
        ]
        stdin_payload = "\n".join(json.dumps(r) for r in requests) + "\n"
        res = self._run(["mcp"], env_extra=env, input_text=stdin_payload)
        assert res.returncode == 0
        replies = [json.loads(line) for line in res.stdout.strip().splitlines()]
        assert len(replies) == 3
        assert replies[0]["result"]["protocolVersion"] == "2026-07-28"
        assert replies[0]["result"]["serverInfo"]["name"] == "magic"
        tools = replies[1]["result"]["tools"]
        assert any(t["name"] == "project_open" for t in tools)
        status_payload = json.loads(replies[2]["result"]["content"][0]["text"])
        assert Path(status_payload["project_dir"]).resolve() == proj.parent.resolve()
