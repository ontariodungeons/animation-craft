"""Evidence ledger: a technique is earned only with a source on disk and an on-screen moment.

CSV columns: technique_id, technique, app, source, shot_id, timecode (HH:MM:SS[.ff]), status.
Only rows with status "earned" count. `timeline` maps shot_id -> {"start_s", "end_s"} as cut.
"""
from __future__ import annotations

import csv
import json
import sys
from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class LedgerReport:
    earned: int = 0
    tools: int = 0
    failures: list[str] = field(default_factory=list)


def parse_timecode(tc: str) -> float:
    h, m, s = tc.split(":")
    return int(h) * 3600 + int(m) * 60 + float(s)


def check_ledger(ledger_csv: str | Path, timeline: dict, root: str | Path) -> LedgerReport:
    report = LedgerReport()
    earned: set[str] = set()
    tools: set[str] = set()
    with Path(ledger_csv).open(newline="") as fh:
        for row in csv.DictReader(fh):
            if row["status"] != "earned":
                continue
            tid = row["technique_id"]
            if not (Path(root) / row["source"]).exists():
                report.failures.append(f"{tid}: source missing: {row['source']}")
                continue
            shot = timeline.get(row["shot_id"])
            if shot is None:
                report.failures.append(f"{tid}: shot {row['shot_id']} not in timeline")
                continue
            t = parse_timecode(row["timecode"])
            if not shot["start_s"] <= t <= shot["end_s"]:
                report.failures.append(
                    f"{tid}: timecode {row['timecode']} outside shot {row['shot_id']} "
                    f"({shot['start_s']:.2f}-{shot['end_s']:.2f}s)")
                continue
            earned.add(tid)
            tools.add(row["app"])
    report.earned, report.tools = len(earned), len(tools)
    return report


def main(argv: list[str]) -> int:
    """`python -m pipeline.production.ledger ledger.csv timeline.json [root]`"""
    timeline = json.loads(Path(argv[1]).read_text())
    root = argv[2] if len(argv) > 2 else Path(argv[0]).parent
    report = check_ledger(argv[0], timeline, root)
    print(json.dumps(report.__dict__, indent=2))
    return 1 if report.failures else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
