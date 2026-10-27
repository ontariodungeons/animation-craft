"""Stage 3B: Trace cutout part PNGs into multi-color SVGs via vectorcraft-cli."""
from __future__ import annotations

import json
import shutil
import subprocess
from pathlib import Path
from typing import Any, Dict, List


def trace_part_layers_with_vectorcraft(
    rig_plan_path: Path,
    preset: str = "16 Colors",
) -> Dict[str, Any]:
    """Run vectorcraft-cli imageTrace.makeAndExpand on each cutout part PNG."""
    vc_bin = shutil.which("vectorcraft-cli") or "vectorcraft-cli"
    plan = json.loads(rig_plan_path.read_text(encoding="utf-8"))
    base_dir = rig_plan_path.parent
    traced_parts: List[Dict[str, Any]] = []

    for part in plan.get("parts", []):
        pid = part.get("id") or part.get("part") or "part"
        crop_rel = part.get("cropped_path") or part.get("cropped_file") or part.get("file", "")
        crop_path = Path(crop_rel) if Path(crop_rel).is_absolute() else (base_dir / crop_rel)
        if not crop_path.is_file():
            continue
        svg_path = crop_path.with_suffix(".svg")
        trace_params = json.dumps({"preset": preset, "enhanced": True, "params": {"ignoreWhite": True}})
        proc = subprocess.run(
            [
                vc_bin, "run", "--in", str(crop_path),
                "--cmd", "select.all",
                "--cmd", "imageTrace.makeAndExpand", "--params", trace_params,
                "--export", str(svg_path),
            ],
            capture_output=True, text=True, timeout=30,
        )
        if proc.returncode == 0 and svg_path.is_file():
            part["svg_path"] = str(svg_path)
            try:
                part["svg_file"] = str(svg_path.relative_to(base_dir))
            except ValueError:
                part["svg_file"] = svg_path.name
            traced_parts.append({"part": pid, "svg_path": str(svg_path), "bytes": svg_path.stat().st_size})

    rig_plan_path.write_text(json.dumps(plan, indent=2), encoding="utf-8")
    return {
        "ok": len(traced_parts) > 0,
        "rig_plan": str(rig_plan_path),
        "preset": preset,
        "traced_count": len(traced_parts),
        "traced_parts": traced_parts,
    }
