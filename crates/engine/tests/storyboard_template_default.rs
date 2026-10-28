//! Default storyboard template behaviour: Magic Studio is the default template.
use magicstudio_engine::Session;
use serde_json::json;

#[test]
fn storyboard_template_defaults_to_magic_studio() {
    let dir = std::env::temp_dir().join(format!("ac-storyboard-template-{}", std::process::id()));
    let project = dir.join("Fixture.magic");
    let mut session = Session::default();
    session
        .execute("project.new", json!({"path":project,"overwrite":true}))
        .unwrap();
    let result = session.execute("storyboard.template", json!({})).unwrap();
    let path = result["path"].as_str().expect("storyboard.template returns a path");
    assert!(
        path.ends_with("pipeline/templates/studio-template.deckcraft"),
        "unexpected default storyboard template: {path}"
    );
    assert_eq!(result["generator"], "pipeline/movie_board.py");
    std::fs::create_dir_all(dir.join("storyboard")).unwrap();
    let stale = dir.join("storyboard/template.json");
    let selection = br#"{"path":"/missing/other-template.pptx"}"#;
    std::fs::write(&stale, selection).unwrap();
    assert_eq!(session.execute("storyboard.template", json!({})).unwrap()["path"], result["path"]);
    let other = dir.join("other.deckcraft");
    std::fs::copy(path, &other).unwrap();
    for requested in [other.to_string_lossy().into_owned(), "backlot".into(), "other.pptx".into()] {
        let error = session.execute("storyboard.template", json!({"path":requested})).unwrap_err();
        assert!(error.to_string().contains("uses only"));
    }
    assert_eq!(std::fs::read(stale).unwrap(), selection);
    assert!(session.execute("storyboard.template", json!({"path":path})).is_ok());
    std::fs::remove_dir_all(dir).unwrap();
}
