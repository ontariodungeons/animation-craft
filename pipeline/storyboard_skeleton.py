#!/usr/bin/env python3
"""Write music-to-video's STORYBOARD.md skeleton from the transcript's captions (transcript.srt).

One frame per caption: its number, its time and its words. Time with no caption for 1 s or more
(before the first, between two, after the last) becomes an `instrumental` frame, a placeholder
to design. Frames tile the track. Each frame's `pacing`, first-pass `mood` and `feel` are read from
audiomap.json over its span. Groups stay `TBD (Step 3)` and `style` stays blank. Prints one JSON line.

  storyboard_skeleton.py --srt transcript.srt --audiomap audiomap.json --out STORYBOARD.md
"""
import argparse
import json
import re
from pathlib import Path

GAP = 1.0  # seconds without a caption that make an instrumental frame
MOOD = {"warm": "warm", "heavy": "cinematic", "bright": "hype", "full": "cinematic", "sparse": "dreamy"}


def captions(srt):
    """[(number, start, end, text)] from an SRT file."""
    stamp = re.compile(r"(\d+):(\d+):(\d+)[,.](\d+)\s*-->\s*(\d+):(\d+):(\d+)[,.](\d+)")
    seconds = lambda h, m, s, ms: int(h) * 3600 + int(m) * 60 + int(s) + int(ms) / 1000
    out = []
    for block in re.split(r"\n\s*\n", Path(srt).read_text().strip()):
        lines = block.strip().splitlines()
        hit = next((stamp.search(l) for l in lines if stamp.search(l)), None)
        if not hit:
            continue
        g = hit.groups()
        text = " ".join(l.strip() for l in lines[lines.index(hit.string) + 1:])
        out.append((int(lines[0]) if lines[0].strip().isdigit() else len(out) + 1,
                    seconds(*g[:4]), seconds(*g[4:]), text))
    return out


def spans(cues, dur):
    """Frames that tile [0, dur]: one per caption, plus instrumental frames for long gaps."""
    frames, t = [], 0.0
    for i, (num, start, end, text) in enumerate(cues):
        if start - t >= GAP:
            frames.append({"a": t, "b": start, "cue": None, "sung": ""})
            t = start
        nxt = cues[i + 1][1] if i + 1 < len(cues) else dur
        b = end if nxt - end >= GAP else nxt
        frames.append({"a": t, "b": b, "cue": num, "sung": text})
        t = b
    if dur - t > 0:
        if frames and dur - t < GAP:
            frames[-1]["b"] = dur
        else:
            frames.append({"a": t, "b": dur, "cue": None, "sung": ""})
    return frames


def describe(f, m):
    a, b = f["a"], f["b"]
    phases = [p for p in m["energy_phases"] if p["start"] < b and p["end"] > a]
    rolls = [r for r in m.get("rolls", []) if r["start"] < b and r["end"] > a]
    dense = any(p.get("density") == "dense" for p in phases)
    chars = [p.get("feel", {}).get("character") for p in phases]
    f["pacing"] = "beat_cut" if rolls or dense else "phrase_flow"
    f["mood"] = list(dict.fromkeys(MOOD[c] for c in chars if c in MOOD))[:3] or ["dreamy"]
    loud = max(phases, key=lambda p: p["energy"]) if phases else None
    bits = []
    if loud and loud["level"] != "VOID":
        bits.append(f"loudest {loud['level']} at {loud['start']:g}–{loud['end']:g} s")
    elif loud:
        bits.append("silent (VOID)")
    if rolls:
        bits.append("rolls: " + ", ".join(f"{r['drum']} {r['kind']} {r['start']:.1f}–{r['end']:.1f}" for r in rolls))
    moments = sorted((k["t"], k["kind"]) for k in m["key_moments"] if a <= k["t"] < b)
    if moments:
        bits.append(", ".join(f"{kind} at {t:g}" for t, kind in moments))
    f["feel"] = "; ".join(bits) or "steady"


def render(frames, dur, canvas):
    w, h, fps = canvas
    out = ["---", "compositionId: bgm", f"duration_s: {dur:g}", f"canvas: {{ w: {w}, h: {h}, fps: {fps} }}",
           "style:", "assets: false", 'build_notes: ["one paused timeline per frame", "no remote assets"]', "---"]
    for n, f in enumerate(frames, 1):
        out += ["", f"## Frame {n} — f{n}", "",
                f"- src: compositions/frames/{n:02d}-f{n}.html",
                f"- duration: {f['b'] - f['a']:.3f}s",
                f"- span_sec: [{f['a']:g}, {f['b']:g}]",
                f"- pacing: {f['pacing']}",
                f"- mood: [{', '.join(f['mood'])}]",
                f"- feel: {f['feel']}"]
        out += [f"- caption: {f['cue']}", f"- sung: \"{f['sung']}\""] if f["cue"] else ["- instrumental: true"]
        out += ["", "### Groups", "", "- TBD (Step 3)"]
    return "\n".join(out) + "\n"


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--srt", required=True)
    ap.add_argument("--audiomap", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--canvas", default="1920x1080x30", help="WxHxFPS")
    args = ap.parse_args()
    m = json.loads(Path(args.audiomap).read_text())
    dur = m["audio"]["duration_sec"]
    frames = spans(captions(args.srt), dur)
    for f in frames:
        describe(f, m)
    Path(args.out).write_text(render(frames, dur, [int(v) for v in args.canvas.split("x")]))
    print(json.dumps({"ok": True, "out": args.out, "frames": [
        {"span_sec": [round(f["a"], 3), round(f["b"], 3)], "caption": f["cue"], "instrumental": not f["cue"]}
        for f in frames]}))


if __name__ == "__main__":
    main()
