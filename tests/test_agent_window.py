"""Generic launcher boundary checks; the compositor and GUI are never invoked."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

LAUNCHER = Path(__file__).resolve().parents[1] / 'scripts/agent_window.py'


class AgentWindowTests(unittest.TestCase):
    def run_launcher(self, workspace, *args, reply='ok', code=0):
        with tempfile.TemporaryDirectory() as root:
            stub = Path(root) / 'hyprctl'
            stub.write_text('#!' + sys.executable + '\nimport json,sys\n'
                            f'print(json.dumps(sys.argv[1:]))\nprint({reply!r})\nsys.exit({code})\n')
            stub.chmod(0o700)
            return subprocess.run([sys.executable, str(LAUNCHER), '--workspace', str(workspace),
                                   '--', sys.executable, *args], capture_output=True, text=True,
                                  env={**os.environ, 'HERDR_PANE_ID': 'wTest:p1', 'HERDR_TAB_ID': 'wTest:t1', 'HERDR_WORKSPACE_ID': 'wTest', 'PATH': root + os.pathsep + os.environ['PATH']})

    def test_reserved_workspaces_never_dispatch(self):
        for workspace in ['1', '2', '0', '-1', '3 silent; bad', '2147483648']:
            result = self.run_launcher(workspace)
            self.assertNotEqual(result.returncode, 0)
            self.assertNotIn('dispatch', result.stdout)
            self.assertIn('workspace', result.stderr.lower())

    def test_silent_owned_workspace_and_literal_arguments(self):
        literal = "art 'with spaces'; $(touch /tmp/never-run) ]=]"
        result = self.run_launcher(3, literal)
        self.assertEqual(result.returncode, 0, result.stderr)
        value = json.loads(result.stdout)
        self.assertTrue(value['submitted'])
        dispatched = json.loads(value['reply'].splitlines()[0])
        self.assertEqual(dispatched[0], 'dispatch')
        self.assertIn('workspace = "3 silent"', dispatched[1])
        self.assertIn('no_initial_focus = true', dispatched[1])
        self.assertIn('render_unfocused = true', dispatched[1])
        self.assertIn('focus_on_activate = false', dispatched[1])
        # Decode the generated Lua long string, then the actual POSIX command arguments.
        import re
        import shlex
        match = re.search(r'exec_cmd\(\[(=*)\[(.*)\]\1\],', dispatched[1], re.S)
        self.assertIsNotNone(match)
        shell = match[2].split(' && exec ', 1)[1]
        self.assertEqual(shlex.split(shell), ['env', 'HERDR_WORKSPACE_ID=wTest', 'HERDR_TAB_ID=wTest:t1', 'HERDR_PANE_ID=wTest:p1', sys.executable, literal])
        self.assertEqual(value['workspace'], 3)

    def test_dispatch_failure_never_launches_a_fallback(self):
        result = self.run_launcher(4, code=7, reply='compositor refused')
        self.assertEqual(result.returncode, 7)
        self.assertNotIn('"submitted": true', result.stdout)
        self.assertIn('compositor refused', result.stderr)


if __name__ == '__main__':
    unittest.main()
