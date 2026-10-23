//! Optional global navigation history must not undo successful project persistence/loading.
use magicstudio_engine::Session;
use serde_json::json;
use std::{fs,process::Command};

#[test]
fn new_and_open_survive_unavailable_global_history() {
    let root=std::env::temp_dir().join(format!("ac-navigation-{}",std::process::id()));
    fs::create_dir_all(&root).unwrap();
    // A regular file cannot be used as a config directory, independently of UID/permission mode.
    fs::write(root.join("blocked-config"),b"not a directory").unwrap();
    let child=Command::new(std::env::current_exe().unwrap())
        .args(["--ignored","--exact","navigation_child","--nocapture"])
        .env("AC_NAVIGATION_FIXTURE",&root).env("XDG_CONFIG_HOME",root.join("blocked-config"))
        .env("XDG_STATE_HOME",root.join("state")).output().unwrap();
    let _=fs::remove_dir_all(root);
    assert!(child.status.success(),"{}\n{}",String::from_utf8_lossy(&child.stdout),String::from_utf8_lossy(&child.stderr));
}

#[test]
#[ignore="isolated environment subprocess helper"]
fn navigation_child(){
    let root=std::path::PathBuf::from(std::env::var_os("AC_NAVIGATION_FIXTURE").unwrap());
    let project=root.join("song/project/Synthetic.magic");
    let mut created=Session::user_session();
    let result=created.execute("project.new",json!({"path":project,"overwrite":false})).unwrap();
    assert_eq!(result["ok"],true);assert!(project.is_file());
    assert_eq!(result["warnings"][0]["code"],"recent_history_unavailable");
    let before=fs::read(&project).unwrap();drop(created);
    let mut opened=Session::user_session();
    let result=opened.execute("project.open",json!({"path":project})).unwrap();
    assert_eq!(result["ok"],true);assert_eq!(result["warnings"][0]["code"],"recent_history_unavailable");
    assert_eq!(fs::read(&project).unwrap(),before);
}
