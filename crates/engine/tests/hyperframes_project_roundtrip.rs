use magicstudio_engine::Session;
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

fn fixture(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("magic-hyperframes-{label}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn contained_hyperframes_and_assets_survive_save_reopen_and_folder_move() {
    let root = fixture("roundtrip");
    let original = root.join("original");
    let hf = original.join("hyperframes");
    fs::create_dir_all(hf.join("assets")).unwrap();
    let project = original.join("Song.magic");
    let audio = hf.join("assets/song.wav");
    fs::write(&audio, b"contained audio fixture").unwrap();
    fs::write(hf.join("hyperframes.json"), "{}").unwrap();
    let html = "<div data-composition-id=\"main\">Saved Studio edit</div>";
    fs::write(hf.join("index.html"), html).unwrap();
    image::RgbaImage::new(2, 2).save(original.join("art.png")).unwrap();
    let mut session = Session::default();
    session.execute("project.new", json!({"path":project})).unwrap();
    session.execute("project.add_asset", json!({"id":"art", "path":original.join("art.png")})).unwrap();
    session.song_audio = Some(audio);
    session.video_project_path = Some(hf.join("hyperframes.json"));
    session.execute("project.save", json!({})).unwrap();
    let saved: Value = serde_json::from_slice(&fs::read(&project).unwrap()).unwrap();
    assert!(!magicstudio_engine::project::project_is_dirty(&session, &saved));
    assert_eq!(saved["workshop"]["video_project"], "hyperframes/hyperframes.json");
    assert_eq!(saved["workshop"]["audio"], "hyperframes/assets/song.wav");
    assert_eq!(saved["assets"][0]["path"], "art.png");
    drop(session);
    let moved = root.join("moved");
    fs::rename(&original, &moved).unwrap();
    let mut reopened = Session::default();
    let result = reopened.execute("project.open", json!({"path":moved.join("Song.magic")})).unwrap();
    assert_eq!(result["missing"], json!([]));
    assert_eq!(reopened.song_audio, Some(moved.join("hyperframes/assets/song.wav")));
    assert_eq!(reopened.video_project_path, Some(moved.join("hyperframes/hyperframes.json")));
    reopened.execute("project.save", json!({})).unwrap();
    let saved: Value=serde_json::from_slice(&fs::read(moved.join("Song.magic")).unwrap()).unwrap();
    assert!(!magicstudio_engine::project::project_is_dirty(&reopened, &saved));
    assert_eq!(fs::read_to_string(moved.join("hyperframes/index.html")).unwrap(), html);
    assert_eq!(fs::read(moved.join("hyperframes/assets/song.wav")).unwrap(), b"contained audio fixture");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn legacy_absolute_connections_still_open_and_external_files_are_preserved() {
    let root = fixture("legacy");
    let external = root.join("external.wav");
    fs::write(&external, b"external original").unwrap();
    let project = root.join("project/Legacy.magic");
    fs::create_dir_all(project.parent().unwrap()).unwrap();
    fs::write(&project, json!({"format":"magicstudio.project", "assets":[], "workshop":{"audio":external,"target":"hyperframes"}}).to_string()).unwrap();
    let mut session = Session::default();
    session.execute("project.open", json!({"path":project})).unwrap();
    assert_eq!(session.song_audio, Some(external.clone()));
    session.execute("project.save", json!({})).unwrap();
    assert_eq!(fs::read(external).unwrap(), b"external original");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_and_storyboard_read_the_contained_transcript_and_preserve_legacy_files() {
    let root = fixture("native-transcript");
    let project = root.join("Song.magic");
    let mut session = Session::default();
    session.execute("project.new", json!({"path":project})).unwrap();
    fs::create_dir_all(root.join("hyperframes")).unwrap();
    let legacy = root.join("song.transcript.json");
    fs::write(&legacy, b"legacy source retained for recovery").unwrap();
    let supplied = root.join("received-words.json");
    fs::write(&supplied, json!([{"id":"w0","text":"Actual","start":0.1,"end":0.3},{"id":"w1","text":"lyrics","start":0.4,"end":0.8}]).to_string()).unwrap();
    session.execute("workshop.import_transcript", json!({"path":supplied})).unwrap();
    let saved: Value = serde_json::from_slice(&fs::read(root.join("hyperframes/transcript.json")).unwrap()).unwrap();
    assert_eq!(saved[0]["text"], "Actual");
    assert_eq!(saved[1]["end"], 0.8);
    drop(session);
    let mut reopened = Session::default();
    reopened.execute("project.open", json!({"path":project})).unwrap();
    let status = reopened.execute("workshop.status", json!({})).unwrap();
    let transcript = status["stages"].as_array().unwrap().iter().find(|stage|stage["id"]=="transcript").unwrap();
    assert_eq!(transcript["document"]["words"][0]["text"], "Actual");
    let board = reopened.execute("storyboard.toc", json!({})).unwrap();
    assert!(board.to_string().contains("Actual"), "{board}");
    assert_eq!(fs::read(legacy).unwrap(), b"legacy source retained for recovery");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn imported_timing_refreshes_only_unedited_generated_caption_sources() {
    let root = fixture("caption-handoff");
    let mut session = Session::default();
    session.execute("project.new", json!({"path":root.join("Song.magic")})).unwrap();
    let hf=root.join("hyperframes");fs::create_dir_all(&hf).unwrap();
    fs::write(hf.join("hyperframes.json"), "{}").unwrap();
    fs::write(hf.join("index.html"), "<body><div data-composition-id=\"main\"></div></body>").unwrap();
    let supplied=root.join("words.json");
    fs::write(&supplied,json!([{"text":"First","start":0.1,"end":0.5}]).to_string()).unwrap();
    let result=session.execute("workshop.import_transcript",json!({"path":supplied})).unwrap();
    assert_eq!(result["captions"]["ready"],true);
    let caption=hf.join("compositions/captions.html");
    assert!(fs::read_to_string(&caption).unwrap().contains("First"));
    fs::write(&supplied,json!([{"text":"Second","start":0.2,"end":0.7}]).to_string()).unwrap();
    session.execute("workshop.import_transcript",json!({"path":supplied,"replace":true})).unwrap();
    assert!(fs::read_to_string(&caption).unwrap().contains("Second"));
    fs::write(&caption,"Authored caption source must survive").unwrap();
    fs::write(&supplied,json!([{"text":"Third","start":0.3,"end":0.8}]).to_string()).unwrap();
    assert!(session.execute("workshop.import_transcript",json!({"path":supplied,"replace":true})).is_err());
    assert_eq!(fs::read_to_string(caption).unwrap(),"Authored caption source must survive");
    assert!(!root.join("song.transcript.json").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_project_file_alone_does_not_prove_audio_and_captions_are_synchronized() {
    let root=fixture("readiness");
    let mut session=Session::default();
    session.execute("project.new",json!({"path":root.join("Song.magic")})).unwrap();
    let hf=root.join("hyperframes");fs::create_dir_all(&hf).unwrap();
    fs::write(hf.join("hyperframes.json"),"{}").unwrap();
    fs::write(hf.join("index.html"),"<div>No captions</div>").unwrap();
    fs::write(hf.join("audiomap.json"),"{}").unwrap();
    fs::write(hf.join("transcript.json"),json!([{"text":"Word","start":0.1,"end":0.5}]).to_string()).unwrap();
    let audio=root.join("audio.wav");fs::write(&audio,b"fixture").unwrap();
    session.song_audio=Some(audio);session.video_project_path=Some(hf.join("hyperframes.json"));
    let status=session.execute("center.status",json!({})).unwrap();
    let caption_status=status["prerequisites"].as_array().unwrap().iter().find(|p|p["id"]=="hyperframes").unwrap();
    assert_ne!(caption_status["state"],"ready","{caption_status}");
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[cfg(unix)]
fn transcription_uses_original_audio_and_requested_model_without_retired_skill() {
    use std::os::unix::fs::PermissionsExt;
    let root = fixture("transcribe");
    let project = root.join("Song.magic");
    let audio = root.join("original.wav");
    fs::write(&audio, b"original recording fixture").unwrap();
    let cli = root.join("hyperframes");
    fs::create_dir_all(&cli).unwrap();
    let runtime = root.join("hf-cli");
    // Fake only the external ASR boundary. The real engine imports and validates its output.
    fs::write(&runtime, r##"#!/bin/sh
if [ "$1" = --version ]; then echo fixture; exit 0; fi
printf '%s\n' "$*" >> calls.txt
if [ "$1" = transcribe ] && [ "$3" = -d ]; then
  test "$2" = "$(dirname "$PWD")/original.wav" || exit 71
  printf '%s' '[{"id":"w0","text":"Hello","start":0.1,"end":0.5}]' > "$4/transcript.json"
  echo '{"ok":true,"engine":"fixture","model":"large-v3","wordCount":1}'
elif [ "$1" = transcribe ] && [ "$3" = --to ]; then
  printf '1\n00:00:00,100 --> 00:00:00,500\nHello\n\n' > "$6"
else exit 72
fi
"##).unwrap();
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
    let mut session = Session::default();
    session.execute("project.new", json!({"path":project})).unwrap();
    session.song_audio = Some(audio.clone());
    let result = session.execute("audio.analyze", json!({"hyperframes_cli":runtime,"model":"large-v3","analyze_beats":false})).unwrap();
    assert_eq!(result["ok"], true);
    assert_eq!(result["words"], 1);
    let calls = fs::read_to_string(cli.join("calls.txt")).unwrap();
    assert!(calls.contains("--model large-v3"), "{calls}");
    assert!(!calls.contains("--no-runtime-install"), "missing prerequisites must be installed by HyperFrames");
    assert!(!cli.join("assets/bgm.mp3").exists());
    assert!(!cli.join("STORYBOARD.md").exists());
    assert_eq!(fs::read(audio).unwrap(), b"original recording fixture");
    assert!(root.join("hyperframes/transcript.json").is_file());
    assert!(!root.join("song.transcript.json").exists());
    fs::remove_dir_all(root).unwrap();
}
