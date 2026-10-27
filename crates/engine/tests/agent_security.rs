use magicstudio_engine::{Session, agent, local_ipc};
use serde_json::json;
use std::fs;

#[test]
fn static_agent_log_and_session_config_work_without_agent_polling() {
    let dir = std::env::temp_dir().join(format!("mc-agent-log-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let project_dir = dir.join("project");
    fs::create_dir_all(project_dir.join(".agent")).unwrap();
    let project_path = project_dir.join("Song.magic");
    fs::write(&project_path, "{}").unwrap();
    fs::write(
        project_dir.join(".agent/agent-log.json"),
        serde_json::to_vec(&json!([
            {"role": "user", "text": "hello"},
            {"role": "assistant", "text": "ready"}
        ]))
        .unwrap(),
    )
    .unwrap();

    let mut session = Session::default();
    session.project_path = Some(project_path);
    session.agent_selection = Some(json!({
        "agent": "omp",
        "provider": "ollama",
        "model": "llama3.2:1b",
        "thinking": "",
        "extra_args": []
    }));

    let turns = agent::cmd_agent_log(&mut session, &json!({})).unwrap();
    assert_eq!(turns.as_array().unwrap().len(), 2);

    let mut other = Session::default();
    other.agent_selection = Some(json!({
        "agent": "omp",
        "provider": "ollama",
        "model": "llama3.2:1b",
        "thinking": "",
        "extra_args": []
    }));
    agent::cmd_agent_config(&mut session, &json!({"selection": {"model": "qwen2.5:1.5b"}})).unwrap();
    assert_eq!(agent::cmd_agent_config(&mut session, &json!({})).unwrap()["model"], "qwen2.5:1.5b");
    assert_eq!(agent::cmd_agent_config(&mut other, &json!({})).unwrap()["model"], "llama3.2:1b");

    let cleared = agent::cmd_agent_new_chat(&mut session, &json!({})).unwrap();
    assert_eq!(cleared["turns"], json!([]));
    assert_eq!(agent::cmd_agent_log(&mut session, &json!({})).unwrap(), json!([]));
    let _ = fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn local_ipc_private_path_restricts_directory_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("mc-ipc-sec-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
    local_ipc::private_path(&dir, true).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(local_ipc::private_path(&dir, true).is_err());
    let _ = fs::remove_dir_all(&dir);
}
