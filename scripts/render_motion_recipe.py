#!/usr/bin/env python3
"""Run one layered-motion recipe through mc, retaining receipts for every attempt."""
import argparse
import fcntl
import hashlib
import json
from pathlib import Path
import subprocess
import sys
from datetime import datetime, timezone
from uuid import uuid4


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def write_json(path, value):
    temporary = path.with_suffix('.tmp')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    temporary.replace(path)


def inputs_for(command):
    params = command['params']
    spec_path = Path(params['spec']).resolve()
    spec = json.loads(spec_path.read_text())
    files = [spec_path]
    files += [(spec_path.parent / layer['file']).resolve() for layer in spec['layers']]
    if spec.get('reference'):
        files.append((spec_path.parent / spec['reference']).resolve())
    files = {str(path): digest(path) for path in sorted(set(files))}
    value = {'command': command, 'files': files}
    fingerprint = hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()
    return fingerprint, files


def reusable(entry, fingerprint):
    return (entry.get('inputs') == fingerprint and bool(entry.get('outputs'))
            and all(Path(path).is_file() and digest(path) == checksum
                    for path, checksum in entry['outputs'].items()))


def render(command, mc, receipts):
    fingerprint, files = inputs_for(command)
    run_id = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ-') + uuid4().hex[:8]
    folder = receipts / run_id
    folder.mkdir()
    actual = json.loads(json.dumps(command))
    actual['params']['out_dir'] = str(Path(command['params']['out_dir']).resolve() / run_id)
    receipt = {'status': 'running', 'inputs': fingerprint, 'input_files': files,
               'command': actual, 'outputs': {}}
    write_json(folder / 'receipt.json', receipt)
    print(json.dumps({'scene': Path(command['params']['spec']).stem,
                      'status': 'rendering', 'receipt': str(folder / 'receipt.json')}), flush=True)
    try:
        done = subprocess.run([mc, 'run', '-'], input=json.dumps(actual) + '\n',
                              capture_output=True, text=True)
        (folder / 'stdout.jsonl').write_text(done.stdout)
        (folder / 'stderr.log').write_text(done.stderr)
        lines = [json.loads(line) for line in done.stdout.splitlines() if line.strip()]
        result = lines[-1].get('result', {}) if lines else {}
        receipt.update(exit_code=done.returncode, result=result)
        if (done.returncode or not lines or not lines[-1].get('ok')
                or not result.get('ok') or not result.get('validation_ok')):
            raise ValueError('render failed or did not pass the motion gate; see stdout.jsonl/stderr.log')
        if inputs_for(command)[0] != fingerprint:
            raise ValueError('inputs changed during rendering; rerun this scene')
        artifacts = [item['file'] for item in result['outputs']]
        artifacts.append(result['project'])
        receipt['outputs'] = {path: digest(path) for path in artifacts}
        receipt['status'] = 'complete'
    except (OSError, ValueError, KeyError, IndexError) as error:
        receipt.update(status='failed', error=str(error))
    write_json(folder / 'receipt.json', receipt)
    return {**receipt, 'receipt': str(folder / 'receipt.json')}


def run(args):
    recipe = Path(args.recipe).resolve()
    commands = [json.loads(line) for line in recipe.read_text().splitlines()
                if line.strip() and not line.lstrip().startswith('#')]
    scenes = {}
    for command in commands:
        if command.get('id') != 'export.layered_motion':
            raise ValueError('render recipe accepts only export.layered_motion; project changes stay in their own step')
        scene = Path(command['params']['spec']).stem
        if scene in scenes:
            raise ValueError(f'duplicate scene: {scene}')
        scenes[scene] = command
    selected = args.scene or list(scenes)
    unknown = set(selected) - scenes.keys()
    if unknown:
        raise ValueError(f'unknown scenes: {sorted(unknown)}')
    receipts = recipe.parent / 'receipts'
    receipts.mkdir(exist_ok=True)
    with (receipts / '.lock').open('w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        current_path = receipts / 'current.json'
        current = json.loads(current_path.read_text()) if current_path.exists() else {}
        failed = 0
        for scene in selected:
            command = scenes[scene]
            fingerprint, _ = inputs_for(command)
            if not args.force and reusable(current.get(scene, {}), fingerprint):
                print(json.dumps({'scene': scene, 'status': 'reused',
                                  'receipt': current[scene]['receipt']}), flush=True)
                continue
            entry = render(command, args.mc, receipts)
            if entry['status'] == 'complete':
                current[scene] = {key: entry[key] for key in ('inputs', 'outputs', 'receipt')}
                write_json(current_path, current)
            else:
                failed += 1
            print(json.dumps({'scene': scene, 'status': entry['status'],
                              'receipt': entry['receipt'], 'outputs': entry['outputs'],
                              'error': entry.get('error')}), flush=True)
        return bool(failed)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('recipe', help='canonical JSONL containing export.layered_motion commands')
    parser.add_argument('--mc', default='mc', help='current mc executable')
    parser.add_argument('--scene', action='append', help='render only this spec filename stem; repeat for more scenes')
    parser.add_argument('--force', action='store_true', help='rerender even when current inputs and outputs match')
    args = parser.parse_args()
    try:
        return run(args)
    except (OSError, ValueError, KeyError) as error:
        parser.exit(1, f'{error}\n')


if __name__ == '__main__':
    sys.exit(main())
