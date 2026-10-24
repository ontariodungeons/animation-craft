#!/usr/bin/env python3
"""Hey Ho Little Fish: the lyric design for every sung line, saved with `mc exec words.save`.

Art direction (1920x1080, the video's cut-paper sea):
- verses sit on the sand band at the bottom in the video's teal ink, the hero words in rust serif;
- choruses ride the open water at the top in cream and sand gold with a teal outline;
- every line has hero words in a serif, set large, with a move that acts the word out on its sung
  beat, and a hand-drawn stroke that draws on under it. Small sans words carry the rest.
Rows are laid out from measured text (the same font files the video renders with), centred, and
scaled down when a row would not fit the frame, so nothing runs off the edge.

Usage: heyho-design.py <mc> <bridge-port>   (reads the sections with words.read, saves each one)
"""
import json, subprocess, sys
from PIL import ImageFont

MC, PORT = sys.argv[1], sys.argv[2]
W, MARGIN, GAP = 1920, 110, 0.24  # frame width, side margin, gap between groups (in em of the larger neighbour)
TEAL, RUST, GOLD, CREAM, BLUE, OUT = "#173F49", "#B8442A", "#F2C14E", "#FFF8E9", "#8EC3FF", "#173B43"
ANDIKA = "/run/media/jesse/Speedy/media/hey-ho-little-fish/project/hyperframes/assets/vendor/Andika-Bold.ttf"
FILES = {("serif", 400, True): "/usr/share/fonts/liberation/LiberationSerif-Italic.ttf",
         ("serif", 700, False): "/usr/share/fonts/liberation/LiberationSerif-Bold.ttf",
         ("sans", 400, False): "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
         ("song", 700, False): ANDIKA}
STROKES = {
    "underline": ("0 0 280 30", "M 5 14 Q 61 5 125 13 T 269 10", 0.55),
    "swoop": ("0 0 790 160", "M 10 145 Q 126 170 217 93 T 438 58 Q 578 29 756 7 M 730 5 L 761 5 L 746 29", 1.6),
    "loop": ("0 0 620 110", "M 15 20 C 110 95 425 95 455 40 C 485 -4 350 -3 352 37 C 355 95 515 85 602 50 M 579 33 L 608 49 L 586 73", 1.6),
    "hill": ("0 0 410 95", "M 7 77 Q 73 72 118 32 Q 171 -8 228 31 Q 279 75 400 76", 1.4),
    "down": ("0 0 160 330", "M 73 13 C 27 67 104 129 66 196 Q 47 231 61 289 M 30 264 L 61 294 L 87 260", 1.0),
}

def look(font, weight, italic, size, color, outline=None, motion="quiet"):
    return dict(font=font, weight=weight, italic=italic, size=size, color=color, outline=outline, motion=motion)

sand = lambda n: (n, look("sans", 400, False, 50, TEAL))
water = lambda n: (n, look("sans", 400, False, 52, CREAM, OUT))
def hero(n, size, motion, style="serif-italic", color=RUST, outline=None):
    font, weight, italic = {"serif-italic": ("serif", 400, True), "serif-bold": ("serif", 700, False), "song-bold": ("song", 700, False)}[style]
    return (n, look(font, weight, italic, size, color, outline, motion))

# Each line: rows of (baseline y, groups of consecutive sung words), then strokes (kind, group index, placement).
LINES = {
    "v1-l1": ([(985, [sand(4), hero(1, 140, "pop"), hero(3, 64, "settle", "song-bold", TEAL)])], [("underline", 1, "under")]),
    "v1-l2": ([(150, [water(4), hero(1, 130, "pop", color=GOLD, outline=OUT)]),
               (285, [water(2), hero(1, 120, "rise", "song-bold", CREAM, OUT), water(2), hero(1, 120, "settle", "serif-bold", GOLD, OUT)])],
              [("swoop", 3, "row")]),
    "ch-l1": ([(165, [hero(2, 100, "pop", "song-bold", GOLD, OUT), water(1), hero(1, 160, "loop", color=CREAM, outline=OUT)]),
               (300, [water(1), hero(1, 100, "fall", "serif-bold", BLUE, OUT), water(1), hero(1, 100, "fall", "serif-bold", BLUE, OUT)])],
              [("loop", 2, "after")]),
    "ch-l2": ([(135, [hero(2, 84, "quiet", "song-bold", GOLD, OUT), water(2)]),
               (330, [water(3), hero(1, 200, "pop", "serif-bold", CREAM, OUT), hero(3, 84, "east", color=GOLD, outline=OUT)])],
              [("hill", 3, "under")]),
    "v2-l1": ([(985, [sand(3), hero(2, 130, "pop"), sand(5), hero(1, 84, "settle", "song-bold", TEAL)])], [("underline", 1, "under")]),
    "v2-l2": ([(985, [sand(4), hero(1, 130, "fall", "serif-bold"), sand(3), hero(1, 130, "loop")])], [("down", 3, "after")]),
    "v3-l1": ([(985, [sand(2), hero(1, 130, "east"), hero(1, 130, "loop"), sand(3), hero(1, 84, "pop", "song-bold", TEAL)])], [("loop", 2, "under")]),
    "v3-l2": ([(985, [sand(2), hero(3, 110, "fall", "serif-bold"), sand(3), hero(1, 130, "settle")])], [("underline", 3, "under")]),
}

def width(text, lk):
    # The outline stroke and an italic's overhang widen a word beyond its advance width.
    extra = lk["size"] * ((0.07 if lk["outline"] else 0) + (0.12 if lk["italic"] else 0))
    return ImageFont.truetype(FILES[(lk["font"], lk["weight"], lk["italic"])], round(lk["size"])).getlength(text) + extra

def mc(*args):
    out = subprocess.run([MC, "--bridge", PORT, "exec", *args], capture_output=True, text=True)
    if out.returncode: raise SystemExit(f"mc {args[0]} failed: {out.stderr or out.stdout}")
    return json.loads(out.stdout)

def design(words, rows, strokes, line_start):
    items, boxes, at = [], [], 0
    for baseline, groups in rows:
        placed = []
        for count, lk in groups:
            ws = words[at:at + count]; at += count
            placed.append((ws, dict(lk)))
        # Fit the row: measured widths plus gaps, scaled down together if wider than the frame allows.
        def total(scale):
            ws = [width(" ".join(w["text"] for w in g), {**lk, "size": lk["size"] * scale}) for g, lk in placed]
            gaps = [GAP * max(a["size"], b["size"]) * scale for (_, a), (_, b) in zip(placed, placed[1:])]
            return ws, gaps
        scale = 1.0
        while sum(sum(x) for x in total(scale)) > W - 2 * MARGIN: scale *= 0.96
        ws, gaps = total(scale)
        x = (W - sum(ws) - sum(gaps)) / 2
        for k, ((g, lk), w) in enumerate(zip(placed, ws)):
            lk["size"] = round(lk["size"] * scale)
            top = baseline - lk["size"] * 0.92  # the line box top for a 1.05 line height, baselines aligned
            items.append({**{k2: v for k2, v in lk.items()}, "word_ids": [w_["word_id"] for w_ in g], "x": round(x), "y": round(top),
                          "rotate": 0, "accent": g[-1]["word_id"] if lk["motion"] != "quiet" else None})
            boxes.append((x, top, w, lk["size"], g[-1]))
            x += w + (gaps[k] if k < len(gaps) else 0)
    assert at == len(words), f"{words[0]['line_id']}: rows place {at} of {len(words)} words"
    draws = []
    for kind, group, where in strokes:
        box, d, length = STROKES[kind]
        x, top, w, size, last = boxes[group]
        vb = [float(n) for n in box.split()]
        if where == "row":  # under the whole row the word sits in
            rx = min(b[0] for b in boxes if b[1] == top or abs(b[1] + b[3] * 0.92 - (top + size * 0.92)) < 2)
            rw = max(b[0] + b[2] for b in boxes if abs(b[1] + b[3] * 0.92 - (top + size * 0.92)) < 2) - rx
            dw = rw * 0.8; dh = dw * vb[3] / vb[2]
            sx, sy = rx + rw * 0.1, top + size * 1.0
        elif where == "under":
            dw = max(w, 160); dh = dw * vb[3] / vb[2]
            sx, sy = x + (w - dw) / 2, top + size * 1.08
        else:  # after: beside the word, at its height
            dh = size * (1.2 if kind == "down" else 0.5); dw = dh * vb[2] / vb[3]
            sx, sy = min(x + w + 24, W - dw - 20), top + size * (0.1 if kind == "down" else 0.35)
        start = max(0.0, round(last["start"] - line_start, 2))
        draws.append({"name": kind.capitalize(), "view_box": box, "x": round(sx), "y": round(sy), "w": round(dw), "h": round(dh),
                      "paths": [{"d": d, "stroke": items[group]["color"], "width": 7, "from": start, "to": round(start + length, 2)}]})
    return {"items": items, "draws": draws}

doc = mc("words.read")["document"]
times = {c["word_id"]: c["start"] for s in mc("words.studio_track")["sections"] for c in s["clips"]}
for anim in doc["animations"]:
    lines = {}
    for w in anim["words"]:
        lines.setdefault(w["line_id"], []).append(dict(w, start=times.get(w["word_id"], w["start"])))
    designs = {lid: design(ws, *LINES[lid], ws[0]["start"]) for lid, ws in lines.items()}
    saved = mc("words.save", f"animation_id={anim['animation_id']}", f"expected_source_revision={anim['source_revision']}",
               "origin=heyho-design.py", "style=" + json.dumps({"font": "Andika"}), "design=" + json.dumps(designs))
    applied = mc("words.apply", f"animation_id={anim['animation_id']}")
    print(anim["animation_id"], "rev", saved["animation"]["source_revision"], "lines", len(designs), "in video", applied["in_video"])
