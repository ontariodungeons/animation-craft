"""Locate the Craft CLI binaries (photocraft-cli, vectorcraft-cli, ...).

Search order: the tool's own environment variable (PHOTOCRAFT_CLI for photocraft-cli),
then CRAFT_CLI_DIR, then PATH. Nothing else is guessed.
"""
import os
import shutil
from pathlib import Path
from typing import Optional


class CraftCliNotFound(FileNotFoundError):
    pass


def env_var_for(name: str) -> str:
    return name.upper().replace("-", "_")


def find_craft_cli(name: str) -> Optional[str]:
    explicit = os.environ.get(env_var_for(name))
    if explicit:
        return explicit if Path(explicit).is_file() else None
    directory = os.environ.get("CRAFT_CLI_DIR")
    if directory:
        candidate = Path(directory) / name
        if candidate.is_file():
            return str(candidate)
    return shutil.which(name)


def require_craft_cli(name: str) -> str:
    found = find_craft_cli(name)
    if found:
        return found
    var = env_var_for(name)
    raise CraftCliNotFound(
        f"`{name}` not found. Set {var} to its path, set CRAFT_CLI_DIR to the folder holding it, or put it on PATH."
    )
