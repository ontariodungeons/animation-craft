"""Project management for Magic Studio (`.magic` files and HyperFrames studio websites)."""

from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any

from cli_anything.magic.core.session import _locked_save_json


PROJECT_FORMAT = "magicstudio.project"

PROFILES: dict[str, dict[str, Any]] = {
    "hyperframes": {
        "name": "hyperframes",
        "target": "hyperframes",
        "fps": 30,
        "width": 1920,
        "height": 1080,
        "description": "HyperFrames HTML/GSAP video studio website (hyperframes/index.html)",
    },
    "remotion": {
        "name": "remotion",
        "target": "remotion",
        "fps": 30,
        "width": 1920,
        "height": 1080,
        "description": "Remotion React composition target",
    },
    "character": {
        "name": "character",
        "target": "hyperframes",
        "fps": 24,
        "width": 1080,
        "height": 1080,
        "description": "Isolated 2D/3D character rig and animation project",
    },
}


def list_profiles() -> list[dict[str, Any]]:
    """Return available Magic Studio project profiles."""
    return list(PROFILES.values())


def resolve_project_path(raw: str | Path) -> Path:
    """Resolve a `.magic` file path, a project directory, or a `hyperframes/` website folder."""
    p = Path(raw).expanduser()
    if p.is_file():
        if p.suffix.lower() == ".magic":
            return p.resolve()
        if p.name == "index.html" and p.parent.name == "hyperframes":
            parent_dir = p.parent.parent
            found = list(parent_dir.glob("*.magic"))
            if len(found) == 1:
                return found[0].resolve()
        raise ValueError(f"{p} is not a .magic project file")

    if p.is_dir():
        for anc in [p, *list(p.parents)[:2]]:
            found = sorted(anc.glob("*.magic"))
            if len(found) == 1:
                return found[0].resolve()
            if len(found) > 1:
                raise ValueError(f"{anc} has {len(found)} .magic files; specify the file path")
        raise ValueError(f"No .magic project file found in {p}")

    # Check recent projects catalog by name/stem
    config_home = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))
    recent_file = config_home / "magicstudio" / "recent_projects.json"
    if recent_file.is_file():
        try:
            recent = json.loads(recent_file.read_text(encoding="utf-8"))
            token = str(raw).strip()
            for entry in recent:
                ep = Path(entry)
                if ep.is_file() and (
                    ep.stem.lower() == token.lower() or ep.name.lower() == token.lower()
                ):
                    return ep.resolve()
        except Exception:
            pass

    if p.suffix.lower() != ".magic":
        p = p.with_suffix(".magic")
    return p.resolve() if p.exists() else p.absolute()


def create(
    path: str | Path,
    name: str | None = None,
    profile: str = "hyperframes",
    audio: str | None = None,
    overwrite: bool = False,
) -> dict[str, Any]:
    """Create a new `.magic` project file on disk."""
    p = Path(path).expanduser()
    if p.suffix.lower() != ".magic":
        p = p.with_suffix(".magic")
    p = p.absolute()
    if p.exists() and not overwrite:
        raise FileExistsError(f"Project already exists: {p} (pass overwrite=True to replace)")

    prof = PROFILES.get(profile, PROFILES["hyperframes"])
    proj_name = name or p.stem
    doc: dict[str, Any] = {
        "format": PROJECT_FORMAT,
        "version": 1,
        "name": proj_name,
        "assets": [],
        "workshop": {
            "audio": audio,
            "draft_approved": False,
            "target": prof["target"],
            "video_project": None,
        },
    }
    p.parent.mkdir(parents=True, exist_ok=True)
    _locked_save_json(p, doc)
    return {
        "ok": True,
        "project": str(p.resolve()),
        "name": proj_name,
        "profile": prof["name"],
        "document": doc,
    }


def open(path: str | Path) -> dict[str, Any]:
    """Open and validate an existing `.magic` project file."""
    resolved = resolve_project_path(path)
    if not resolved.is_file():
        raise FileNotFoundError(f"Project file not found: {resolved}")
    doc = json.loads(resolved.read_text(encoding="utf-8"))
    if doc.get("format") != PROJECT_FORMAT:
        raise ValueError(f"{resolved} is not a valid Magic Studio project")
    return {
        "ok": True,
        "project": str(resolved),
        "name": doc.get("name", resolved.stem),
        "assets": doc.get("assets", []),
        "workshop": doc.get("workshop", {}),
        "document": doc,
    }


def save(project_data: dict[str, Any], path: str | Path) -> dict[str, Any]:
    """Save `.magic` project data to `path` using exclusive file locking."""
    target = Path(path).expanduser()
    if target.suffix.lower() != ".magic":
        target = target.with_suffix(".magic")
    _locked_save_json(target, project_data)
    return {"ok": True, "project": str(target.resolve())}


def info(path: str | Path) -> dict[str, Any]:
    """Inspect a `.magic` project and its HyperFrames website / storyboard state."""
    opened = open(path)
    proj_file = Path(opened["project"])
    proj_dir = proj_file.parent
    hf_dir = proj_dir / "hyperframes"
    hf_index = hf_dir / "index.html"
    hf_json = hf_dir / "hyperframes.json"
    sb_decks = sorted((proj_dir / "storyboard").glob("*.deckcraft")) if (proj_dir / "storyboard").is_dir() else []

    return {
        "ok": True,
        "project": str(proj_file),
        "project_dir": str(proj_dir),
        "name": opened["name"],
        "asset_count": len(opened["assets"]),
        "assets": opened["assets"],
        "workshop": opened["workshop"],
        "hyperframes": {
            "exists": hf_index.is_file() or hf_json.is_file(),
            "index_html": str(hf_index) if hf_index.is_file() else None,
            "project_path": str(hf_json if hf_json.is_file() else hf_index)
            if (hf_index.is_file() or hf_json.is_file())
            else None,
        },
        "storyboard": {
            "exists": bool(sb_decks),
            "deck": str(sb_decks[0]) if sb_decks else None,
        },
    }
