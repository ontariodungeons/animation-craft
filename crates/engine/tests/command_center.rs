use animationcraft_engine::Session;
use serde_json::json;
use std::{fs, path::PathBuf};
fn fixture(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("ac-center-{}-{}", std::process::id(), name));
    fs::create_dir_all(&p).unwrap();
    p
}
#[test]
fn new_intake_retains_invalid_received_bytes_and_remains_openable() {
    let d = fixture("invalid");
    let input = d.join("broken.json");
    fs::write(&input, b"[broken").unwrap();
    let project = d.join("case/project/Case.acproj");
    let mut s = Session::default();
    let r = s
        .execute("project.intake", json!({"path":project,"transcript":input}))
        .unwrap();
    assert_eq!(r["ok"], true);
    assert_eq!(r["intake_ready"], false);
    assert!(r["problems"].as_array().unwrap().len() > 0);
    let received = r["received"][0]["path"].as_str().unwrap();
    assert_eq!(fs::read(received).unwrap(), b"[broken");
    let mut reopened = Session::default();
    assert_eq!(
        reopened
            .execute("project.open", json!({"path":project}))
            .unwrap()["ok"],
        true
    );
    let state = reopened.execute("center.status", json!({})).unwrap();
    assert_eq!(state["transcript"]["valid"], false);
    assert!(state["commands"].as_array().unwrap().len() > 20);
}
#[test]
fn hyperframes_words_import_without_srt_preserves_occurrences_and_end_tail() {
    let d = fixture("words");
    let raw = d.join("words.json");
    let map = d.join("audiomap.json");
    let words = json!([{"id":"w0","text":"again","start":0.39,"end":0.92},{"id":"w1","text":"again","start":1.0,"end":1.5}]);
    fs::write(&raw, words.to_string()).unwrap();
    fs::write(&map, json!({"audio":{"duration_sec":4.5}}).to_string()).unwrap();
    let mut s = Session::default();
    let r = s
        .execute(
            "project.intake",
            json!({"path":d.join("case/project/Case.acproj"),"transcript":raw,"audiomap":map}),
        )
        .unwrap();
    assert_eq!(r["intake_ready"], true);
    let imported: serde_json::Value =
        serde_json::from_slice(&fs::read(d.join("case/project/case.transcript.json")).unwrap())
            .unwrap();
    assert_eq!(imported["words"], words);
    let toc = s.execute("storyboard.init", json!({})).unwrap();
    assert_eq!(toc["ok"], true);
    let read = s.execute("storyboard.read", json!({"scene":3})).unwrap();
    assert_eq!(read["end"], 4.5);
}
#[test]
fn resumed_intake_requires_explicit_transcript_replacement() {
    let d = fixture("resume");
    let raw = d.join("words.json");
    fs::write(
        &raw,
        json!([{"text":"original","start":0.0,"end":1.0}]).to_string(),
    )
    .unwrap();
    let p = d.join("case/project/Case.acproj");
    let mut s = Session::default();
    s.execute("project.intake", json!({"path":p,"transcript":raw}))
        .unwrap();
    let dest = d.join("case/project/case.transcript.json");
    let before = fs::read(&dest).unwrap();
    fs::write(
        &raw,
        json!([{"text":"changed","start":0.0,"end":1.0}]).to_string(),
    )
    .unwrap();
    let r = s
        .execute(
            "project.intake",
            json!({"path":p,"resume":true,"transcript":raw}),
        )
        .unwrap();
    assert_eq!(r["intake_ready"], false);
    assert_eq!(fs::read(dest).unwrap(), before);
}
