use magicstudio_engine::{ProjectAsset, Session};
use serde_json::{json, Value};

#[test]
fn resolves_actions_after_angle_reload_and_preserves_explicit_rest() {
    let root = std::env::temp_dir().join(format!("action-track-{}", std::process::id()));
    std::fs::create_dir_all(root.join("rigs/side")).unwrap();
    image::RgbaImage::from_pixel(16, 16, image::Rgba([255, 0, 0, 255]))
        .save(root.join("part.png")).unwrap();
    let rig = |track| json!({"canvas":[16,16],"fps":24,
        "layers":[{"id":"body","file":root.join("part.png"),"pivot":[2,2]}],
        "tracks":[{"id":track,"label":track,"duration":1,"motion":{}}]});
    std::fs::write(root.join("rig.json"), rig("front").to_string()).unwrap();
    std::fs::write(root.join("rigs/side/rig.json"), rig("walk").to_string()).unwrap();
    std::fs::write(root.join("character.magic"), json!({"active_angle":"front",
        "angles":{"front":{"id":"front"},"side":{"id":"side"}},
        "poses":{"walk":{"id":"walk","angle":"side"}}}).to_string()).unwrap();
    std::fs::write(root.join("Test.magic"), "{\"theatre\":null}").unwrap();
    let mut s = Session {project_path:Some(root.join("Test.magic")),
        project_assets:vec![ProjectAsset {id:"subject".into(),name:"Subject".into(),kind:"cutout_rig".into(),
            path:Some(root.join("rig.json")),preset:None,parts:vec![]}], ..Session::default()};
    let result = s.execute("motion.action_track", json!({"character":"subject","action":"walk"})).unwrap();
    assert_eq!(result["track"], "walk");
    assert_eq!(result["standard"], false);
    assert_eq!(result["rest"], false);
    assert_eq!(result["character_panel"]["active_angle"], "side");
    let saved: Value = serde_json::from_slice(&std::fs::read(root.join("character.magic")).unwrap()).unwrap();
    assert_eq!(saved["active_angle"], "side");
    assert_eq!(saved["active_pose"], "walk");
    assert_eq!(result["layers"][0]["id"], "body");
    assert_eq!(result["layers"][0]["image_sha256"].as_str().unwrap().len(), 64);
    let fallback = s.execute("motion.action_track", json!({"character":"subject","action":"run"})).unwrap();
    assert_eq!(fallback["track"], "@run");
    assert_eq!(fallback["standard"], true);
    let rest = s.execute("motion.action_track", json!({"character":"subject","action":"rest","track":"walk","rest":true})).unwrap();
    assert_eq!(rest["track"], "walk");
    assert_eq!(rest["rest"], true);
    assert!(s.execute("motion.action_track", json!({"character":"subject","action":"missing"})).is_err());
    assert!(s.execute("motion.action_track", json!({"character":"subject","action":"walk","track":"missing"})).is_err());
    assert!(s.execute("motion.action_track", json!({"character":"subject"})).is_err());
    std::fs::remove_dir_all(root).unwrap();
}
