#!/usr/bin/env python3
"""Long file check for staged changes.

A file over the line limit is the thing that actually costs time: an agent has
to read all of it before it can change any of it. This check fails the commit
when the change pushes a file past the limit, and shouts about files that were
already past it, so the debt is visible instead of invisible.

Limits live in code-limits.json at the repository root:

  {"max_file_lines": 2000, "max_function_ccn": 15}

Exit codes: 0 = no new violation, 1 = this commit created one.
"""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LIMITS_FILE = ROOT / "code-limits.json"
DEFAULT_MAX_FILE_LINES = 2000


def limits() -> dict:
    try:
        return json.loads(LIMITS_FILE.read_text())
    except Exception:
        return {}


def git_text(*args: str) -> str:
    result = subprocess.run(["git", *args], capture_output=True)
    if result.returncode != 0:
        return ""
    return result.stdout.decode("utf-8", "replace")


def git_bytes(spec: str) -> bytes | None:
    result = subprocess.run(["git", "show", spec], capture_output=True)
    if result.returncode != 0:
        return None
    return result.stdout


def count_lines(content: bytes | None) -> int:
    if not content:
        return 0
    lines = content.count(b"\n")
    if content and not content.endswith(b"\n"):
        lines += 1
    return lines


def staged_files() -> list[str]:
    return [name.strip() for name in git_text("diff", "--cached", "--name-only", "--diff-filter=ACMR").splitlines() if name.strip()]


def main() -> int:
    max_lines = int(limits().get("max_file_lines", DEFAULT_MAX_FILE_LINES))
    failures: list[str] = []
    warnings: list[str] = []
    for path in staged_files():
        after = count_lines(git_bytes(":" + path))
        before = count_lines(git_bytes("HEAD:" + path))
        if after <= max_lines:
            continue
        if before > max_lines:
            warnings.append(f"FIX THIS (already over): {path} is {after} lines (limit {max_lines})")
        else:
            failures.append(f"FIX THIS: {path} is {after} lines (limit {max_lines})")
    for line in warnings:
        print(f"long file: {line}")
    for line in failures:
        print(f"long file: {line}")
    if failures:
        print(
            f"long file: FAILED - {len(failures)} file(s) over the {max_lines} line limit. "
            "Split the file (one domain per module) before committing, or raise "
            "max_file_lines in code-limits.json if the size is deliberate."
        )
        return 1
    print(f"long file: PASSED (limit {max_lines} lines)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
