#!/usr/bin/env python3
"""Expand shared layer references into the standalone specs used by Studio and mc."""
import argparse
from copy import deepcopy
import json
from pathlib import Path


def compose(scene, library):
    result = deepcopy(scene)
    layers = []
    for reference in scene['layers']:
        key = reference if isinstance(reference, str) else reference['use']
        variant, layer_id = key.split('/', 1)
        layer = {'id': layer_id, **deepcopy(library['variants'][variant][layer_id])}
        for name, value in (reference.get('override', {}) if isinstance(reference, dict) else {}).items():
            if value is None:
                layer.pop(name, None)
            else:
                layer[name] = value
        layers.append(layer)
    result['layers'] = layers
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('animation', type=Path, help='project animation directory with source/ and specs/')
    args = parser.parse_args()
    source = args.animation / 'source'
    library = json.loads((source / 'layers.json').read_text())
    destination = args.animation / 'specs'
    destination.mkdir(exist_ok=True)
    for path in sorted((source / 'scenes').glob('*.json')):
        result = compose(json.loads(path.read_text()), library)
        target = destination / path.name
        # Keep matching files untouched so open Studio editors retain their revision.
        if target.exists() and json.loads(target.read_text()) == result:
            continue
        temporary = target.with_suffix('.tmp')
        temporary.write_text(json.dumps(result, indent=1) + '\n')
        temporary.replace(target)
        print(target)


if __name__ == '__main__':
    main()
