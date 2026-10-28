"""Rendering and export operations for Magic Studio."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from cli_anything.magic.core.session import Session


EXPORT_PRESETS: dict[str, dict[str, Any]] = {
    "storyboard_png": {
        "name": "storyboard_png",
        "command": "storyboard.render",
        "format": "png",
        "description": "Render native DeckCraft storyboard pages to PNG previews",
    },
    "video_snapshot": {
        "name": "video_snapshot",
        "command": "video.snapshot",
        "format": "png",
        "description": "Render a single frame PNG from the project's HyperFrames/Remotion studio",
    },
    "video_mp4": {
        "name": "video_mp4",
        "command": "video.render",
        "format": "mp4",
        "description": "Render full MP4 video from the project's HyperFrames/Remotion studio",
    },
    "layered_motion_webm": {
        "name": "layered_motion_webm",
        "command": "export.layered_motion",
        "format": "webm",
        "description": "Render transparent VP9 WebM tracks from a layered motion spec via EffectCraft",
    },
}


def list_presets() -> list[dict[str, Any]]:
    """Return available export presets."""
    return list(EXPORT_PRESETS.values())


def render(
    session: Session,
    preset: str = "storyboard_png",
    output_path: str | Path | None = None,
    extra_params: dict[str, Any] | None = None,
    bridge: str | None = None,
) -> dict[str, Any]:
    """Execute a real backend render/export using the given preset."""
    if preset not in EXPORT_PRESETS:
        raise ValueError(
            f"Unknown export preset '{preset}'. Valid presets: {', '.join(EXPORT_PRESETS)}"
        )
    spec = EXPORT_PRESETS[preset]
    params = dict(extra_params or {})
    if output_path:
        if spec["command"] in {"video.snapshot", "video.render"}:
            params["out"] = str(Path(output_path).expanduser().resolve())
        elif spec["command"] == "export.layered_motion":
            params["out_dir"] = str(Path(output_path).expanduser().resolve())
        else:
            params["output"] = str(Path(output_path).expanduser().resolve())
    return session.execute(spec["command"], params, bridge=bridge)
