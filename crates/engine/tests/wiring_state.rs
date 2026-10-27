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
