"""The `magic` entry point opens the real native application, without terminal UI."""
import os
import sys
from cli_anything.magic.utils.magic_backend import find_magicstudio_shell


def main() -> None:
    try:
        shell = find_magicstudio_shell()
        os.execv(shell, [shell, *sys.argv[1:]])
    except (OSError, RuntimeError) as exc:
        print(f"magic: {exc}", file=sys.stderr)
        raise SystemExit(1)
