#!/usr/bin/env python3
"""Apply a reviewed PhotoCraft command recipe to any supported image.

All editing is performed by PhotoCraft. This runner makes no assumptions about
characters, anatomy, colors, dimensions, number of parts, or sheet layout.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('recipe', type=Path, help='JSON array of PhotoCraft command/params objects')
    parser.add_argument('--out-dir', required=True, type=Path, help='New directory; existing outputs are never overwritten')
    args = parser.parse_args()
    try:
        source = args.source.resolve(strict=True)
        steps = json.loads(args.recipe.read_text())
        if not isinstance(steps, list) or not steps:
            raise ValueError('Recipe must contain at least one explicit command')
        for step in steps:
            if not isinstance(step, dict) or not isinstance(step.get('command'), str) or not isinstance(step.get('params', {}), dict):
                raise ValueError('Every step needs a command string and params object')
        binary = os.environ.get('PHOTOCRAFT_CLI') or shutil.which('photocraft-cli')
        if not binary:
            raise ValueError('PhotoCraft CLI not found; set PHOTOCRAFT_CLI or install via artcraft-launcher')
        args.out_dir.mkdir(parents=True, exist_ok=False)
        command = [binary, 'run', str(source)]
        for step in steps:
            command += ['--cmd', step['command'], '--params', json.dumps(step.get('params', {}))]
        project = args.out_dir / 'asset.pcraft'
        command += ['--out', str(project)]
        (args.out_dir / 'recipe.json').write_text(json.dumps(steps, indent=2))
        result = subprocess.run(command, capture_output=True, text=True, timeout=120)
        (args.out_dir / 'commands.jsonl').write_text(result.stdout)
        (args.out_dir / 'stderr.txt').write_text(result.stderr)
        if result.returncode:
            raise RuntimeError(result.stderr.strip() or 'PhotoCraft command failed')
        for line in result.stdout.splitlines():
            if line.startswith('{'):
                row = json.loads(line)
                payload = row.get('result')
                if row.get('error') or (isinstance(payload, dict) and payload.get('ok') is False):
                    raise RuntimeError(f"PhotoCraft operation failed: {row.get('command')}")
        png = args.out_dir / 'asset.png'
        result = subprocess.run([binary, 'convert', str(project), str(png)], capture_output=True, text=True, timeout=120)
        if result.returncode or not png.is_file():
            raise RuntimeError(result.stderr.strip() or 'PNG export was not created')
        print(json.dumps({'project': str(project.resolve()), 'image': str(png.resolve()), 'review_required': True}))
    except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as error:
        parser.exit(1, f'{error}\n')


if __name__ == '__main__':
    main()
