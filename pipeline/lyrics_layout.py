#!/usr/bin/env python3
"""Design every sung line from a project's lyric recipe (`lyrics-design.json`).

The recipe holds the art direction (palette, where the lyrics sit, which words are heroes and how they
act); nothing about a song lives here. Each line becomes groups of consecutive words: hero words in their
own lettering and move, the rest in the base lettering. Rows are laid out from measured text (the font
files the video renders with, found through fontconfig), centred, split into two rows or scaled down so
nothing leaves the frame. A hero with a stroke draws it on under or beside itself on its sung beat.

  lyrics_layout.py --recipe R --input I        prints {"designs": {animation_id: {line_id: design}}}
  lyrics_layout.py --recipe R --project PROJECT --magic-cli CLI [--apply]
                                               reads, saves (and applies) through a running Studio
"""
import argparse, json, re, subprocess, sys
from functools import lru_cache
from PIL import ImageFont

STROKES = {  # hand-drawn strokes: view box, path, draw-on seconds
    "underline": ("0 0 280 30", "M 5 14 Q 61 5 125 13 T 269 10", 0.55),
    "swoop": ("0 0 790 160", "M 10 145 Q 126 170 217 93 T 438 58 Q 578 29 756 7 M 730 5 L 761 5 L 746 29", 1.6),
    "loop": ("0 0 620 110", "M 15 20 C 110 95 425 95 455 40 C 485 -4 350 -3 352 37 C 355 95 515 85 602 50 M 579 33 L 608 49 L 586 73", 1.6),
    "hill": ("0 0 410 95", "M 7 77 Q 73 72 118 32 Q 171 -8 228 31 Q 279 75 400 76", 1.4),
    "down": ("0 0 160 330", "M 73 13 C 27 67 104 129 66 196 Q 47 231 61 289 M 30 264 L 61 294 L 87 260", 1.0),
}
LETTERING = {"song-bold": ("song", 700, False), "serif-italic": ("serif", 400, True), "serif-bold": ("serif", 700, False),
             "sans": ("sans", 400, False), "sans-bold": ("sans", 700, False)}
BANDS = {"top": ([150], [135, 270]), "bottom": ([985], [865, 990])}  # baselines: one row, two rows
GAP = 0.24  # gap between groups, in em of the larger neighbour


@lru_cache(None)
def font_file(font, weight, italic, song_family):
    family = {"serif": "serif", "sans": "sans-serif"}.get(font) or song_family or "sans-serif"
    pattern = family + (":bold" if weight >= 700 else "") + (":italic" if italic else "")
    return subprocess.run(["fc-match", "-f", "%{file}", pattern], capture_output=True, text=True, check=True).stdout


def width(text, lk, song_family):
    extra = lk["size"] * ((0.07 if lk.get("outline") else 0) + (0.12 if lk["italic"] else 0))
    face = ImageFont.truetype(font_file(lk["font"], lk["weight"], lk["italic"], song_family), round(lk["size"]))
    return face.getlength(text) + extra


def key(text):
    return re.sub(r"[^a-z]", "", text.lower())


def look(recipe, spec):
    pal = recipe.get("palette", {})
    colour = lambda c: pal.get(c, c) if c else None
    font, weight, italic = LETTERING[spec.get("lettering", "sans")]
    return {"font": font, "weight": weight, "italic": italic, "size": spec.get("size", 56), "color": colour(spec.get("color", "#000000")),
            "outline": colour(spec.get("outline")), "motion": spec.get("motion", "quiet")}


def groups(recipe, words):
    """Consecutive base words together; each hero (repeats of one hero merged) on its own; at most `max_heroes` per line."""
    heroes, budget, out = recipe.get("heroes", {}), recipe.get("max_heroes", 2), []
    for w in words:
        hero = heroes.get(key(w["text"]))
        if hero and budget and not (out and out[-1][2] == key(w["text"])):
            budget -= 1
            out.append([[w], hero, key(w["text"])])
        elif hero and out and out[-1][2] == key(w["text"]):
            out[-1][0].append(w)
        elif out and out[-1][2] is None:
            out[-1][0].append(w)
        else:
            out.append([[w], None, None])
    return [(ws, look(recipe, hero or recipe["base"]), hero) for ws, hero, _ in out]


def design(recipe, words, band):
    W, H = recipe.get("frame", [1920, 1080]); margin = recipe.get("margin", 110); song = recipe.get("font")
    placed = groups(recipe, words)
    text = lambda ws: " ".join(w["text"] for w in ws)
    def fit(row):
        def total(scale):
            ws = [width(text(g), {**lk, "size": lk["size"] * scale}, song) for g, lk, _ in row]
            gaps = [GAP * max(a[1]["size"], b[1]["size"]) * scale for a, b in zip(row, row[1:])]
            return ws, gaps
        scale = 1.0
        while sum(map(sum, total(scale))) > W - 2 * margin: scale *= 0.96
        return scale, total(scale)
    scale, _ = fit(placed)
    rows = [placed]
    if scale < 0.85 and len(placed) > 1:  # two rows, split where the widths balance
        full = [width(text(g), lk, song) for g, lk, _ in placed]
        cut = min(range(1, len(placed)), key=lambda k: abs(sum(full[:k]) - sum(full[k:])))
        rows = [placed[:cut], placed[cut:]]
    baselines = BANDS[band][len(rows) - 1]
    items, boxes = [], []
    for baseline, row in zip(baselines, rows):
        scale, (ws, gaps) = fit(row)
        x = (W - sum(ws) - sum(gaps)) / 2
        for k, ((g, lk, hero), w) in enumerate(zip(row, ws)):
            size = round(lk["size"] * scale); top = baseline - size * 0.92
            items.append({**lk, "size": size, "word_ids": [w_["word_id"] for w_ in g], "x": round(x), "y": round(top), "rotate": 0,
                          "accent": g[-1]["word_id"] if lk["motion"] != "quiet" else None})
            boxes.append((x, top, w, size, g[-1], hero))
            x += w + (gaps[k] if k < len(gaps) else 0)
    draws = []
    for i, (x, top, w, size, last, hero) in enumerate(boxes):
        if not hero or not hero.get("stroke") or draws: continue  # one stroke per line, from its first hero that has one
        box, d, length = STROKES[hero["stroke"]]
        vb = [float(n) for n in box.split()]
        if hero["stroke"] == "down":  # beside the word, at its height
            dh = size * 1.2; dw = dh * vb[2] / vb[3]; sx, sy = min(x + w + 24, W - dw - 20), top + size * 0.1
        else:  # under the word
            dw = max(w, 160); dh = dw * vb[3] / vb[2]; sx, sy = x + (w - dw) / 2, top + size * 1.08
        sung = [w["start"] for w in words if w.get("start") is not None]  # a printed word the singer left out has no time
        start = max(0.0, round((last["start"] if last.get("start") is not None else sung[0]) - sung[0], 2)) if sung else 0.0
        draws.append({"name": hero["stroke"].capitalize(), "view_box": box, "x": round(sx), "y": round(min(sy, H - dh - 10)), "w": round(dw), "h": round(dh),
                      "paths": [{"d": d, "stroke": items[i]["color"], "width": 7, "from": start, "to": round(start + length, 2)}]})
    return {"items": items, "draws": draws}


def designs(recipe, animations):
    """{animation_id: {line_id: design}} for every sung line; `lines` in the recipe override a line's band."""
    out = {}
    for anim in animations:
        band = recipe.get("bands", {}).get(anim.get("section"), recipe.get("band", "top"))
        lines = {}
        for w in anim["words"]:
            lines.setdefault(w["line_id"], []).append(w)
        out[anim["animation_id"]] = {lid: design(recipe, ws, recipe.get("lines", {}).get(lid, {}).get("band", band)) for lid, ws in lines.items()}
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--recipe", required=True); ap.add_argument("--input"); ap.add_argument("--project"); ap.add_argument("--magic-cli", default="magic-cli")
    ap.add_argument("--apply", action="store_true")
    a = ap.parse_args()
    recipe = json.load(open(a.recipe))
    if a.input:
        print(json.dumps({"designs": designs(recipe, json.load(open(a.input))["animations"])}))
        return
    mc = lambda *args: json.loads(subprocess.run([a.magic_cli, *(["--project", a.project] if a.project else []), "exec", *args], capture_output=True, text=True, check=True).stdout)
    doc = mc("words.read")["document"]
    times = {c["word_id"]: c["start"] for s in mc("words.studio_track")["sections"] for c in s["clips"]}
    anims = [{**x, "section": x["scope"]["section"], "words": [dict(w, start=times.get(w["word_id"], w["start"])) for w in x["words"]]} for x in doc["animations"]]
    for anim_id, d in designs(recipe, anims).items():
        anim = next(x for x in doc["animations"] if x["animation_id"] == anim_id)
        saved = mc("words.save", f"animation_id={anim_id}", f"expected_source_revision={anim['source_revision']}", "origin=lyrics_layout.py",
                   "style=" + json.dumps({"font": recipe.get("font")} if recipe.get("font") else {}), "design=" + json.dumps(d))
        applied = mc("words.apply", f"animation_id={anim_id}") if a.apply else {}
        print(json.dumps({"animation_id": anim_id, "revision": saved["animation"]["source_revision"], "lines": len(d), "in_video": applied.get("in_video")}))


if __name__ == "__main__":
    main()
