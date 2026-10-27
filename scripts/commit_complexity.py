#!/usr/bin/env python3
"""Commit complexity analytics (lizard CLI).

Runs the `lizard` CLI over the staged changes and rates each commit:

  IMPROVING  - staged (recent) code complexity does not exceed the base
              (HEAD) complexity (sum of per-file cyclomatic-complexity
              deltas over changed functions is <= 0).
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


def lizard_ccns(content: bytes, suffix: str) -> list[float] | None:
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
        return ccns


def main() -> int:
    files = staged_changed_files()
    pairs: list[tuple[str, float, float]] = []
    added: list[tuple[str, float]] = []
    for path in files:
        after_list = lizard_ccns(git_bytes(":" + path) or b"", os.path.splitext(path)[1])
        before_list = lizard_ccns(git_bytes("HEAD:" + path) or b"", os.path.splitext(path)[1])
        if before_list is None:
            added.append((path, max(after_list) if after_list else 0.0))
        elif after_list is not None:
            pairs.append((path, sum(before_list), sum(after_list)))

    if pairs:
        delta = sum(after - before for _, before, after in pairs)
        rating = "IMPROVING" if delta <= 0 else "WORSE"
        lines = [f"complexity: rating={rating} changed_files={len(pairs)} ccn_delta={delta:.2f}"]
        for path, before, after in pairs:
            state = "IMPROVING" if after - before <= 0 else "WORSE"
            lines.append(f"  complexity: {path}: before_ccn_sum={before:.2f} after_ccn_sum={after:.2f} -> {state}")
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
