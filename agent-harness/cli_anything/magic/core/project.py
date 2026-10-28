"""Project management for Magic Studio (`.magic` files and HyperFrames studio websites)."""

from __future__ import annotations

import json
import os
from pathlib import Path



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
