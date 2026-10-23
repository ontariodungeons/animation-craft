//! Real CLI/process-boundary tests; the fake OpenCode never authenticates.
#[test]
#[cfg(unix)]
fn login_handoff_preserves_terminal_arguments_and_exit() {
    let result = std::process::Command::new("/usr/bin/python3")
        .args(["-c", CHECK, env!("CARGO_BIN_EXE_mc")])
        .output().unwrap();
    assert!(result.status.success(), "{}{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
}

#[cfg(unix)]
const CHECK: &str = r#"
import errno, json, os, pathlib, pty, select, subprocess, sys, tempfile, time
cli = str(pathlib.Path(sys.argv[1]).resolve())
with tempfile.TemporaryDirectory(prefix='ac-login-') as tmp:
    root = pathlib.Path(tmp)
    tool = root / 'opencode'
    tool.write_text('''#!/usr/bin/python3
import json, os, pathlib, sys
pathlib.Path(os.environ['CALLS']).write_text(json.dumps(sys.argv[1:]))
assert all(os.isatty(fd) for fd in (0,1,2)), 'terminal streams were captured'
os.write(1, b'NATIVE_OUT\\n')
os.write(2, b'NATIVE_ERR\\n')
sys.exit(int(os.environ.get('NATIVE_EXIT','0')))
''')
    tool.chmod(0o700)
    env = dict(os.environ, PATH=tmp, CALLS=str(root/'calls'))
    denied = subprocess.run([cli, 'login', 'openrouter'], env=env, capture_output=True, text=True)
    assert denied.returncode == 1 and 'interactive terminal' in denied.stderr, denied
    assert not (root/'calls').exists(), 'backend launched without terminal'

    def terminal(args, code=0, missing=False):
        (root/'calls').unlink(missing_ok=True)
        master, slave = pty.openpty()
        e = dict(env, NATIVE_EXIT=str(code))
        if missing: e['PATH'] = str(root/'missing')
        p = subprocess.Popen([cli]+args, env=e, stdin=slave, stdout=slave, stderr=slave)
        os.close(slave)
        data = bytearray()
        deadline = time.monotonic()+10
        try:
            while True:
                assert time.monotonic() < deadline, 'login process timed out'
                if not select.select([master], [], [], .1)[0]: continue
                try: chunk=os.read(master, 8192)
                except OSError as ex:
                    if ex.errno == errno.EIO: break
                    raise
                if not chunk: break
                data.extend(chunk)
            result=p.wait(timeout=2)
        finally:
            os.close(master)
            if p.poll() is None: p.kill(); p.wait()
        calls=json.loads((root/'calls').read_text()) if (root/'calls').exists() else None
        return result, data.decode(), calls

    for args, expected, code in [(['login','openrouter'],['auth','login','openrouter'],0),
                                  (['login'],['auth','login'],0),
                                  (['login','openrouter'],['auth','login','openrouter'],23)]:
        actual, output, calls=terminal(args,code)
        assert actual==code and calls==expected, (actual,output,calls)
        assert 'NATIVE_OUT' in output and 'NATIVE_ERR' in output, output
    for args in [['login','openrouter','extra'], ['--bridge','19897','login','openrouter']]:
        actual,output,calls=terminal(args)
        assert actual==2 and calls is None, (actual,output,calls)
    actual,output,calls=terminal(['login','openrouter'],missing=True)
    assert actual==1 and calls is None and 'OpenCode' in output, (actual,output,calls)
print('LOGIN_HANDOFF_ASSERTIONS_COMPLETED')
"#;
