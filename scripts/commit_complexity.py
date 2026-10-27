#!/usr/bin/env python3
"""Commit complexity analytics (lizard CLI).

Runs the `lizard` CLI over the staged changes and rates each commit:

  IMPROVING  - staged (recent) code complexity does not exceed the base
              (HEAD) complexity for the same files (average CCN stayed flat
              or dropped).
  WORSE      - staged code complexity is higher than the base complexity.

Added files (no HEAD baseline) are rated against lizard's default cyclomatic
complexity threshold (CCN > 15 counts as worse).

Informational: prints the rating; exits 0 on a completed run.
"""
from __future__ import annotations

import os
import re
import subprocess
import tempfile

import xml.etree.ElementTree as ET

LIZARD = "lizard"
CCN_THRESHOLD = 15.0
SUPPORTED = re.compile(
    r"\.(py|js|jsx|ts|tsx|java|c|cc|cpp|cs|go|rb|php|swift|scala|lua|rs|rust|fortran|kotlin|sol|erl|zig|vue|perl|st|r|plsql|objc|ttcn)$",
    re.IGNORECASE,
)


def git_bytes(spec: str) -> bytes | None:
    result = subprocess.run(["git", "show", spec], capture_output=True)
    if result.returncode != 0:
        return None
    return result.stdout


def git_text(*args: str) -> str:
    result = subprocess.run(["git", *args], capture_output=True)
    if result.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)} failed: {result.stderr.decode('utf-8', 'replace')}")
    return result.stdout.decode("utf-8", "replace")


def staged_changed_files() -> list[str]:
    out = git_text("diff", "--cached", "--name-only", "--diff-filter=ACMR")
    files: list[str] = []
    for line in out.splitlines():
        name = line.strip()
        if name and SUPPORTED.search(name):
            files.append(name)
    return files


def lizard_avg_ccn(content: bytes, suffix: str) -> float | None:
    with tempfile.TemporaryDirectory() as tmp:
        target = os.path.join(tmp, "lizard_target" + suffix)
        with open(target, "wb") as handle:
            handle.write(content)
        result = subprocess.run([LIZARD, "--xml", target], capture_output=True, text=True)
        if result.returncode != 0 or not result.stdout.strip():
            return None
        root = ET.fromstring(result.stdout)
        ccns: list[float] = []
        for measure in root.iter("measure"):
            if (measure.get("type") or "") != "Function":
                continue
            for item in measure.findall("item"):
                values = [(value.text or "").strip() for value in item.findall("value")]
                if len(values) < 3:
                    continue
                ccns.append(float(values[2]))
        if not ccns:
            return None
        return sum(ccns) / len(ccns)


def main() -> int:
    files = staged_changed_files()
    pairs: list[tuple[str, float, float]] = []
    added: list[tuple[str, float]] = []
    for path in files:
        after = lizard_avg_ccn(git_bytes(":" + path) or b"", os.path.splitext(path)[1])
        before = lizard_avg_ccn(git_bytes("HEAD:" + path) or b"", os.path.splitext(path)[1])
        if before is None:
            added.append((path, after or 0.0))
        else:
            pairs.append((path, before, after or 0.0))

    if pairs:
        before_avg = sum(pair[1] for pair in pairs) / len(pairs)
        after_avg = sum(pair[2] for pair in pairs) / len(pairs)
        rating = "IMPROVING" if after_avg <= before_avg else "WORSE"
        lines = [f"complexity: rating={rating} changed_files={len(pairs)} avg_ccn_before={before_avg:.2f} avg_ccn_after={after_avg:.2f}"]
        for path, before_avg_ccn, after_avg_ccn in pairs:
            state = "IMPROVING" if after_avg_ccn <= before_avg_ccn else "WORSE"
            lines.append(f"  complexity: {path}: before_ccn={before_avg_ccn:.2f} after_ccn={after_avg_ccn:.2f} -> {state}")
    elif added:
        worst = max((value for _, value in added), default=0.0)
        rating = "IMPROVING" if worst <= CCN_THRESHOLD else "WORSE"
        lines = [f"complexity: rating={rating} added_files={len(added)} max_ccn={worst:.2f} (threshold={CCN_THRESHOLD})"]
    else:
        lines = ["complexity: rating=IMPROVING changed_files=0 (no analyzable staged files)"]

    for line in lines:
        print(line)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
