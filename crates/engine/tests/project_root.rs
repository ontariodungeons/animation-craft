//! Run settings tests in a child so config/history never touch the user's files.
use magicstudio_engine::Session;
use serde_json::{Value, json};
use std::{fs, process::Command};

#[test]
fn production_root_survives_test_projects_and_recent_history() {
    let root = std::env::temp_dir().join(format!("ac-project-root-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "project_root_child", "--nocapture"])
        .env("AC_ROOT_FIXTURE", &root)
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_RUNTIME_DIR", root.join("runtime"))
        .output()
        .unwrap();
    let _ = fs::remove_dir_all(root);
    assert!(
        child.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    );
}

#[test]
#[ignore = "isolated environment subprocess helper"]
fn project_root_child() {
    let root = std::path::PathBuf::from(std::env::var_os("AC_ROOT_FIXTURE").unwrap());
    let production = root.join("production");
    let tests = root.join("explicit-tests");
    fs::create_dir_all(&production).unwrap();
    fs::create_dir_all(&tests).unwrap();
    let mut session = Session::user_session();
    assert_eq!(
        session.execute("project.root", json!({})).unwrap()["root"],
        "/run/media/jesse/Speedy/media"
    );
    let configured = session
        .execute("project.root", json!({"root":production}))
        .unwrap();
    assert_eq!(configured["root"], production.to_string_lossy().as_ref());
    let config = std::path::PathBuf::from(configured["config"].as_str().unwrap());
    let mut existing: Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    existing["unrelated_preference"] = json!("preserve me");
    fs::write(&config, existing.to_string()).unwrap();
    session
        .execute("project.root", json!({"root":production}))
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&config).unwrap()).unwrap()["unrelated_preference"],
        "preserve me"
    );
    let saved = fs::read(&config).unwrap();
    assert!(serde_json::from_slice::<Value>(&saved).unwrap().is_object());
    for invalid in [
        json!({"root":""}),
        json!({"root":"relative"}),
        json!({"root":false}),
    ] {
        assert!(session.execute("project.root", invalid).is_err());
        assert_eq!(fs::read(&config).unwrap(), saved);
    }
    let test = session
        .execute("project.intake", json!({"root":tests,"name":"Test Song"}))
        .unwrap();
    assert_eq!(
        test["project"],
        tests
            .join("test-song/project/Test Song.magic")
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(test["location"]["source"], "explicit root override");
    assert_eq!(
        test["location"]["production_root"],
        production.to_string_lossy().as_ref()
    );
    assert_eq!(fs::read(&config).unwrap(), saved);
    // Fixtures under the configured temporary directory never enter Recent, including custom TMPDIR.
    assert!(!magicstudio_engine::project_catalog::list().iter().any(|p| p.contains("explicit-tests")));
    let created = session
        .execute("project.intake", json!({"name":"Production Song"}))
        .unwrap();
    let expected = production.join("production-song/project/Production Song.magic");
    assert_eq!(created["project"], expected.to_string_lossy().as_ref());
    assert!(expected.is_file());
    assert_eq!(created["location"]["source"], "production root");
    let reopened = Session::default()
        .execute("project.open", json!({"path":expected}))
        .unwrap();
    assert_eq!(reopened["ok"], true);
    assert_eq!(fs::read(&config).unwrap(), saved);
    assert_eq!(
        Session::default()
            .execute("project.root", json!({}))
            .unwrap()["root"],
        configured["root"]
    );
    let explicit = tests.join("different-location/project/Explicit.magic");
    let receipt = session
        .execute("project.intake", json!({"path":explicit}))
        .unwrap();
    assert_eq!(receipt["location"]["source"], "explicit path override");
    assert!(explicit.is_file());
    assert_eq!(fs::read(&config).unwrap(), saved);
    fs::remove_dir_all(&production).unwrap();
    let unavailable = session
        .execute("project.intake", json!({"name":"Unavailable Root"}))
        .unwrap_err();
    assert!(unavailable.to_string().contains("unavailable"));
    assert!(
        !production.exists(),
        "an unavailable mount must not be recreated implicitly"
    );
    // Corrupt settings fail explicitly; history must never become a fallback.
    fs::write(&config, b"{broken").unwrap();
    assert!(
        session
            .execute("project.intake", json!({"name":"Must Not Exist"}))
            .is_err()
    );
    assert!(!production.join("must-not-exist").exists());
    assert_eq!(fs::read(&config).unwrap(), b"{broken");
}
