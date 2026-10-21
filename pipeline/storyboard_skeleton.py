#!/usr/bin/env python3
"""Write music-to-video's STORYBOARD.md skeleton (its Step 2) from audiomap.json + transcript.json.

Applies the rules in music-to-video references/frame-skeleton.md, plus the lyrics:
  cuts      where the music drops into an onset desert: a DROP key moment or hard stop that
            starts a VOID energy phase (if there are none, phrase edges);
  snap      to the nearest beat within half a beat, only when the grid is reliable
            (rolls present or a dense phase); otherwise the audiomap anchor stays;
  merge     a frame shorter than one bar into its shorter neighbour; never cut inside a roll;
  pacing    beat_cut when a roll or a dense phase falls in the frame, else phrase_flow;
  mood      first pass from the phases' spectral character (edit it freely);
  feel      one line of measured facts (levels, rolls, key moments, how it ends);
  sung      the transcript's lines whose midpoint is in the frame (lines are never split);
            a frame with none is `instrumental`.
Groups stay `TBD (Step 3)` and `style` stays blank, as the skill requires. Prints one JSON line.

  storyboard_skeleton.py --audiomap audiomap.json --transcript transcript.json --out STORYBOARD.md
"""
import argparse
import json
from pathlib import Path

MOOD = {"warm": "warm", "heavy": "cinematic", "bright": "hype", "full": "cinematic", "sparse": "dreamy"}


def words_of(transcript):
    doc = json.loads(Path(transcript).read_text())
    words = doc if isinstance(doc, list) else doc.get("words", [])
    return [w for w in words if str(w.get("text", "")).strip()]


def lines_of(words):
    """Sung lines: words up to sentence punctuation, each with its start and end."""
    lines, cur = [], []
    for w in words:
        cur.append(w)
        if str(w["text"]).rstrip().endswith((".", "!", "?")):
            lines.append(cur)
            cur = []
    if cur:
        lines.append(cur)
    return [{"text": " ".join(w["text"] for w in ln), "start": ln[0]["start"], "end": ln[-1]["end"]} for ln in lines]


def cut_times(m):
    voids = {round(p["start"], 3) for p in m["energy_phases"] if p["level"] == "VOID" and p["start"] > 0}
    marks = [(k["t"], abs(k.get("delta", 0))) for k in m["key_moments"] if k["kind"] == "DROP"]
    marks += [(h["t"], abs(h.get("delta", 0))) for h in m["hard_stops"]]
    cuts = sorted({t for t, _ in marks if round(t, 3) in voids})
    source = "drops into silence"
    if not cuts:
        cuts = [p["start"] for p in m["phrases"][1:]]
        source = "phrase edges"
    return cuts, source


def build(m, words, canvas):
    dur = m["audio"]["duration_sec"]
    beats = m["grid"].get("beats_sec") or []
    bpm = m["tempo"].get("bpm") or 0
    half = 30.0 / bpm if bpm else 0
    bar = 4 * 60.0 / bpm if bpm else 0
    rolls = m.get("rolls", [])
    reliable = bool(rolls) or any(p.get("density") == "dense" for p in m["energy_phases"])
    in_roll = lambda t: any(r["start"] < t < r["end"] for r in rolls)

    cuts, source = cut_times(m)
    snapped = []
    for t in cuts:
        near = min(beats, key=lambda b: abs(b - t)) if beats else t
        s = near if reliable and abs(near - t) <= half and not in_roll(near) else t
        if not in_roll(s) and 0 < s < dur:
            snapped.append(round(s, 3))
    edges = [0.0] + sorted(set(snapped)) + [round(dur, 3)]

    # Merge sub-bar frames into the shorter neighbour until none are left.
    while bar and len(edges) > 2:
        spans = [(edges[i + 1] - edges[i], i) for i in range(len(edges) - 1)]
        short = [i for length, i in spans if length < bar]
        if not short:
            break
        i = short[0]
        prev_len = spans[i - 1][0] if i > 0 else float("inf")
        next_len = spans[i + 1][0] if i + 1 < len(spans) else float("inf")
        del edges[i if prev_len <= next_len else i + 1]

    frames = []
    for n, (a, b) in enumerate(zip(edges, edges[1:]), 1):
        phases = [p for p in m["energy_phases"] if p["start"] < b and p["end"] > a]
        f_rolls = [r for r in rolls if r["start"] < b and r["end"] > a]
        dense = any(p.get("density") == "dense" for p in phases)
        chars = [p.get("feel", {}).get("character") for p in phases]
        mood = list(dict.fromkeys(MOOD[c] for c in chars if c in MOOD))[:3] or ["dreamy"]
        moments = sorted((k["t"], k["kind"]) for k in m["key_moments"] if a <= k["t"] < b)
        loud = max(phases, key=lambda p: p["energy"]) if phases else None
        bits = []
        if loud and loud["level"] == "VOID":
            bits.append("silent (VOID)")
        elif loud:
            bits.append(f"loudest {loud['level']} at {loud['start']:g}–{loud['end']:g} s (energy {loud['energy']:.2f})")
        if f_rolls:
            bits.append("rolls: " + ", ".join(f"{r['drum']} {r['kind']} {r['start']:.1f}–{r['end']:.1f}" for r in f_rolls))
        if moments:
            bits.append(", ".join(f"{kind} at {t:g}" for t, kind in moments))
        if not any(p.get("onsets") for p in phases):
            bits.append("no onsets")
        # A whole line belongs to the frame holding its midpoint, so no line is split.
        sung = [ln["text"] for ln in lines_of(words) if a <= (ln["start"] + ln["end"]) / 2 < b]
        frames.append({
            "n": n, "a": a, "b": b,
            "pacing": "beat_cut" if f_rolls or dense else "phrase_flow",
            "mood": mood, "feel": "; ".join(bits) or "steady",
            "sung": " ".join(sung),
        })
    return {"dur": dur, "bpm": bpm, "reliable": reliable, "source": source, "frames": frames, "canvas": canvas}


def render(plan, summary):
    w, h, fps = plan["canvas"]
    out = [
        "---",
        "compositionId: bgm",
        f"duration_s: {plan['dur']:g} # == audiomap.audio.duration_sec",
        f"canvas: {{ w: {w}, h: {h}, fps: {fps} }}",
        "style: # blank — Step 3 fills it from the chosen frame.md preset",
        "assets: false",
        'build_notes: ["one paused timeline per frame", "no remote assets"]',
        f"# Skeleton written by storyboard_skeleton.py: {summary}. Cuts from {plan['source']};",
        f"# beat grid {'reliable (snapped)' if plan['reliable'] else 'not reliable (anchors kept)'}. Mood is a first pass: edit it.",
        "---",
    ]
    for f in plan["frames"]:
        n, a, b = f["n"], f["a"], f["b"]
        out += [
            "",
            f"## Frame {n} — f{n}",
            "",
            f"- src: compositions/frames/{n:02d}-f{n}.html",
            f"- duration: {b - a:.3f}s",
            f"- span_sec: [{a:g}, {b:g}]",
            f"- pacing: {f['pacing']}",
            f"- mood: [{', '.join(f['mood'])}]",
            f"- feel: {f['feel']}",
            f"- sung: \"{f['sung']}\"" if f["sung"] else "- instrumental: true # no words in the transcript: a placeholder to design",
            "",
            "### Groups",
            "",
            "- TBD (Step 3)",
        ]
    return "\n".join(out) + "\n"


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--audiomap", required=True)
    ap.add_argument("--transcript", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--canvas", default="1920x1080x30", help="WxHxFPS")
    args = ap.parse_args()
    m = json.loads(Path(args.audiomap).read_text())
    canvas = [int(v) for v in args.canvas.split("x")]
    plan = build(m, words_of(args.transcript), canvas)
    Path(args.out).write_text(render(plan, m.get("summary", "")))
    print(json.dumps({"ok": True, "out": args.out, "source": plan["source"], "reliable_grid": plan["reliable"],
                      "frames": [{"span_sec": [f["a"], f["b"]], "pacing": f["pacing"],
                                  "instrumental": not f["sung"]} for f in plan["frames"]]}))


if __name__ == "__main__":
    main()
