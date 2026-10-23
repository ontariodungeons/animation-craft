from __future__ import annotations

import csv
import tempfile
import unittest
from pathlib import Path

from pipeline.production import ledger

TIMELINE = {"01a": {"start_s": 0.0, "end_s": 20.0}, "02a": {"start_s": 20.0, "end_s": 35.0}}
FIELDS = ["technique_id", "technique", "app", "source", "shot_id", "timecode", "status"]


class TestLedger(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        (self.root / "stage.blend").write_bytes(b"x")
        (self.root / "mix.scraft").write_bytes(b"x")

    def _write(self, rows: list[dict]) -> Path:
        path = self.root / "ledger.csv"
        with path.open("w", newline="") as fh:
            writer = csv.DictWriter(fh, fieldnames=FIELDS)
            writer.writeheader()
            for row in rows:
                writer.writerow({"status": "earned", **row})
        return path

    def _row(self, tid: str, app: str = "Blender", source: str = "stage.blend",
             shot: str = "01a", tc: str = "00:00:05") -> dict:
        return {"technique_id": tid, "technique": tid, "app": app, "source": source,
                "shot_id": shot, "timecode": tc}

    def test_counts_earned_rows(self) -> None:
        path = self._write([self._row("T01"), self._row("T02", app="SoundCraft", source="mix.scraft",
                                                         shot="02a", tc="00:00:21")])
        report = ledger.check_ledger(path, TIMELINE, root=self.root)
        self.assertEqual((report.earned, report.tools, report.failures), (2, 2, []))

    def test_fails_missing_source(self) -> None:
        report = ledger.check_ledger(self._write([self._row("T01", source="nope.blend")]),
                                     TIMELINE, root=self.root)
        self.assertEqual(report.earned, 0)
        self.assertIn("T01: source missing: nope.blend", report.failures)

    def test_fails_timecode_outside_shot(self) -> None:
        report = ledger.check_ledger(self._write([self._row("T01", tc="00:00:25")]),
                                     TIMELINE, root=self.root)
        self.assertEqual(report.earned, 0)
        self.assertIn("T01: timecode 00:00:25 outside shot 01a (0.00-20.00s)", report.failures)

    def test_fails_shot_cut_from_timeline(self) -> None:
        report = ledger.check_ledger(self._write([self._row("T01", shot="99z")]),
                                     TIMELINE, root=self.root)
        self.assertIn("T01: shot 99z not in timeline", report.failures)

    def test_reused_technique_counts_once(self) -> None:
        path = self._write([self._row("T01"), self._row("T01", shot="02a", tc="00:00:30")])
        self.assertEqual(ledger.check_ledger(path, TIMELINE, root=self.root).earned, 1)

    def test_planned_rows_do_not_count(self) -> None:
        path = self._write([{**self._row("T01"), "status": "planned"}])
        report = ledger.check_ledger(path, TIMELINE, root=self.root)
        self.assertEqual((report.earned, report.tools, report.failures), (0, 0, []))


if __name__ == "__main__":
    unittest.main()
