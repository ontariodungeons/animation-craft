use magicstudio_engine::{ProjectAsset, Session};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

fn fixture() -> (Session, PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "motion-authoring-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let project = root.join("Generic.magic");
    fs::write(&project, "{\"theatre\":null}").unwrap();
    image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]))
        .save(root.join("part.png"))
        .unwrap();
    let rig = root.join("rig.json");
    fs::write(&rig,json!({"canvas":[16,16],"fps":24,"layers":[{"id":"body","file":root.join("part.png"),"pivot":[2,2]}],"tracks":[{"id":"move","label":"Move","duration":1,"loop":false,"motion":{}}]}).to_string()).unwrap();
    // Full-canvas artwork is required by the shared loader.
    let mut pixels = image::RgbaImage::new(16, 16);
    for y in 0..4 {
        for x in 0..4 {
            pixels.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
        }
    }
    pixels.save(root.join("part.png")).unwrap();
    let session = Session {
        project_path: Some(project),
        project_assets: vec![ProjectAsset {
            id: "subject".into(),
            name: "Subject".into(),
            kind: "cutout_rig".into(),
            path: Some(rig.clone()),
            preset: None,
            parts: vec![],
        }],
        ..Session::default()
    };
    (session, root, rig)
}
fn params(s: &Session, rig: &PathBuf) -> Value {
    json!({"expected_project":s.project_path,"character":"subject","track":"move","part":"body","expected_rig_sha256":format!("{:x}",Sha256::digest(fs::read(rig).unwrap()))})
}
#[test]
fn persistent_channels_sample_and_undo_rejects_stale_source() {
    let (mut s, root, rig) = fixture();
    let original = fs::read(&rig).unwrap();
    let mut p = params(&s, &rig);
    p["position"] = json!([5, 3]);
    p["scale"] = json!([150, 100]);
    let edit = s
        .execute("motion.part.set", p.clone())
        .expect("shared part setter must exist");
    assert_ne!(edit["rig_sha256"], p["expected_rig_sha256"]);
    let sample = s
        .execute(
            "motion.sample",
            json!({"character":"subject","track":"move","time":0.5}),
        )
        .unwrap();
    assert_eq!(sample["frames"].as_array().unwrap().len(), 1);
    assert_eq!(
        sample["frames"][0]["layers"][0]["position"],
        json!([5.0, 3.0])
    );
    assert!(s.execute("motion.part.set", p).is_err());
    let mut undo = params(&s, &rig);
    undo["undo"] = edit["undo"].clone();
    s.execute("motion.edit.undo", undo).unwrap();
    assert_eq!(fs::read(&rig).unwrap(), original);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn pose_exports_actual_sampled_pixels_and_refuses_unsaved_theatre() {
    let (mut s, root, rig) = fixture();
    let mut p = params(&s, &rig);
    p["position"] = json!([5, 3]);
    s.execute("motion.part.set", p).expect("part setter");
    let mut p = params(&s, &rig);
    p["time"] = json!(0.5);
    p["theatre_state"] = Value::Null;
    let pose = s
        .execute("motion.pose.export", p.clone())
        .expect("pose export");
    assert_eq!(pose["approval"], "pending");
    let image = image::open(pose["path"].as_str().unwrap())
        .unwrap()
        .into_rgba8();
    assert_eq!(image.get_pixel(5, 3).0, [255, 0, 0, 255]);
    assert_eq!(image.get_pixel(0, 0).0[3], 0);
    assert!(
        s.project_assets
            .iter()
            .any(|a| Some(a.id.as_str()) == pose["asset_id"].as_str())
    );
    p["theatre_state"] = json!({"sheetsById":{"subject":{"changed":true}}});
    assert!(s.execute("motion.pose.export", p).is_err());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn visibility_uses_captured_character_instead_of_current_selection() {
    let (mut s, root, _) = fixture();
    s.execute("project.load_asset", json!({"id":"subject"}))
        .unwrap();
    s.active_asset = None;
    let p = json!({"character":"subject","expected_project":s.project_path,"part":"body","visible":false});
    s.execute("part.set_state", p)
        .expect("captured character must resolve independently of selection");
    assert!(s.active_asset.is_none());
    assert!(
        !s.assets["subject"]
            .parts
            .iter()
            .find(|p| p.name == "body")
            .unwrap()
            .visible
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn exact_field_reset_and_action_metadata_keep_stable_ids_and_guards() {
    let (mut s, root, rig) = fixture();
    let original = fs::read(&rig).unwrap();
    let mut p = params(&s, &rig);
    p["rotation"] = json!(9000);
    assert!(s.execute("motion.part.set", p).is_err());
    assert_eq!(fs::read(&rig).unwrap(), original);
    let other = root.join("Other.magic");
    fs::write(&other, "{}").unwrap();
    let mut p = params(&s, &rig);
    p["expected_project"] = json!(other);
    p["rotation"] = json!(15);
    assert!(s.execute("motion.part.set", p).is_err());
    let mut p = params(&s, &rig);
    p["rotation"] = json!(15);
    p["position"] = json!([5, 3]);
    p["pivot"] = json!([4, 4]);
    s.execute("motion.part.set", p).unwrap();
    let mut p = params(&s, &rig);
    p["fields"] = json!(["position", "pivot"]);
    s.execute("motion.part.reset", p).unwrap();
    let read = s
        .execute("motion.tracks", json!({"character":"subject"}))
        .unwrap();
    let track = read["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == "move")
        .unwrap();
    assert_eq!(track["static_overrides"]["body"]["position"], json!([0, 0]));
    assert_eq!(track["static_overrides"]["body"]["rotation"], 15.0);
    assert_eq!(read["layers"][0]["pivot"], json!([2.0, 2.0]));
    let mut p = params(&s, &rig);
    p["label"] = json!("New label");
    p["duration"] = json!(2);
    p["loop"] = json!(true);
    let edit = s.execute("motion.action.set", p).unwrap();
    let after: Value = serde_json::from_slice(&fs::read(&rig).unwrap()).unwrap();
    assert_eq!(after["tracks"][0]["id"], "move");
    assert_eq!(after["tracks"][0]["label"], "New label");
    assert_eq!(after["tracks"][0]["duration"], 2.0);
    assert_eq!(after["tracks"][0]["loop"], true);
    let mut undo = params(&s, &rig);
    undo["undo"] = edit["undo"].clone();
    undo["character"] = json!("other");
    assert!(s.execute("motion.edit.undo", undo).is_err());
    s.theatre_state =
        Some(json!({"definitionVersion":"0.4.0","revisionHistory":[],"sheetsById":{"subject / move":{"sequence":{"tracksByObject":{"body":{}}}}}}));
    let read = s
        .execute("motion.tracks", json!({"character":"subject"}))
        .unwrap();
    let track = read["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == "move")
        .unwrap();
    assert_eq!(track["authoring"]["part_set"], true);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn untouched_empty_theatre_mount_and_saved_theatre_keyframes_sample_and_export() {
    let (mut s, root, rig) = fixture();
    let mut p = params(&s, &rig);
    p["time"] = json!(0.5);
    p["theatre_state"] = json!({"definitionVersion":"0.4.0","revisionHistory":[],"sheetsById":{
        "subject / move":{"sequence":{"type":"PositionalSequence","length":10,"subUnitsPerUnit":30,"tracksByObject":{}},"staticOverrides":{"byObject":{}}}
    }});
    let pose = s
        .execute("motion.pose.export", p.clone())
        .expect("an untouched mounted editor must export");
    assert_eq!(pose["approval"], "pending");
    p["theatre_state"]["sheetsById"]["subject / move"]["staticOverrides"]["byObject"]["body"] =
        json!({"rotation":0});
    assert!(
        s.execute("motion.pose.export", p).is_err(),
        "unsaved Theatre edits that differ from disk must be refused until saved"
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn pose_rest_and_amount_match_the_visible_shared_sample() {
    let (mut s, root, rig) = fixture();
    let mut edit = params(&s, &rig);
    edit["position"] = json!([6, 4]);
    s.execute("motion.part.set", edit).unwrap();
    let mut p = params(&s, &rig);
    p["time"] = json!(0.5);
    p["theatre_state"] = Value::Null;
    p["amount"] = json!(50);
    let pose = s.execute("motion.pose.export", p.clone()).unwrap();
    assert_eq!(
        pose["sample"]["frames"][0]["layers"][0]["position"],
        json!([3.0, 2.0])
    );
    p["rest"] = json!(true);
    let rest = s.execute("motion.pose.export", p).unwrap();
    let image = image::open(rest["path"].as_str().unwrap())
        .unwrap()
        .into_rgba8();
    assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 255]);
    assert_eq!(rest["rest"], true);
    assert_eq!(rest["amount"], 50.0);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn serialized_null_theatre_opens_as_unsaved_empty_state() {
    let (mut s, root, _) = fixture();
    let path = s.project_path.clone().unwrap();
    fs::write(
        &path,
        json!({"format":"magicstudio.project","version":1,"assets":[],"theatre":null}).to_string(),
    )
    .unwrap();
    s.execute("project.open", json!({"path":path,"record_recent":false}))
        .unwrap();
    assert!(
        s.theatre_state.is_none(),
        "JSON null is not a native save file"
    );
    let read = s.execute("motion.theatre.read", json!({})).unwrap();
    assert_eq!(read["status"], "no_saved_state");
    s.theatre_state = Some(Value::Null);
    let read = s.execute("motion.theatre.read", json!({})).unwrap();
    assert_eq!(read["saved"], false);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn stacking_order_changes_pixels_not_parent_transforms_and_has_guarded_undo() {
    let (mut s, root, rig) = fixture();
    let mut raw: Value = serde_json::from_slice(&fs::read(&rig).unwrap()).unwrap();
    let mut blue = image::RgbaImage::new(16, 16);
    for y in 0..4 {
        for x in 0..4 {
            blue.put_pixel(x, y, image::Rgba([0, 0, 255, 255]));
        }
    }
    blue.save(root.join("overlay.png")).unwrap();
    raw["layers"].as_array_mut().unwrap().push(
        json!({"id":"overlay","file":root.join("overlay.png"),"pivot":[2,2],"parent":"body"}),
    );
    fs::write(&rig, raw.to_string()).unwrap();
    let before = fs::read(&rig).unwrap();
    let mut p = params(&s, &rig);
    p["order"] = json!(["overlay", "body"]);
    p.as_object_mut().unwrap().remove("part");
    let changed = s
        .execute("motion.layers.set", p)
        .expect("shared stacking mutation");
    let meta = s
        .execute("motion.tracks", json!({"character":"subject"}))
        .unwrap();
    assert_eq!(meta["layer_order"], json!(["overlay", "body"]));
    assert_eq!(meta["layers"][0]["depth"], 0);
    assert_eq!(meta["layers"][0]["parent"], "body");
    let mut p = params(&s, &rig);
    p["time"] = json!(0.0);
    p["theatre_state"] = Value::Null;
    let pose = s.execute("motion.pose.export", p).unwrap();
    let image = image::open(pose["path"].as_str().unwrap())
        .unwrap()
        .into_rgba8();
    assert_eq!(image.get_pixel(2, 2).0, [255, 0, 0, 255]);
    let mut p = params(&s, &rig);
    p["order"] = json!(["body", "body"]);
    assert!(s.execute("motion.layers.set", p).is_err());
    let mut p = params(&s, &rig);
    p["undo"] = changed["undo"].clone();
    s.execute("motion.edit.undo", p).unwrap();
    assert_eq!(fs::read(&rig).unwrap(), before);
    let mut p = params(&s, &rig);
    p["time"] = json!(0.0);
    p["theatre_state"] = Value::Null;
    let pose = s.execute("motion.pose.export", p).unwrap();
    let image = image::open(pose["path"].as_str().unwrap())
        .unwrap()
        .into_rgba8();
    assert_eq!(image.get_pixel(2, 2).0, [0, 0, 255, 255]);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn saved_theatre_keyframes_evaluate_in_shared_sampler_and_retime_with_duration() {
    let (mut s,root,rig)=fixture();
    let state=json!({"definitionVersion":"0.4.0","revisionHistory":["r1"],"sheetsById":{
        "subject / move":{"sequence":{"type":"PositionalSequence","length":1.0,"subUnitsPerUnit":30,"tracksByObject":{"body":{"trackData":{"rotation":{"keyframes":[{"position":0.0,"value":0.0},{"position":1.0,"value":20.0}]}}}}},"staticOverrides":{"byObject":{}}}
    }});
    fs::write(s.project_path.as_ref().unwrap(),json!({"theatre":state}).to_string()).unwrap();
    s.theatre_state=Some(state);
    let sample=s.execute("motion.sample",json!({"character":"subject","track":"move","time":0.5})).unwrap();
    assert_eq!(sample["frames"][0]["layers"][0]["rotation"],json!(10.0));
    let mut edit=params(&s,&rig);
    edit["duration"]=json!(4.0);
    let res=s.execute("motion.action.set",edit).unwrap();
    assert_eq!(res["ok"],true);
    let sample2=s.execute("motion.sample",json!({"character":"subject","track":"move","time":2.0})).unwrap();
    assert_eq!(sample2["duration"],json!(4.0));
    assert_eq!(sample2["frames"][0]["layers"][0]["rotation"],json!(10.0));
    fs::remove_dir_all(root).unwrap();
}
