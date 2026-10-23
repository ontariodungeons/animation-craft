use magicstudio_engine::Session;
use serde_json::json;
use std::fs;

#[test]
fn removed_static_copy_path_fails_without_altering_originals() {
    let root=std::env::temp_dir().join(format!("ac-preserve-import-{}",std::process::id()));
    let source=root.join("archive");let target=root.join("active");
    fs::create_dir_all(source.join("storyboard/.preview")).unwrap();
    fs::create_dir_all(target.join("storyboard")).unwrap();
    fs::write(source.join("storyboard/storyboard.pdf"),b"archived PDF bytes").unwrap();
    fs::write(source.join("storyboard/.preview/page-01.png"),b"archived preview bytes").unwrap();
    fs::write(source.join("storyboard/.preview/layout.json"),b"{\"page_size\":[842,595],\"first_scene_page\":2}").unwrap();
    fs::write(target.join("storyboard/scene-1.md"),b"user's existing authored scene").unwrap();
    let mut s=Session::default();
    s.execute("project.new",json!({"path":target.join("Fixture.magic")})).unwrap();
    let error=s.execute("storyboard.import_openmontage",json!({"path":source,"preserve_document":true})).unwrap_err().to_string();
    assert!(error.contains("Static document copying was removed"));
    assert!(!target.join("storyboard/storyboard.pdf").exists());
    assert_eq!(fs::read(target.join("storyboard/scene-1.md")).unwrap(),b"user's existing authored scene");
    assert_eq!(fs::read(source.join("storyboard/storyboard.pdf")).unwrap(),b"archived PDF bytes");
    fs::remove_dir_all(root).unwrap();
}
