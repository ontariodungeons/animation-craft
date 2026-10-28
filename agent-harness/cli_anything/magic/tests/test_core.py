"""Unit tests for Magic Studio CLI-Anything core modules."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from cli_anything.magic.core import export as export_mod
from cli_anything.magic.core import project as proj_mod
from cli_anything.magic.core.session import Session, _locked_save_json
from cli_anything.magic.magic_cli import _preprocess_argv, parse_kv_args


def test_list_profiles() -> None:
    profiles = proj_mod.list_profiles()
    names = {p["name"] for p in profiles}
    assert {"hyperframes", "remotion", "character"}.issubset(names)


def test_create_and_open_project(tmp_path: Path) -> None:
    proj_file = tmp_path / "DemoSong.magic"
    created = proj_mod.create(proj_file, name="Demo Song", profile="hyperframes")
    assert created["ok"] is True
    assert Path(created["project"]).is_file()

    opened = proj_mod.open(proj_file)
    assert opened["name"] == "Demo Song"
    assert opened["workshop"]["target"] == "hyperframes"


def test_create_project_refuses_overwrite(tmp_path: Path) -> None:
    proj_file = tmp_path / "Existing.magic"
    proj_mod.create(proj_file, name="Existing")
    with pytest.raises(FileExistsError):
        proj_mod.create(proj_file, name="Existing", overwrite=False)


def test_resolve_project_from_hyperframes_website(tmp_path: Path) -> None:
    proj_file = tmp_path / "WebsiteSong.magic"
    proj_mod.create(proj_file, name="Website Song")
    hf_dir = tmp_path / "hyperframes"
    hf_dir.mkdir()
    index_html = hf_dir / "index.html"
    index_html.write_text("<!doctype html><html><body>HyperFrames</body></html>", encoding="utf-8")

    resolved_from_dir = proj_mod.resolve_project_path(hf_dir)
    resolved_from_html = proj_mod.resolve_project_path(index_html)
    assert resolved_from_dir == proj_file.resolve()
    assert resolved_from_html == proj_file.resolve()


def test_project_info_detects_hyperframes_website_without_json(tmp_path: Path) -> None:
    proj_file = tmp_path / "NoJson.magic"
    proj_mod.create(proj_file, name="NoJson")
    hf_dir = tmp_path / "hyperframes"
    hf_dir.mkdir()
    index_html = hf_dir / "index.html"
    index_html.write_text("<div id='root' data-composition-id='main'></div>", encoding="utf-8")

    details = proj_mod.info(proj_file)
    assert details["hyperframes"]["exists"] is True
    assert details["hyperframes"]["index_html"] == str(index_html)
    assert details["hyperframes"]["project_path"] == str(index_html)


def test_locked_save_json_persists_atomically(tmp_path: Path) -> None:
    target = tmp_path / "nested" / "state.json"
    _locked_save_json(target, {"hello": "world", "count": 42})
    loaded = json.loads(target.read_text(encoding="utf-8"))
    assert loaded == {"hello": "world", "count": 42}


def test_session_set_project_and_reload(tmp_path: Path) -> None:
    sess_file = tmp_path / "session.json"
    proj_file = tmp_path / "Bound.magic"
    created = proj_mod.create(proj_file, name="Bound")

    s1 = Session(session_file=sess_file)
    s1.set_project(created["document"], proj_file, studio_endpoint="unix:/tmp/fake.sock")

    s2 = Session(session_file=sess_file)
    assert s2.project_path == str(proj_file.resolve())
    assert s2.studio_endpoint == "unix:/tmp/fake.sock"


def test_live_bridge_mutation_does_not_overwrite_project(tmp_path: Path) -> None:
    """Finding 1: A live command must never overwrite newer engine-written project data."""
    import socket
    import threading

    sess_file = tmp_path / "session.json"
    proj_file = tmp_path / "LiveBridge.magic"
    created = proj_mod.create(proj_file, name="Initial")
    sock_path = tmp_path / "studio.sock"

    def bridge_server() -> None:
        srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        srv.bind(str(sock_path))
        srv.listen(2)
        try:
            conn, _ = srv.accept()
            with conn:
                raw = conn.makefile("r", encoding="utf-8").readline()
                req = json.loads(raw)
                # Simulate the Rust Studio bridge writing a newer asset to the .magic file on disk
                disk_doc = json.loads(proj_file.read_text(encoding="utf-8"))
                disk_doc.setdefault("assets", []).append(
                    {"id": "live-asset-1", "path": req["params"]["path"]}
                )
                proj_file.write_text(json.dumps(disk_doc, indent=2), encoding="utf-8")
                reply = {
                    "ok": True,
                    "result": {"id": "live-asset-1", "path": req["params"]["path"]},
                }
                conn.sendall((json.dumps(reply) + "\n").encode("utf-8"))
        finally:
            srv.close()

    t = threading.Thread(target=bridge_server, daemon=True)
    t.start()

    sess = Session(session_file=sess_file)
    sess.set_project(created["document"], proj_file, studio_endpoint=f"unix:{sock_path}")
    # Execute a live command and then save_session (as the CLI teardown hook does)
    res = sess.execute("project.add_asset", {"path": "images/hero.png", "kind": "image"})
    assert res["id"] == "live-asset-1"
    sess.save_session()
    t.join(timeout=2.0)

    # Disk must retain the live Studio asset and NOT be overwritten by sess.project_data
    after_doc = json.loads(proj_file.read_text(encoding="utf-8"))
    assert len(after_doc.get("assets", [])) == 1
    assert after_doc["assets"][0]["id"] == "live-asset-1"


def test_dry_run_suppresses_live_and_headless_writes(tmp_path: Path) -> None:
    """Finding 2: dry_run must return a dry-run receipt without mutating or writing files."""
    sess_file = tmp_path / "session.json"
    proj_file = tmp_path / "DrySource.magic"
    created = proj_mod.create(proj_file, name="DrySource")
    dest_file = tmp_path / "DrySaved.magic"

    sess = Session(session_file=sess_file)
    sess.set_project(created["document"], proj_file)

    receipt = sess.execute("project.save", {"path": str(dest_file)}, dry_run=True)
    assert receipt["dry_run"] is True
    assert receipt["command"] == "project.save"
    assert not dest_file.exists()

    live_receipt = sess.execute(
        "project.save",
        {"path": str(dest_file)},
        bridge=f"unix:{tmp_path / 'unused.sock'}",
        dry_run=True,
    )
    assert live_receipt["dry_run"] is True
    assert not dest_file.exists()


def test_mcp_advertises_all_bridge_ui_tools_and_dispatches(tmp_path: Path) -> None:
    """Finding 5: MCP must advertise bridge-only UI controls and dispatch them."""
    import socket
    import threading
    from cli_anything.magic.core.mcp_server import BRIDGE_UI_TOOLS, build_mcp_tools

    tools = build_mcp_tools()
    tool_names = {t["name"] for t in tools}
    expected_ui = {t["name"] for t in BRIDGE_UI_TOOLS}
    assert {"ui_look", "ui_click", "ui_set", "ui_key", "ui_state", "ui_elements", "ui_drag", "ui_screenshot", "ui_focus", "ui_eval"}.issubset(tool_names)
    assert expected_ui.issubset(tool_names)

    sock_path = tmp_path / "mcp-ui.sock"
    received: list[dict[str, object]] = []

    def ui_bridge() -> None:
        srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        srv.bind(str(sock_path))
        srv.listen(2)
        try:
            conn, _ = srv.accept()
            with conn:
                raw = conn.makefile("r", encoding="utf-8").readline()
                req = json.loads(raw)
                received.append(req)
                conn.sendall((json.dumps({"ok": True, "result": {"clicked": req["params"]["target"]}}) + "\n").encode("utf-8"))
        finally:
            srv.close()

    t = threading.Thread(target=ui_bridge, daemon=True)
    t.start()

    sess = Session(session_file=tmp_path / "session.json")
    out = sess.execute("ui.click", {"target": "tab-storyboard"}, bridge=f"unix:{sock_path}")
    t.join(timeout=2.0)
    assert out.get("clicked") == "tab-storyboard"
    assert received and received[0]["id"] == "ui.click"


def test_session_closed_studio_refuses_fallback(tmp_path: Path) -> None:
    sess_file = tmp_path / "session.json"
    sess = Session(session_file=sess_file)
    sess.studio_endpoint = f"unix:{tmp_path / 'nonexistent-studio.sock'}"
    with pytest.raises(RuntimeError, match="assigned Studio is unavailable"):
        sess.execute("workshop.status", {})


def test_export_presets() -> None:
    presets = export_mod.list_presets()
    names = {p["name"] for p in presets}
    assert {"storyboard_png", "video_snapshot", "video_mp4", "layered_motion_webm"} == names


def test_parse_kv_args() -> None:
    parsed = parse_kv_args(("scene=2", "fps=24.5", "save=true", "draft=false", "field=action"))
    assert parsed == {
        "scene": 2,
        "fps": 24.5,
        "save": True,
        "draft": False,
        "field": "action",
    }


def test_preprocess_argv_studio_shorthand() -> None:
    assert _preprocess_argv(["studio", "Bumble Bee.magic"]) == [
        "studio",
        "open",
        "Bumble Bee.magic",
    ]
    assert _preprocess_argv(["studio", "new"]) == ["studio", "new"]
    assert _preprocess_argv(["studio", "list"]) == ["studio", "list"]


def test_cli_domain_groups_and_no_legacy_prefixes() -> None:
    from cli_anything.magic.magic_cli import cli
    from cli_anything.magic.utils import magic_backend as backend

    expected_groups = {
        "studio",
        "project",
        "storyboard",
        "character",
        "part",
        "image",
        "motion",
        "animation",
        "audio",
        "words",
        "skills",
        "crew",
        "agent",
        "analytics",
        "video",
        "ui",
        "export",
        "session",
        "app",
    }
    assert expected_groups.issubset(set(cli.commands.keys()))
    assert not any(
        k in cli.commands for k in ("backstage", "booth", "center", "tui", "sprocket")
    )

    specs = backend.list_engine_commands()
    assert len(specs) > 100
    for spec in specs:
        assert not str(spec["id"]).startswith(backend.LEGACY_COMMAND_PREFIXES)
