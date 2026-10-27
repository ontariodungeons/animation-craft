//! Explicit CLI and MCP observations must stay on their selected Studio bridge.
#[test]
fn bridge_timeout_uses_only_the_canonical_magicstudio_override() {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    for (variable, should_succeed) in [("MAGICSTUDIO_BRIDGE_TIMEOUT_MS", false), ("ANIMATIONCRAFT_BRIDGE_TIMEOUT_MS", true)] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();
            let mut request = String::new();
            BufReader::new(&stream).read_line(&mut request).unwrap();
            assert!(request.contains("project.info"));
            std::thread::sleep(std::time::Duration::from_millis(300));
            // A timed-out client is expected to have closed its socket.
            let _ = writeln!(stream, "{}", serde_json::json!({"ok":true,"result":{"observed":true}}));
        });
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_mc"))
            .args(["--bridge", &address.to_string(), "exec", "project.info"])
            .env_remove("MAGICSTUDIO_BRIDGE_TIMEOUT_MS")
            .env_remove("ANIMATIONCRAFT_BRIDGE_TIMEOUT_MS")
            .env(variable, "50")
            .output().unwrap();
        server.join().unwrap();
        assert_eq!(output.status.success(), should_succeed, "{variable}: {}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        if should_succeed {
            assert_eq!(serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["observed"], true);
        } else {
            assert!(String::from_utf8_lossy(&output.stderr).contains("bridge read"));
        }
    }
}

#[test]
#[cfg(unix)]
fn snapshot_and_tail_honor_the_explicit_bridge() {
    let result = std::process::Command::new("/usr/bin/python3")
        .args(["-c", CHECK, env!("CARGO_BIN_EXE_mc")]).output().unwrap();
    assert!(result.status.success(), "{}{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
}
#[cfg(unix)]
const CHECK: &str = r#"import json, os, pathlib, socket, subprocess, sys, tempfile, threading
cli=str(pathlib.Path(sys.argv[1]).resolve())
with tempfile.TemporaryDirectory(prefix='mc-snapshot-target-') as tmp:
    root=pathlib.Path(tmp); project=root/'Selected.magic'
    project.write_text(json.dumps({'format':'magicstudio.project','version':1,'assets':[],'workshop':{}}))
    env=dict(os.environ,XDG_CONFIG_HOME=str(root/'config'),XDG_STATE_HOME=str(root/'state'),XDG_RUNTIME_DIR=str(root/'runtime'))
    for mode in ['cli-snapshot','cli-tail','mcp-snapshot','mcp-tail']:
        listener=socket.socket();listener.bind(('127.0.0.1',0));listener.listen();listener.settimeout(.1)
        port=str(listener.getsockname()[1]);done=threading.Event();calls=[];errors=[]
        def serve():
            while not done.is_set():
                try:conn,_=listener.accept()
                except socket.timeout:continue
                except OSError:break
                try:
                    with conn:
                        conn.settimeout(2);stream=conn.makefile('rwb');req=json.loads(stream.readline());calls.append(req['id'])
                        if req['id']=='ui.feedback':result={'connection':'connected','active_project':str(project),'engine_unsaved':False,'unsaved_edits':0,'dirty_reasons':[]}
                        elif req['id']=='ui.state':result={'project':str(project),'animation':{'character':'pinned-bee','track':'authored','time':1.25,'selected_layer':'wing'}}
                        elif req['id']=='motion.tracks':result={'fps':24}
                        else:raise AssertionError(req)
                        stream.write((json.dumps({'ok':True,'result':result})+'\n').encode());stream.flush()
                except Exception as e:errors.append(repr(e))
        thread=threading.Thread(target=serve);thread.start()
        try:
            observation='center.tail' if mode.endswith('tail') else 'center.snapshot'
            if mode.startswith('cli'):
                p=subprocess.run([cli,'--bridge',port,'exec',observation,'project='+str(project)],env=env,text=True,capture_output=True,timeout=10)
                assert p.returncode==0,(p.stdout,p.stderr)
                result=json.loads(p.stdout)
            else:
                request={'jsonrpc':'2.0','id':7,'method':'tools/call','params':{'name':observation.replace('.','_'),'arguments':{'project':str(project)}}}
                p=subprocess.run([cli,'--bridge',port,'mcp'],input=json.dumps(request)+'\n',env=env,text=True,capture_output=True,timeout=10)
                assert p.returncode==0,(p.stdout,p.stderr)
                result=json.loads(json.loads(p.stdout)['result']['content'][0]['text'])
            assert not errors,errors
            assert result['project']==str(project),result
            if mode.endswith('snapshot'):
                assert result['subject']=='pinned-bee' and result['fps']==24,result
            else:assert result['events']==[],result
        finally:
            done.set();listener.close();thread.join(2)
print('PASS explicit bridge targeting for CLI/MCP snapshot and tail')
"#;
