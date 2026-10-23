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
#[cfg(unix)]
fn fixture(name: &str) -> (std::path::PathBuf, Session, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("ac-wiring-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = Session::default();
    s.execute("project.new", json!({"path": dir.join("Fixture.magic")})).unwrap();
    let cli = runtime(&dir, "mkdir -p \"$2\"; printf '%s' '{}' > \"$2/hyperframes.json\"; printf '%s' '<div></div>' > \"$2/index.html\"");
    (dir, s, cli)
}

#[test]
#[cfg(unix)]
fn status_reports_only_the_stored_video_project() {
    let (dir, mut s, _) = fixture("stored-only");
    // A FilmCraft file left in the folder is not this project's video project until video.init says so.
    std::fs::write(dir.join("Old.fcproj"), "{}").unwrap();
    let status = s.execute("workshop.status", json!({})).unwrap();
    assert!(status["video_project_path"].is_null(), "nothing was initialised: {status}");
    assert!(status.get("video_project").is_none(), "one answer, not a stored one and a guessed one");
    assert_eq!(status["video_project_exists"], false);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[cfg(unix)]
fn deleted_video_project_reports_missing_not_another_file() {
    let (dir, mut s, cli) = fixture("deleted");
    s.video_target = "hyperframes".into();
    let r = s.execute("video.init", json!({"target": "hyperframes", "hyperframes_cli": cli})).unwrap();
    let stored = r["video_project_path"].as_str().unwrap().to_string();
    std::fs::remove_dir_all(dir.join("hyperframes")).unwrap();
    std::fs::write(dir.join("Old.fcproj"), "{}").unwrap();
    let status = s.execute("workshop.status", json!({})).unwrap();
    assert_eq!(status["video_project_path"], json!(stored), "the stored answer stays the answer");
    assert_eq!(status["video_project_exists"], false, "and it says the file is gone");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[cfg(unix)]
fn changing_target_does_not_keep_the_old_targets_project() {
    let (dir, mut s, cli) = fixture("retarget");
    s.video_target = "hyperframes".into();
    let r = s.execute("video.init", json!({"target": "hyperframes", "hyperframes_cli": cli})).unwrap();
    let hyperframes = r["video_project_path"].as_str().unwrap().to_string();
    let status = s.execute("workshop.set_target", json!({"target": "remotion"})).unwrap();
    assert_ne!(status["video_project_path"], json!(hyperframes), "remotion must not report the HyperFrames project");
    assert_eq!(status["video_project_exists"], status["video_project_path"].as_str().is_some_and(|p| std::path::Path::new(p).exists()));
    std::fs::remove_dir_all(dir).unwrap();
}
