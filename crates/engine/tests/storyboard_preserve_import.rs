use animationcraft_engine::Session;
use serde_json::json;
use std::fs;

#[test]
fn preserve_import_copies_document_bytes_without_rebuilding_existing_scenes() {
    let root=std::env::temp_dir().join(format!("ac-preserve-import-{}",std::process::id()));
    let source=root.join("archive");let target=root.join("active");
    fs::create_dir_all(source.join("storyboard/.preview")).unwrap();
    fs::create_dir_all(target.join("storyboard")).unwrap();
    fs::write(source.join("storyboard/storyboard.pdf"),b"archived PDF bytes").unwrap();
    fs::write(source.join("storyboard/.preview/page-01.png"),b"archived preview bytes").unwrap();
    fs::write(source.join("storyboard/.preview/layout.json"),b"{\"page_size\":[842,595],\"first_scene_page\":2}").unwrap();
    fs::write(target.join("storyboard/scene-1.md"),b"user's existing authored scene").unwrap();
    let mut s=Session::default();
    s.execute("project.new",json!({"path":target.join("Fixture.acproj")})).unwrap();
    let result=s.execute("storyboard.import_openmontage",json!({"path":source,"preserve_document":true})).unwrap();
    assert_eq!(result["preserved_existing_document"],true);
    assert_eq!(fs::read(target.join("storyboard/storyboard.pdf")).unwrap(),b"archived PDF bytes");
    assert_eq!(fs::read(target.join("storyboard/.preview/page-01.png")).unwrap(),b"archived preview bytes");
    assert_eq!(fs::read(target.join("storyboard/scene-1.md")).unwrap(),b"user's existing authored scene");
    assert!(!target.join("storyboard/movie-board.json").exists());
    assert_eq!(fs::read(source.join("storyboard/storyboard.pdf")).unwrap(),b"archived PDF bytes");
    s.execute("storyboard.import_openmontage",json!({"path":source,"preserve_document":true})).unwrap();
    fs::write(target.join("storyboard/storyboard.pdf"),b"different authored PDF").unwrap();
    assert!(s.execute("storyboard.import_openmontage",json!({"path":source,"preserve_document":true})).is_err());
    assert_eq!(fs::read(target.join("storyboard/storyboard.pdf")).unwrap(),b"different authored PDF");
    fs::remove_dir_all(root).unwrap();
}
