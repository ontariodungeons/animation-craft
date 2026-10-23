//! Real mc process coverage: discovery, MCP identity, persisted project connections and failures.
use serde_json::{Value, json};
use std::{fs, io::Write, path::{Path, PathBuf}, process::{Command, Output, Stdio}};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("mc-entrypoint-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&root).unwrap();
        for dir in ["config", "state", "runtime"] { fs::create_dir(root.join(dir)).unwrap(); }
        Self(root)
    }
    fn run(&self, args: &[&str], input: &str) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_mc"))
            .args(args).current_dir(&self.0)
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .env("XDG_STATE_HOME", self.0.join("state"))
            .env("XDG_RUNTIME_DIR", self.0.join("runtime"))
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn().unwrap();
        child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
        child.wait_with_output().unwrap()
    }
}
impl Drop for Fixture { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
fn success(output: &Output) -> &str {
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::str::from_utf8(&output.stdout).unwrap()
}

#[test]
fn canonical_help_version_and_mcp_discover_same_registry() {
    let fixture = Fixture::new();
    let help = fixture.run(&["--help"], "");
    assert!(success(&help).starts_with("mc: drive Magic Studio"));
    assert!(success(&help).contains("command `mc`, args"));
    assert_eq!(success(&fixture.run(&["--version"], "")).trim(), format!("mc {}", env!("CARGO_PKG_VERSION")));
    let output = fixture.run(&["mcp"], &format!("{}\n{}\n",
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})));
    let replies: Vec<Value> = success(&output).lines().map(|s| serde_json::from_str(s).unwrap()).collect();
    assert_eq!(replies[0]["result"]["serverInfo"]["name"], "mc");
    let tools = replies[1]["result"]["tools"].as_array().unwrap();
    for id in ["project_identity", "project_open", "image_generate", "agent_send"] {
        assert!(tools.iter().any(|tool| tool["name"] == id), "missing tool {id}");
    }
    let bad = fixture.run(&["exec", "missing.command"], "");
    assert_eq!(bad.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&bad.stderr).starts_with("mc:"));
}

#[test]
fn existing_magic_identity_and_asset_bytes_survive_cli_readback() {
    let fixture = Fixture::new();
    let project = fixture.0.join("Existing.magic");
    let asset = fixture.0.join("retained.svg");
    fs::write(&asset, b"<svg xmlns='http://www.w3.org/2000/svg' width='2' height='2'/>").unwrap();
    let doc = json!({"format":"magicstudio.project","version":1,"name":"Existing",
        "assets":[{"id":"art","name":"Retained art","path":asset,"kind":"image","preset":null}],
        "workshop":{"audio":null,"draft_approved":false,"target":"hyperframes","video_project":null}});
    fs::write(&project, serde_json::to_vec_pretty(&doc).unwrap()).unwrap();
    let before_project = fs::read(&project).unwrap();
    let before_asset = fs::read(&asset).unwrap();
    let input = [json!({"id":"project.identity","params":{"path":project}}),
        json!({"id":"project.open","params":{"path":project,"record_recent":false}}),
        json!({"id":"project.list_assets","params":{}})]
        .iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
    let output = fixture.run(&["run", "-"], &input);
    let replies: Vec<Value> = success(&output).lines().map(|s| serde_json::from_str(s).unwrap()).collect();
    assert_eq!(replies.len(), 3);
    for reply in &replies { assert_eq!(reply["ok"], true); }
    assert_eq!(Path::new(replies[0]["result"]["project"].as_str().unwrap()), project.canonicalize().unwrap());
    assert_eq!(replies[1]["result"]["missing"], json!([]));
    assert_eq!(replies[2]["result"]["assets"][0]["id"], "art");
    assert_eq!(Path::new(replies[2]["result"]["assets"][0]["path"].as_str().unwrap()), asset);
    assert_eq!(fs::read(&project).unwrap(), before_project);
    assert_eq!(fs::read(&asset).unwrap(), before_asset);
}

#[test]
fn mcp_notifications_and_optional_discovery_follow_json_rpc() {
    let fixture = Fixture::new();
    let requests = [
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":1,"method":"ping"}),
        json!({"jsonrpc":"2.0","id":2,"method":"resources/list"}),
        json!({"jsonrpc":"2.0","id":3,"method":"resources/templates/list"}),
        json!({"jsonrpc":"2.0","id":4,"method":"prompts/list"}),
        json!({"jsonrpc":"2.0","id":5,"method":"missing-method"}),
    ];
    let output = fixture.run(&["mcp"], &requests.iter().map(Value::to_string).collect::<Vec<_>>().join("\n"));
    let replies: Vec<Value> = success(&output).lines().map(|s| serde_json::from_str(s).unwrap()).collect();
    assert_eq!(replies.len(), 5, "notifications must not receive responses");
    for (index, reply) in replies.iter().enumerate() { assert_eq!(reply["id"], index + 1); }
    assert_eq!(replies[0]["result"], json!({}));
    assert_eq!(replies[1]["result"]["resources"], json!([]));
    assert_eq!(replies[2]["result"]["resourceTemplates"], json!([]));
    assert_eq!(replies[3]["result"]["prompts"], json!([]));
    assert_eq!(replies[4]["error"]["code"], -32601);
    assert_eq!(replies[4]["error"]["message"], "Method not found: missing-method");
}
