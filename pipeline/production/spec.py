"""Production spec: chapters -> shots -> dialogue lines -> technique ids.

One JSON file drives voice generation, the FilmCraft timeline and the evidence
ledger. `import_script` builds the first version from a screenplay in the
"## NN · Title — mm:ss–mm:ss" / `SPEAKER, direction: “text”` format.
"""
from __future__ import annotations

import json
import re
import sys
from dataclasses import asdict, dataclass, field
from pathlib import Path


@dataclass
class Line:
    id: str
    speaker: str
    text: str
    direction: str = ""
    wav: str | None = None
    duration_s: float | None = None


@dataclass
class Shot:
    id: str
    duration_s: float
    kind: str = "board"
    source: str = ""
    lines: list[Line] = field(default_factory=list)
    techniques: list[str] = field(default_factory=list)


@dataclass
class Chapter:
    id: str
    title: str
    lesson: str
    start_s: float
    end_s: float
    shots: list[Shot] = field(default_factory=list)
    tools: list[str] = field(default_factory=list)


@dataclass
class Spec:
    title: str
    fps: int
    width: int
    height: int
    chapters: list[Chapter] = field(default_factory=list)


CHAPTER_RE = re.compile(r"^## (\d+) · (.+?) — (\d+):(\d+)–(\d+):(\d+)\s*$")
# SPEAKER[, direction]: “text” — the quote may continue over several lines.
LINE_RE = re.compile(r"^([A-Z][A-Z0-9 ]*?)(?:, ([^:]+))?:\s*(“.*)?$")
LESSON_RE = re.compile(r"[Ll]abel: \*\*(.+?)\*\*")
LEDGER_ROW_RE = re.compile(r"^\| (\d+)\. [^|]+\| (.+?) \| (.+?) \|\s*$")
TECH_RE = re.compile(r"\*\*(\d+)\*\*")


def import_script(text: str) -> list[Chapter]:
    chapters: list[Chapter] = []
    rows = text.splitlines()
    i = 0
    while i < len(rows):
        row = rows[i]
        m = CHAPTER_RE.match(row)
        if m:
            cid, title = m.group(1), m.group(2)
            start = int(m.group(3)) * 60 + int(m.group(4))
            end = int(m.group(5)) * 60 + int(m.group(6))
            chapters.append(Chapter(cid, title, "", float(start), float(end),
                                    [Shot(f"{cid}a", float(end - start))]))
            i += 1
            continue
        if chapters:
            chapter = chapters[-1]
            lesson = LESSON_RE.search(row)
            if lesson and not chapter.lesson:
                chapter.lesson = lesson.group(1)
            m = LINE_RE.match(row)
            if m:
                quote = m.group(3)
                if quote is None and i + 1 < len(rows) and rows[i + 1].startswith("“"):
                    i += 1
                    quote = rows[i]
                if quote is not None:
                    parts = [quote]
                    while not parts[-1].rstrip().endswith("”") and i + 1 < len(rows):
                        i += 1
                        parts.append(rows[i])
                    body = "\n".join(parts).strip()[1:-1]
                    shot = chapter.shots[0]
                    shot.lines.append(Line(f"{chapter.id}-{len(shot.lines) + 1:02d}",
                                           m.group(1).strip(), body, (m.group(2) or "").strip()))
        i += 1
    return chapters


def assign_techniques(chapters: list[Chapter], ledger_target: str) -> None:
    """Attach technique ids (T01…) and tools from the ledger target table, by chapter number."""
    by_number = {int(c.id): c for c in chapters}
    for row in ledger_target.splitlines():
        m = LEDGER_ROW_RE.match(row)
        if not m or int(m.group(1)) not in by_number:
            continue
        chapter = by_number[int(m.group(1))]
        chapter.shots[0].techniques = [f"T{int(n):02d}" for n in TECH_RE.findall(m.group(2))]
        chapter.tools = [t.strip() for t in m.group(3).split(",")]


def total_duration(spec: Spec) -> float:
    return sum(shot.duration_s for chapter in spec.chapters for shot in chapter.shots)


def save_spec(spec: Spec, path: str | Path) -> None:
    Path(path).write_text(json.dumps(asdict(spec), indent=2, ensure_ascii=False) + "\n")


def load_spec(path: str | Path) -> Spec:
    raw = json.loads(Path(path).read_text())
    chapters = []
    for c in raw.pop("chapters"):
        shots = [Shot(**{**s, "lines": [Line(**l) for l in s["lines"]]}) for s in c.pop("shots")]
        chapters.append(Chapter(**c, shots=shots))
    return Spec(**raw, chapters=chapters)


def main(argv: list[str]) -> int:
    """`python -m pipeline.production.spec production.json` prints a summary;
    `... --import SCRIPT.md LEDGER_TARGET.md --title T` (re)creates it."""
    path = Path(argv[0])
    if "--import" in argv:
        k = argv.index("--import")
        title = argv[argv.index("--title") + 1] if "--title" in argv else path.parent.name
        chapters = import_script(Path(argv[k + 1]).read_text())
        assign_techniques(chapters, Path(argv[k + 2]).read_text())
        save_spec(Spec(title, 30, 1920, 1080, chapters), path)
    spec = load_spec(path)
    shots = [s for c in spec.chapters for s in c.shots]
    techniques = {t for s in shots for t in s.techniques}
    lines = sum(len(s.lines) for s in shots)
    print(json.dumps({"chapters": len(spec.chapters), "shots": len(shots), "lines": lines,
                      "techniques": len(techniques), "duration_s": total_duration(spec)}))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
