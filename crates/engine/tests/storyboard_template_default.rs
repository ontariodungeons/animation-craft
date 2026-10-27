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
}
