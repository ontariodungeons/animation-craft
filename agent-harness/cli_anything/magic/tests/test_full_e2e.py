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
        tool_names = {t["name"] for t in tools}
        assert "project_open" in tool_names
        assert {"ui_look", "ui_click", "ui_set", "ui_key", "ui_state"}.issubset(tool_names)
        status_payload = json.loads(replies[2]["result"]["content"][0]["text"])
        assert Path(status_payload["project_dir"]).resolve() == proj.parent.resolve()

    def test_cli_live_bridge_mutation_does_not_overwrite_project_file(self, tmp_path: Path) -> None:
        """Finding 1 (E2E): One-shot CLI `magic project add-asset` over a live bridge must not overwrite newer .magic data."""
        import socket
        import threading

        env = self._isolated_env(tmp_path)
        proj = tmp_path / "LiveCli.magic"
        self._run(["--json", "project", "new", "-o", str(proj), "--name", "LiveCli"], env_extra=env)

        sock_path = tmp_path / "live-cli.sock"

        def bridge_server() -> None:
            srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            srv.bind(str(sock_path))
            srv.listen(2)
            try:
                conn, _ = srv.accept()
                with conn:
                    raw = conn.makefile("r", encoding="utf-8").readline()
                    req = json.loads(raw)
                    disk_doc = json.loads(proj.read_text(encoding="utf-8"))
                    disk_doc.setdefault("assets", []).append(
                        {"id": "bridge-written-asset", "path": req["params"]["path"]}
                    )
                    proj.write_text(json.dumps(disk_doc, indent=2), encoding="utf-8")
                    conn.sendall(
                        (
                            json.dumps(
                                {
                                    "ok": True,
                                    "result": {
                                        "id": "bridge-written-asset",
                                        "path": req["params"]["path"],
                                    },
                                }
                            )
                            + "\n"
                        ).encode("utf-8")
                    )
            finally:
                srv.close()

        t = threading.Thread(target=bridge_server, daemon=True)
        t.start()

        res = self._run(
            [
                "--json",
                "--bridge",
                f"unix:{sock_path}",
                "project",
                "add-asset",
                "sprites/bee.png",
                "--kind",
                "image",
            ],
            env_extra=env,
        )
        t.join(timeout=2.0)
        assert res.returncode == 0
        saved_doc = json.loads(proj.read_text(encoding="utf-8"))
        assert len(saved_doc.get("assets", [])) == 1
        assert saved_doc["assets"][0]["id"] == "bridge-written-asset"

    def test_dry_run_project_save_o_does_not_create_file(self, tmp_path: Path) -> None:
        """Finding 2: `magic --dry-run project save -o <temp-path>` must not create the file."""
        env = self._isolated_env(tmp_path)
        proj = tmp_path / "Base.magic"
        self._run(["--json", "project", "new", "-o", str(proj), "--name", "Base"], env_extra=env)

        save_as_target = tmp_path / "ShouldNotBeCreated.magic"
        res = self._run(
            ["--json", "--dry-run", "project", "save", "-o", str(save_as_target)],
            env_extra=env,
        )
        assert res.returncode == 0
        payload = json.loads(res.stdout)
        assert payload.get("dry_run") is True
        assert not save_as_target.exists()

    def test_magic_run_headless_and_live_bridge(self, tmp_path: Path) -> None:
        """Finding 3: `magic run` must execute JSONL batches in both headless and live sessions."""
        import socket
        import threading

        env = self._isolated_env(tmp_path)
        proj = tmp_path / "BatchRun.magic"
        self._run(["--json", "project", "new", "-o", str(proj), "--name", "BatchRun"], env_extra=env)

        batch_lines = (
            json.dumps({"command": "workshop.status", "params": {}})
            + "\n"
            + json.dumps({"command": "workshop.set_target", "params": {"target": "remotion"}})
            + "\n"
        )
        headless_res = self._run(["run", "-"], env_extra=env, input_text=batch_lines)
        assert headless_res.returncode == 0
        replies = [json.loads(line) for line in headless_res.stdout.strip().splitlines()]
        assert len(replies) == 2
        assert replies[0]["ok"] is True
        assert replies[1]["ok"] is True
        saved_doc = json.loads(proj.read_text(encoding="utf-8"))
        assert saved_doc["workshop"]["target"] == "remotion"

        # Now test `magic run` routed to a live bridge socket
        sock_path = tmp_path / "live-batch.sock"

        def batch_bridge() -> None:
            srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            srv.bind(str(sock_path))
            srv.listen(4)
            try:
                for _ in range(2):
                    conn, _ = srv.accept()
                    with conn:
                        raw = conn.makefile("r", encoding="utf-8").readline()
                        req = json.loads(raw)
                        conn.sendall(
                            (
                                json.dumps(
                                    {"ok": True, "result": {"live_cmd": req["id"]}}
                                )
                                + "\n"
                            ).encode("utf-8")
                        )
            finally:
                srv.close()

        t = threading.Thread(target=batch_bridge, daemon=True)
        t.start()

        live_res = self._run(
            ["--bridge", f"unix:{sock_path}", "run", "-"],
            env_extra=env,
            input_text=batch_lines,
        )
        t.join(timeout=2.0)
        assert live_res.returncode == 0
        live_replies = [json.loads(line) for line in live_res.stdout.strip().splitlines()]
        assert len(live_replies) == 2
        assert live_replies[0]["result"]["live_cmd"] == "workshop.status"
        assert live_replies[1]["result"]["live_cmd"] == "workshop.set_target"

    def test_headless_save_error_propagates_nonzero(self, tmp_path: Path) -> None:
        """Finding 4: Automatic project write failures must propagate and exit non-zero."""
        env = self._isolated_env(tmp_path)
        proj_dir = tmp_path / "ro_proj"
        proj_dir.mkdir()
        proj = proj_dir / "ReadOnly.magic"
        self._run(["--json", "project", "new", "-o", str(proj), "--name", "ReadOnly"], env_extra=env)

        # Make the .magic file and parent dir read-only so write_project_file fails
        proj.chmod(0o444)
        proj_dir.chmod(0o555)
        try:
            res = self._run(
                ["--json", "exec", "workshop.set_target", "target=remotion"],
                check=False,
                env_extra=env,
            )
            assert res.returncode != 0
            combined = res.stdout + res.stderr
            assert "Permission denied" in combined or "os error 13" in combined
        finally:
            proj_dir.chmod(0o755)
            proj.chmod(0o644)
