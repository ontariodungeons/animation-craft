use magicstudio_engine::Session;
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
    let project = d.join("case/project/Case.magic");
    let mut s = Session::default();
    let r = s
        .execute("project.intake", json!({"path":project,"transcript":input}))
        .unwrap();
    assert_eq!(r["ok"], true);
    assert_eq!(r["intake_ready"], false);
    assert_eq!(r["storyboard_ready"], false);
    assert!(!r["steps"].as_array().unwrap().iter().any(|v|v["command"]=="audio.analyze"));
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
            json!({"path":d.join("case/project/Case.magic"),"transcript":raw,"audiomap":map}),
        )
        .unwrap();
    assert_eq!(r["intake_ready"], true);
    assert_eq!(r["storyboard_ready"], true);
    assert!(PathBuf::from(r["storyboard"]["deck"].as_str().unwrap()).is_file());
    assert!(!r["steps"].as_array().unwrap().iter().any(|v|v["command"]=="audio.analyze"));
    let imported: serde_json::Value =
        serde_json::from_slice(&fs::read(d.join("case/project/case.transcript.json")).unwrap())
            .unwrap();
    assert_eq!(imported["words"], words);
    let deck=PathBuf::from(r["storyboard"]["deck"].as_str().unwrap());
    let before=fs::read(&deck).unwrap();
    let resumed=s.execute("project.intake",json!({"path":d.join("case/project/Case.magic"),"resume":true})).unwrap();
    assert_eq!(resumed["storyboard_ready"],true);
    assert_eq!(resumed["storyboard"]["reused_existing_document"],true);
    assert!(!resumed["steps"].as_array().unwrap().iter().any(|v|v["command"]=="audio.analyze"));
    assert_eq!(fs::read(deck).unwrap(),before);
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
    let p = d.join("case/project/Case.magic");
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

#[test]
fn audio_only_intake_attempts_existing_pipeline_and_retains_recoverable_failure() {
    let d=fixture("bad-audio");
    let audio=d.join("invalid.wav");
    fs::write(&audio,b"not audio").unwrap();
    let project=d.join("case/project/Case.magic");
    let mut s=Session::default();
    let r=s.execute("project.intake",json!({"path":project,"audio":audio})).unwrap();
    assert_eq!(r["intake_ready"],false);
    assert_eq!(r["storyboard_ready"],false);
    let step=r["steps"].as_array().unwrap().iter().find(|v|v["command"]=="audio.analyze").expect("intake must attempt the shared audio pipeline");
    assert_eq!(step["status"],"failed");
    assert!(step["error"].as_str().unwrap().len()>0);
    assert_eq!(r["next_commands"][0]["command"],"audio.analyze");
    assert_eq!(fs::read(audio).unwrap(),b"not audio");
    assert!(project.is_file());
    let mut reopened=Session::default();
    reopened.execute("project.open",json!({"path":project})).unwrap();
    let state=reopened.execute("center.status",json!({})).unwrap();
    assert!(!state["problems"].as_array().unwrap().is_empty());
}

#[test]
fn shared_observation_does_not_confuse_files_or_saved_receipts_with_readiness() {
    let d = fixture("observed-prerequisites");
    let mut s=Session::default();
    s.execute("project.new",json!({"path":d.join("Observed.magic")})).unwrap();
    fs::write(d.join("intake-state.json"),json!({"intake_ready":true,"storyboard_ready":true,"video_ready":true}).to_string()).unwrap();
    let observed=s.execute("center.status",json!({})).unwrap();
    let find=|id:&str|observed["prerequisites"].as_array().unwrap().iter().find(|v|v["id"]==id).unwrap();
    assert_eq!(find("song")["state"],"missing");
    assert_eq!(find("transcript")["state"],"missing");
    assert_eq!(find("deckcraft")["state"],"missing");
    assert_eq!(find("storyboard")["state"],"unknown");
    assert_eq!(find("hyperframes")["state"],"missing");
    let art=observed["next_commands"].as_array().unwrap().iter().find(|v|v["params"]["filter"]=="project.add_asset").unwrap();
    assert_eq!(art["command"],"command.list");assert_eq!(art["operation"],"exec");assert_eq!(art["label"],"See asset import arguments");assert!(art["reason"].as_str().unwrap().starts_with("No reference art is registered"));
    assert_eq!(observed["next_commands"].as_array().unwrap().iter().filter(|v|v["params"]["filter"]=="project.add_asset").count(),1,"Do not recommend optional rig work before reference art exists");
    assert_eq!(observed["metrics"]["timed_words"],0);
    assert!(observed["observation"]["id"].is_string());
    assert_eq!(observed["studio_acknowledgement"],"unobserved; query ui.feedback separately");
    let transcript=d.join("observed.transcript.json");
    fs::write(&transcript,b"{\"words\":[{\"text\":\"bad\",\"start\":2,\"end\":1}]}").unwrap();
    let bad=s.execute("center.status",json!({})).unwrap();
    assert_eq!(bad["prerequisites"].as_array().unwrap().iter().find(|v|v["id"]=="transcript").unwrap()["state"],"blocked");
    assert!(!bad["next_commands"].as_array().unwrap().is_empty());
    fs::remove_dir_all(d).unwrap();
}
