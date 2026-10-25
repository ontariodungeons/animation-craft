#!/usr/bin/env python3
"""Launch a foreground GUI process on its agent's workspace, without taking focus."""
import argparse
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys


def workspace(value):
    if not value.isascii() or not value.isdecimal() or not 3 <= int(value) <= 2147483647:
        raise argparse.ArgumentTypeError('agent workspace must be 3 or higher; 1 and 2 belong to Jesse')
    return int(value)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--workspace', required=True, type=workspace)
    parser.add_argument('--dry-run', action='store_true', help='print dispatch without launching anything')
    parser.add_argument('command', nargs=argparse.REMAINDER, help='-- executable [arguments]')
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ['--'] else args.command
    if not command:
        parser.error('provide -- executable [arguments]')
    executable = shutil.which(command[0])
    if not executable:
        parser.error(f'executable not found: {command[0]}')
    command[0] = os.path.abspath(executable)
    context = [f'{key}={os.environ[key]}' for key in ('HERDR_WORKSPACE_ID', 'HERDR_TAB_ID', 'HERDR_PANE_ID') if os.environ.get(key)]
    launch = ['env', *context, *command] if context else command
    shell = f'cd -- {shlex.quote(str(Path.cwd()))} && exec {shlex.join(launch)}'
    # Lua long strings preserve shell quoting without interpreting escapes or embedded delimiters.
    equals = '='
    while f']{equals}]' in shell:
        equals += '='
    literal = f'[{equals}[{shell}]{equals}]'
    dispatch = f'hl.dsp.exec_cmd({literal}, {{workspace = "{args.workspace} silent", no_initial_focus = true, render_unfocused = true, focus_on_activate = false}})'
    result = {'workspace': args.workspace, 'command': command, 'dispatch': dispatch, 'submitted': False}
    if not args.dry_run:
        try:
            reply = subprocess.run(['hyprctl', 'dispatch', dispatch], capture_output=True, text=True)
        except OSError as error:
            print(error, file=sys.stderr)
            return 1
        if reply.returncode:
            print(reply.stderr or reply.stdout, file=sys.stderr, end='')
            return reply.returncode
        result.update(submitted=True, reply=reply.stdout.strip())
    print(json.dumps(result))
    return 0


if __name__ == '__main__':
    sys.exit(main())
