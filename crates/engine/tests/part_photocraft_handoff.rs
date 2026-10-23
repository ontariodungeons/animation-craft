//! `part.open_photocraft` resolves one keyed-rig part's PNG (or its retained
//! original) for the PhotoCraft handoff. `spawn` stays false so no window opens.
use magicstudio_engine::Session;
use image::{ImageBuffer, Rgba};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn sha(path: &Path) -> String {
    format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()))
}

/// A keyed two-part rig in a temp project, opened and loaded.
fn fixture(tag: &str) -> (Session, PathBuf) {
    let root = std::env::temp_dir().join(format!("ac-part-photocraft-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    ImageBuffer::from_pixel(8, 6, Rgba::<u8>([40, 50, 60, 255])).save(root.join("source.png")).unwrap();
    ImageBuffer::from_pixel(3, 2, Rgba::<u8>([10, 20, 30, 255])).save(root.join("antenna-l.png")).unwrap();
    ImageBuffer::from_pixel(2, 2, Rgba::<u8>([45, 55, 65, 255])).save(root.join("body.png")).unwrap();
    let rig = root.join("rig.json");
    std::fs::write(&rig, serde_json::to_vec_pretty(&json!({"source":"source.png","canvas":[8,6],
        "layer_order":["antenna-l","body"],"layers":{
        "antenna-l":{"box":[1,1,3,2],"pivot":[2.0,1.5]},"body":{"box":[4,1,2,2],"pivot":[5.0,2.0]}}})).unwrap()).unwrap();
    let project = root.join("project.magic");
    std::fs::write(&project, serde_json::to_vec_pretty(&json!({"format":"magicstudio.project","version":1,"name":"Fixture",
        "assets":[{"id":"bee","name":"Bee","preset":null,"path":rig,"kind":"cutout_rig","parts":[]}],"workshop":{}})).unwrap()).unwrap();
    let mut session = Session::default();
    session.cwd = root.clone();
    session.execute("project.open", json!({"path":project})).unwrap();
    session.execute("project.load_asset", json!({"id":"bee"})).unwrap();
    (session, root)
}

fn open(session: &mut Session, params: Value) -> Value {
    session.execute("part.open_photocraft", params).unwrap()
}

#[test]
fn current_part_reports_file_size_and_digest() {
    let (mut session, root) = fixture("current");
    let file = root.join("antenna-l.png");
    let result = open(&mut session, json!({"part":"antenna-l"}));
    assert_eq!(result["ok"], true);
    assert_eq!(result["command"], "part.open_photocraft");
    assert_eq!(result["part"], "antenna-l");
    assert_eq!(result["which"], "current", "which defaults to current");
    assert_eq!(result["file"], json!(file));
    assert!(result["original_file"].is_null(), "no replacement yet, so no retained original");
    assert_eq!((result["width"].as_u64(), result["height"].as_u64()), (Some(3), Some(2)));
    assert_eq!(result["sha256"], sha(&file));
    assert_eq!(result["spawned"], false);
    assert!(result["photocraft_cli"].is_null() || result["photocraft_cli"].is_string());

    let body = open(&mut session, json!({"part":"body","spawn":false}));
    assert_eq!(body["file"], json!(root.join("body.png")));
    assert_eq!((body["width"].as_u64(), body["height"].as_u64()), (Some(2), Some(2)));

    let fallback = open(&mut session, json!({"part":"antenna-l","which":"original"}));
    assert_eq!(fallback["file"], json!(file), "original without a backup is the layer PNG");
    assert_eq!(fallback["which"], "original");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn invalid_part_and_params_are_rejected() {
    let (mut session, root) = fixture("invalid");
    let error = session.execute("part.open_photocraft", json!({"part":"antenna-x"})).unwrap_err().to_string();
    assert!(error.contains("not in this rig"), "{error}");
    assert!(session.execute("part.open_photocraft", json!({})).is_err(), "part is required");
    assert!(session.execute("part.open_photocraft", json!({"part":"antenna-l","which":"older"})).is_err());
    assert!(session.execute("part.open_photocraft", json!({"part":"antenna-l","spawn":"yes"})).is_err(), "spawn must be a boolean");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn original_resolves_retained_backup_after_replacement() {
    let (mut session, root) = fixture("original");
    let file = root.join("antenna-l.png");
    let first_hash = sha(&file);
    let replace = |session: &mut Session, candidate: &Path, revision: u64| {
        session.execute("part.replace_image", json!({"asset":"bee","part":"antenna-l","candidate":candidate,
            "expected_rig_sha256":sha(&root.join("rig.json")),"expected_part_sha256":sha(&file),
            "expected_candidate_sha256":sha(candidate),"expected_revision":revision})).unwrap()
    };
    let candidate = root.join("candidate.png");
    ImageBuffer::from_pixel(3, 2, Rgba::<u8>([80, 100, 120, 190])).save(&candidate).unwrap();
    replace(&mut session, &candidate, 0);
    let candidate2 = root.join("candidate2.png");
    ImageBuffer::from_pixel(3, 2, Rgba::<u8>([1, 2, 3, 200])).save(&candidate2).unwrap();
    replace(&mut session, &candidate2, 1);

    let current = open(&mut session, json!({"part":"antenna-l","which":"current"}));
    assert_eq!(current["file"], json!(file));
    assert_eq!(current["sha256"], sha(&candidate2));
    let backup = current["original_file"].as_str().expect("retained original is reported");
    assert!(backup.contains(".part-image-history") && backup.ends_with("before.png"));

    let original = open(&mut session, json!({"part":"antenna-l","which":"original"}));
    assert_eq!(original["file"].as_str(), Some(backup));
    assert_eq!(original["sha256"], first_hash, "original is the pre-first-replacement image");
    assert_eq!((original["width"].as_u64(), original["height"].as_u64()), (Some(3), Some(2)));

    let body = open(&mut session, json!({"part":"body","which":"original"}));
    assert!(body["original_file"].is_null(), "other parts' history does not leak");
    assert_eq!(body["file"], json!(root.join("body.png")));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn command_is_registered_as_unjournaled_parts_menu_item() {
    let spec = magicstudio_engine::commands::find("part.open_photocraft").unwrap();
    assert_eq!(spec.label, "Open Part in PhotoCraft");
    assert_eq!(spec.menu, ["Animation", "Parts"]);
    assert!(!spec.journal);
}
