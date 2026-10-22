"""Generate every dialogue line in a production spec through local Voicebox.

WAVs are written next to the spec (paths stored relative to it), durations are
measured with ffprobe, and each shot is lengthened if its lines need more time.
"""
from __future__ import annotations

import json
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

from pipeline.production.spec import Line, load_spec, save_spec

BREATH_S = 0.4


class VoiceboxClient:
    def __init__(self, base: str = "http://127.0.0.1:17600", timeout_s: int = 900) -> None:
        self.base = base
        self.timeout_s = timeout_s

    def _json(self, path: str, payload: dict | None = None) -> dict:
        data = json.dumps(payload).encode() if payload is not None else None
        req = urllib.request.Request(self.base + path, data=data,
                                     headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=60) as r:
            return json.load(r)

    def generate(self, text: str, profile_id: str, instruct: str = "") -> bytes:
        payload = {"text": text, "profile_id": profile_id, "language": "en"}
        if instruct:
            payload["instruct"] = instruct
        gid = self._json("/generate", payload)["id"]
        deadline = time.time() + self.timeout_s
        while time.time() < deadline:
            time.sleep(1)
            status = self._json(f"/history/{gid}")
            if status.get("status") == "completed":
                with urllib.request.urlopen(f"{self.base}/audio/{gid}", timeout=60) as r:
                    return r.read()
            if status.get("status") == "failed":
                raise RuntimeError(f"Voicebox generation failed: {status.get('error')}")
        raise TimeoutError(f"Voicebox generation {gid} timed out")


def measure_duration(path: Path) -> float:
    out = subprocess.check_output(["ffprobe", "-v", "error", "-show_entries", "format=duration",
                                   "-of", "default=nw=1:nk=1", str(path)])
    return float(out.decode().strip())


def generate_lines(spec_path: str | Path, out_dir: str | Path, profiles: dict[str, str],
                   client=None, only: list[str] | None = None,
                   instructs: dict[str, str] | None = None) -> list[Line]:
    spec_path, out_dir = Path(spec_path), Path(out_dir)
    client = client or VoiceboxClient()
    spec = load_spec(spec_path)
    shots = [s for c in spec.chapters for s in c.shots]
    missing = sorted({l.speaker for s in shots for l in s.lines} - profiles.keys())
    if missing:
        raise KeyError(f"no voice profile for speakers: {', '.join(missing)}")
    out_dir.mkdir(parents=True, exist_ok=True)
    done: list[Line] = []
    for shot in shots:
        for line in shot.lines:
            wav = out_dir / f"{line.id}.wav"
            if not wav.exists() or (only is not None and line.id in only):
                instruct = (instructs or {}).get(line.speaker, "")
                if line.direction:
                    instruct = f"{instruct} {line.direction}".strip()
                wav.write_bytes(client.generate(line.text, profiles[line.speaker], instruct))
            line.wav = str(wav.relative_to(spec_path.parent))
            line.duration_s = round(measure_duration(wav), 3)
            done.append(line)
        needed = sum(l.duration_s or 0 for l in shot.lines) + BREATH_S * len(shot.lines)
        shot.duration_s = round(max(shot.duration_s, needed), 3)
        save_spec(spec, spec_path)  # save per shot so an interrupted run keeps its progress
    return done


def main(argv: list[str]) -> int:
    """`python -m pipeline.production.voice production.json casting.json [--only id,id]`
    casting.json: {"profiles": {SPEAKER: profile_id}, "instructs": {SPEAKER: "..."}}"""
    casting = json.loads(Path(argv[1]).read_text())
    only = argv[argv.index("--only") + 1].split(",") if "--only" in argv else None
    spec_path = Path(argv[0])
    lines = generate_lines(spec_path, spec_path.parent / "audio" / "vo", casting["profiles"],
                           only=only, instructs=casting.get("instructs"))
    print(json.dumps({"lines": len(lines), "speech_s": round(sum(l.duration_s for l in lines), 2),
                      "next": [f"python -m pipeline.production.timeline {spec_path} --boards"]}))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
