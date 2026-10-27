use magicstudio_engine::Session;
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

fn fixture() -> (PathBuf, Session) {
    let dir = std::env::temp_dir().join(format!(
        "magicstudio-intake-status-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    let mut session = Session::default();
    session.execute("project.new", json!({"path":dir.join("Example.magic")})).unwrap();
    (dir, session)
}

fn transcript_stage(session: &mut Session) -> Value {
    session.execute("workshop.status", json!({})).unwrap()["stages"]
        .as_array().unwrap().iter().find(|v| v["id"] == "transcript").unwrap().clone()
}

#[test]
fn workshop_validates_transcript_bytes_and_retains_invalid_inputs_after_reopen() {
    let (dir, mut session) = fixture();
    assert_eq!(transcript_stage(&mut session)["done"], false);
    let path = dir.join("example.transcript.json");
    for bytes in [
        "[broken".to_string(),
        json!({"words":[{"text":"bad","start":2,"end":1}],"paragraphs":[{"text":"bad","start":2,"end":1}]}).to_string(),
    ] {
        fs::write(&path, &bytes).unwrap();
        let stage = transcript_stage(&mut session);
        assert_eq!(stage["done"], false, "{stage}");
        assert!(stage["why"].as_str().unwrap().contains("repair"));
        assert!(!stage["problems"].as_array().unwrap().is_empty());
        assert_eq!(fs::read_to_string(&path).unwrap(), bytes);
    }
    session.execute("project.save", json!({})).unwrap();
    let mut reopened = Session::default();
    reopened.execute("project.open", json!({"path":dir.join("Example.magic")})).unwrap();
    assert_eq!(transcript_stage(&mut reopened)["done"], false);
    let words = json!([{"id":"w0","text":"again","start":0.0,"end":0.5}]);
    fs::write(&path, json!({"words":words,"paragraphs":words}).to_string()).unwrap();
    let stage = transcript_stage(&mut reopened);
    assert_eq!(stage["done"], true, "{stage}");
    assert_eq!(stage["document"]["words"], words);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn workshop_reads_native_board_source_and_rejects_stale_or_unvalidated_timing() {
    let (dir, mut session) = fixture();
    let source = dir.join("native.json");
    let words = json!([{"id":"w0","text":"hello","start":0.0,"end":1.0}]);
    fs::write(&source, words.to_string()).unwrap();
    fs::create_dir_all(dir.join("storyboard")).unwrap();
    let board = dir.join("storyboard/movie-board.json");
    fs::write(&board, json!({"source_transcript":source,"timing_valid":true}).to_string()).unwrap();
    let stage = transcript_stage(&mut session);
    assert_eq!(stage["done"], true, "{stage}");
    assert_eq!(stage["document"]["words"], words);
    fs::write(&board, json!({"source_transcript":source,"timing_valid":false,"timing_warning":"Recording overrun"}).to_string()).unwrap();
    let stage = transcript_stage(&mut session);
    assert_eq!(stage["done"], false);
    assert!(stage["why"].as_str().unwrap().contains("Recording overrun"));
    fs::remove_file(source).unwrap();
    fs::write(&board, json!({"source_transcript":dir.join("native.json"),"timing_valid":true}).to_string()).unwrap();
    assert_eq!(transcript_stage(&mut session)["done"], false);
    fs::remove_dir_all(dir).unwrap();
}
