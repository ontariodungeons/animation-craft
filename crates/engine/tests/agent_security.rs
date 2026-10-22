//! Offline security regression tests. The fake provider executable is a process-boundary fixture,
//! never a replacement production route. Each case re-execs with isolated settings and dummy data.
use animationcraft_engine::{Session, agent};
use serde_json::json;
use std::{fs, path::PathBuf, process::Command};

const TOOL: &str = r#"#!/usr/bin/python3
import json, os, sys, pathlib
root=pathlib.Path(os.environ['AC_SECURITY_FIXTURE'])
args=sys.argv[1:]
with (root/'calls.jsonl').open('a') as f: f.write(json.dumps(args)+'\n')
operation=args[1]
case=os.environ['AC_SECURITY_CASE']
if case=='auth':
 print('401 Unauthorized Authorization: Bearer dummy-secret-123 api_key=dummy-secret-456',file=sys.stderr)
 sys.exit(23)
model={'providerID':'opencode','id':'synthetic-free'}
price=1 if case=='paid' else 0
if operation=='model.list':
 print(json.dumps({'data':[dict(m,name='Synthetic',enabled=True,capabilities={'tools':True},cost=[{'input':price,'output':price,'cache':{'read':0,'write':0}}]) for m in [model,{'providerID':'secondary','id':'synthetic-other-free'}]]}))
elif operation=='session.create':
 n=int((root/'counter').read_text())+1 if (root/'counter').exists() else 1
 (root/'counter').write_text(str(n))
 model=json.loads(args[-1])['model']
 (root/'active-model').write_text(json.dumps(model))
 print(json.dumps({'data':{'id':'ses_synthetic_'+str(n),'model':model}}))
elif operation=='session.get':
 model=json.loads((root/'active-model').read_text())
 if case=='mismatch': model['id']='synthetic-paid'
 print(json.dumps({'data':{'id':'ses_synthetic_'+(root/'counter').read_text(),'model':model}}))
elif operation=='session.prompt': print(json.dumps({'data':{'id':'msg_synthetic','time':{'created':1}}}))
elif operation=='session.message.list':
 print(json.dumps({'data':[{'id':'reply','type':'assistant','time':{'created':2},'content':[{'type':'text','text':'synthetic reply'}]},{'type':'idle','time':{'created':3}}]}))
elif operation in ['session.permission.list','session.form.list']: print('{"data":[]}')
else: sys.exit(91)
"#;

fn run_case(name: &str) {
    let root = std::env::temp_dir().join(format!("ac-security-{}-{name}", std::process::id()));
    fs::create_dir_all(root.join("config/animationcraft")).unwrap();
    fs::create_dir_all(root.join("project")).unwrap();
    fs::write(root.join("opencode"), TOOL).unwrap();
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(root.join("opencode"), fs::Permissions::from_mode(0o700)).unwrap();
    }
    fs::write(root.join("config/animationcraft/agent.json"), json!({
        "agent":"opencode","provider":"opencode","model":"synthetic-free",
        "api_key":"dummy-config-secret"
    }).to_string()).unwrap();
    let result=Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "agent_security_child", "--nocapture"])
        .env("AC_SECURITY_CASE",name).env("AC_SECURITY_FIXTURE",&root)
        .env("XDG_CONFIG_HOME",root.join("config"))
        .env("PATH",&root).output().unwrap();
    let _=fs::remove_dir_all(&root);
    assert!(result.status.success(),"case {name}: {}\n{}",String::from_utf8_lossy(&result.stdout),String::from_utf8_lossy(&result.stderr));
    assert!(String::from_utf8_lossy(&result.stdout).contains("CASE_ASSERTIONS_COMPLETED"));
}

#[test] fn agent_security_saved_selection_and_fresh_sessions() { run_case("fresh"); }
#[test] fn agent_security_active_model_selection_is_isolated() { run_case("isolation"); }
#[test] fn agent_security_queued_reply_is_retrievable() { run_case("queued"); }
#[test] fn agent_security_auth_rejection_is_redacted() { run_case("auth"); }
#[test] fn agent_security_model_mismatch_never_prompts() { run_case("mismatch"); }
#[test] fn agent_security_unknown_model_never_prompts() { run_case("unknown"); }
#[test] fn agent_security_adapter_override_cannot_change_app_selection() { run_case("override"); }
#[test] fn agent_security_changed_app_selection_reaches_next_fresh_session() { run_case("switch"); }
#[test] fn agent_security_invalid_saved_model_never_falls_back() { run_case("invalid"); }
#[test] fn agent_security_free_only_rejects_paid_model() { run_case("paid"); }
#[test] fn agent_security_new_chat_has_no_inference_and_preserves_history() { run_case("newchat"); }
#[test] fn agent_security_extra_args_cannot_override_selection_or_permissions() { run_case("extra"); }

#[test]
#[ignore = "subprocess fixture entry point, invoked by each isolated behavior test"]
fn agent_security_child() {
    let case=std::env::var("AC_SECURITY_CASE").expect("run through isolated parent test");
    let root=PathBuf::from(std::env::var_os("AC_SECURITY_FIXTURE").unwrap());
    let mut session=Session::default();
    session.project_path=Some(root.join("project/Synthetic.acproj"));
    let mut params=json!({"message":"Synthetic offline check","wait":5});
    if case=="invalid" {
        fs::write(root.join("config/animationcraft/agent.json"),json!({"agent":"opencode","provider":"opencode","model":123}).to_string()).unwrap();
    }
    if case=="unknown" {
        fs::write(root.join("config/animationcraft/agent.json"),json!({"agent":"opencode","provider":"opencode","model":"missing-free"}).to_string()).unwrap();
    }
    if case=="override" {params["agent"]=json!("codex");}
    if case=="paid" {params["free_only"]=json!(true);}
    if case=="extra" {
        fs::write(root.join("config/animationcraft/agent.json"),json!({"agent":"opencode","provider":"opencode","model":"synthetic-free","extra_args":["--dangerously-skip-permissions"]}).to_string()).unwrap();
    }
    if case=="newchat" {
        fs::create_dir_all(root.join("project/.agent")).unwrap();
        fs::write(root.join("project/.agent/agent-log.json"),"[\"previous-private-context\"]").unwrap();
        let first=agent::cmd_agent_new_chat(&mut session,&json!({})).unwrap();
        assert_eq!(first["inference_started"],false);
        let second=agent::cmd_agent_new_chat(&mut session,&json!({})).unwrap();
        assert_ne!(first["session"],second["session"]);
        assert_eq!(second["turns"],json!([]));
        let history=fs::read_to_string(root.join("project/.agent/chat-history.jsonl")).unwrap();
        assert!(history.contains(first["session"].as_str().unwrap()));
        let current=agent::cmd_agent_session(&mut session,&json!({})).unwrap();
        assert_eq!(current["session"],second["session"]);
        let calls=fs::read_to_string(root.join("calls.jsonl")).unwrap();
        assert!(!calls.contains("session.prompt"));
        assert!(!calls.contains("previous-private-context"));
        let archived:Vec<_>=fs::read_dir(root.join("project/.agent")).unwrap().filter_map(Result::ok).filter(|e|e.file_name().to_string_lossy().starts_with("agent-log.")).collect();
        assert_eq!(archived.len(),1);
        assert_eq!(fs::read_to_string(archived[0].path()).unwrap(),"[\"previous-private-context\"]");
        println!("CASE_ASSERTIONS_COMPLETED");
        return;
    }
    if case=="isolation" {
        let before=fs::read(root.join("config/animationcraft/agent.json")).unwrap();
        let mut other=Session::default();
        agent::cmd_agent_config(&mut session,&json!({})).unwrap();
        agent::cmd_agent_config(&mut other,&json!({})).unwrap();
        agent::cmd_agent_config(&mut session,&json!({"selection":{"provider":"secondary","model":"synthetic-other-free"}})).unwrap();
        assert_eq!(agent::cmd_agent_config(&mut session,&json!({})).unwrap()["model"],"synthetic-other-free");
        assert_eq!(agent::cmd_agent_config(&mut other,&json!({})).unwrap()["model"],"synthetic-free");
        assert_eq!(fs::read(root.join("config/animationcraft/agent.json")).unwrap(),before);
        println!("CASE_ASSERTIONS_COMPLETED");return;
    }
    if case=="queued" {
        params["wait"]=json!(0);params["_request_id"]=json!("queued-check");
        let sent=agent::cmd_agent_send(&mut session,&params).unwrap();
        assert_eq!(sent["status"],"running");assert_eq!(sent["request_id"],"queued-check");
        let log=agent::cmd_agent_log(&mut session,&json!({"refresh":true})).unwrap();
        assert!(log.as_array().unwrap().iter().any(|m|m["role"]=="assistant"&&m["request_id"]=="queued-check"&&m["text"]=="synthetic reply"));
        assert_eq!(agent::cmd_agent_session(&mut session,&json!({})).unwrap()["request"]["status"],"replied");
        let again=agent::cmd_agent_log(&mut session,&json!({"refresh":true})).unwrap();assert_eq!(log,again);
        session.project_path=Some(root.join("project/Other.acproj"));
        assert!(agent::cmd_agent_send(&mut session,&json!({"message":"must not cross projects","new_chat":false})).is_err());
        println!("CASE_ASSERTIONS_COMPLETED");return;
    }
    let reply=agent::cmd_agent_send(&mut session,&params);
    if case=="fresh" || case=="switch" {
        let first=reply.unwrap();
        assert_eq!(first["model"],"synthetic-free");
        assert_eq!(first["provider"],"opencode");
        assert_eq!(first["text"],"synthetic reply");
        assert!(first["session"].as_str().is_some());
        if case=="switch" {
            agent::cmd_agent_config(&mut session,&json!({"selection":{"provider":"secondary","model":"synthetic-other-free"}})).unwrap();
            fs::write(root.join("config/animationcraft/agent.json"),json!({"agent":"opencode","provider":"secondary","model":"synthetic-other-free","image_model":"preserve-me"}).to_string()).unwrap();
        }
        let second=agent::cmd_agent_send(&mut session,&params).unwrap();
        if case=="switch" {
            assert_eq!(second["provider"],"secondary");
            assert_eq!(second["model"],"synthetic-other-free");
            let config:serde_json::Value=serde_json::from_slice(&fs::read(root.join("config/animationcraft/agent.json")).unwrap()).unwrap();
            assert_eq!(config["image_model"],"preserve-me");
        }
        assert_ne!(first["session"],second["session"]);
        assert!(!second.to_string().contains("dummy-config-secret"));
        let calls=fs::read_to_string(root.join("calls.jsonl")).unwrap();
        for line in calls.lines().filter(|l|l.contains("session.create")) {
            assert!(line.contains("synthetic-free") || line.contains("synthetic-other-free"));
            assert!(!line.contains("parentID"));
        }
    } else {
        let error=reply.unwrap_err().to_string();
        if case=="auth" {
            assert!(error.to_lowercase().contains("authentication"),"{error}");
            assert!(error.contains("23"),"{error}");
            assert!(!error.contains("dummy-secret"),"{error}");
            assert!(!error.contains("Bearer"),"{error}");
        }
        if case=="invalid" {assert!(error.contains("Invalid saved model"),"{error}");}
        if case=="paid" {assert!(error.contains("nonzero or unknown cost"),"{error}");}
        if case=="extra" {assert!(error.contains("Extra chat arguments"),"{error}");}
        if case=="unknown" {assert!(error.contains("absent or disabled"),"{error}");}
        if case=="mismatch" {assert!(error.contains("differs"),"{error}");}
        if case=="override" {assert!(error.contains("overridden"),"{error}");}
        let calls=fs::read_to_string(root.join("calls.jsonl")).unwrap_or_default();
        assert!(!calls.contains("session.prompt"),"must reject before inference: {calls}");
        if case=="override" || case=="extra" {assert!(calls.is_empty());}
    }
    println!("CASE_ASSERTIONS_COMPLETED");
}

#[cfg(unix)]
#[test]
fn agent_security_rejects_nonprivate_symlink_and_wrong_type_paths() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let root=std::env::temp_dir().join(format!("ac-path-security-{}",std::process::id()));
    fs::create_dir_all(&root).unwrap();
    fs::set_permissions(&root,fs::Permissions::from_mode(0o700)).unwrap();
    assert!(animationcraft_engine::local_ipc::private_path(&root,true).is_ok());
    fs::set_permissions(&root,fs::Permissions::from_mode(0o755)).unwrap();
    assert!(animationcraft_engine::local_ipc::private_path(&root,true).is_err());
    fs::set_permissions(&root,fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(root.join("regular"),b"dummy").unwrap();
    symlink(&root,root.join("link")).unwrap();
    assert!(animationcraft_engine::local_ipc::private_path(&root.join("link"),true).is_err());
    assert!(animationcraft_engine::local_ipc::private_path(&root.join("regular"),false).is_err());
    fs::remove_dir_all(root).unwrap();
}
