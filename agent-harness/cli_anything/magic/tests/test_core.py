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


def test_session_snapshot_undo_redo(tmp_path: Path) -> None:
    sess_file = tmp_path / "session.json"
    proj_file = tmp_path / "UndoTest.magic"
    created = proj_mod.create(proj_file, name="Initial")

    sess = Session(session_file=sess_file)
    sess.set_project(created["document"], proj_file)
    sess.snapshot()
    assert sess.project_data is not None
    sess.project_data["name"] = "Changed"

    sess.undo()
    assert sess.project_data["name"] == "Initial"

    sess.redo()
    assert sess.project_data["name"] == "Changed"


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
