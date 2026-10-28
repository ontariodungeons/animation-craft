#!/usr/bin/env python3
"""Install this built Magic Studio checkout's GUI/CLI and desktop entry locally.

Prepare dependencies with: uv pip install --python .venv/bin/python -e agent-harness
Build with: cargo build -p magicstudio-shell
This installs Magic only; it never installs or configures other Craft apps.
"""
import argparse
import os
from pathlib import Path
import shutil


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prefix', type=Path, default=Path.home() / '.local')
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    build = root / 'target' / args.profile
    shell = build / 'magicstudio-shell'
    python = root / '.venv/bin/python'
    if not shell.is_file() or not python.is_file():
        parser.error('Build the native shell and prepare the project virtual environment first')
    destination = args.prefix / 'share/magic-studio/bin'
    destination.mkdir(parents=True, exist_ok=True)
    # Replace the directory entry, not the running executable's inode.
    staged = destination / 'magicstudio-shell.new'
    shutil.copy2(shell, staged)
    staged.replace(destination / 'magicstudio-shell')
    for name, module in [('magic', 'gui'), ('magic-cli', 'magic_cli')]:
        entry = destination / name
        staged = destination / (name + '.new')
        staged.write_text(f'#!{python}\nfrom cli_anything.magic.{module} import main\nmain()\n')
        staged.chmod(0o755)
        staged.replace(entry)
        for link in [args.prefix / 'bin' / name, build / name]:
            link.parent.mkdir(parents=True, exist_ok=True)
            if link.is_symlink():
                link.unlink()
            elif link.exists():
                raise RuntimeError(f'Refusing to replace an unrelated regular executable: {link}')
            link.symlink_to(entry)
    shell_link = args.prefix / 'bin/magicstudio-shell'
    if not shell_link.exists() and not shell_link.is_symlink():
        shell_link.symlink_to(destination / 'magicstudio-shell')
    desktop = args.prefix / 'share/applications/ai.storyteller.magicstudio.desktop'
    desktop.parent.mkdir(parents=True, exist_ok=True)
    icon = args.prefix / 'share/icons/hicolor/256x256/apps/ai.storyteller.magicstudio.png'
    icon.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(root / 'apps/magicstudio-shell/icons/icon.png', icon)
    template = (root / 'apps/magicstudio-shell/magic.desktop').read_text()
    desktop.write_text(template.replace('Exec=magic %f', f'Exec="{args.prefix / "bin/magic"}" %f'))
    print(f'Installed Magic GUI, CLI and desktop entry under {args.prefix}')


if __name__ == '__main__':
    main()
