use magicstudio_engine::Session;
use serde_json::json;
#[cfg(unix)]
fn runtime(dir: &std::path::Path, body: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let file = dir.join("hyperframes-cli");
    std::fs::write(&file, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700)).unwrap();
    file
}
#[test]
#[cfg(unix)]
fn video_project_path_persists_across_reopen() {
    let dir = std::env::temp_dir().join(format!("ac-wiring-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let project = dir.join("Fixture.magic");
    let mut s = Session::default();
    s.execute("project.new", json!({"path": project})).unwrap();
    s.video_target = "hyperframes".into();
    let cli = runtime(&dir, "mkdir -p \"$2\"; printf '%s' '{}' > \"$2/hyperframes.json\"; printf '%s' '<div></div>' > \"$2/index.html\"");
    let r = s.execute("video.init", json!({"target": "hyperframes", "hyperframes_cli": cli})).unwrap();
    assert_eq!(r["reused_existing_project"], false);
    let vpp = r["video_project_path"].as_str().expect("video_project_path returned");
    assert!(std::path::Path::new(vpp).is_file(), "video_project_path must point to existing project file");
    let _ = s.execute("project.close", json!({}));
    // Reopen and verify persistence.
    let mut s2 = Session::default();
    s2.execute("project.open", json!({"path": project})).unwrap();
    assert_eq!(s2.video_project_path.as_deref(), Some(std::path::Path::new(vpp)));
    assert!(s2.video_project_path.as_ref().unwrap().is_file());
    // Idempotency: second init reuses.
    s2.video_target = "hyperframes".into();
    let repeated = s2.execute("video.init", json!({"target": "hyperframes", "hyperframes_cli": cli})).unwrap();
    assert_eq!(repeated["reused_existing_project"], true);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn no_project_means_no_video_project_path() {
    let s = Session::default();
    assert!(s.video_project_path.is_none());
}