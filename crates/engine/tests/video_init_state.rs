use magicstudio_engine::Session;
use serde_json::json;
#[test]
fn hyperframes_init_must_not_claim_a_nonexistent_project(){
 let dir=std::env::temp_dir().join(format!("ac-video-init-{}",std::process::id()));std::fs::create_dir_all(dir.join("hyperframes")).unwrap();
 let mut session=Session::default();session.execute("project.new",json!({"path":dir.join("Fixture.magic")})).unwrap();session.video_target="hyperframes".into();
 let captions=dir.join("hyperframes/transcript.srt");std::fs::write(&captions,"Original timed captions\n").unwrap();
 let result=session.execute("video.init",json!({"target":"hyperframes","hyperframes_cli":"/missing/hyperframes"}));
 assert!(result.is_err(),"A directory and nonexistent filename are not an initialized video: {result:?}");
 assert!(result.unwrap_err().to_string().contains("HyperFrames executable"));
 assert_eq!(std::fs::read_to_string(&captions).unwrap(),"Original timed captions\n");assert!(!dir.join("hyperframes/hyperframes.json").exists());
 std::fs::remove_dir_all(dir).unwrap();
}

#[cfg(unix)]
fn runtime(dir:&std::path::Path,body:&str)->std::path::PathBuf {
 use std::os::unix::fs::PermissionsExt;
 let file=dir.join("hyperframes-cli");std::fs::write(&file,format!("#!/bin/sh\n{body}\n")).unwrap();std::fs::set_permissions(&file,std::fs::Permissions::from_mode(0o700)).unwrap();file
}
#[test]
#[cfg(unix)]
fn initializer_success_without_files_is_failure_and_existing_files_survive() {
 let dir=std::env::temp_dir().join(format!("ac-video-empty-{}",std::process::id()));std::fs::create_dir_all(dir.join("hyperframes")).unwrap();
 let mut s=Session::default();s.execute("project.new",json!({"path":dir.join("Fixture.magic")})).unwrap();
 let captions=dir.join("hyperframes/transcript.srt");std::fs::write(&captions,"Keep").unwrap();let cli=runtime(&dir,"exit 0");
 let r=s.execute("video.init",json!({"target":"hyperframes","hyperframes_cli":cli}));assert!(r.is_err(),"exit0 alone is not initialization");assert_eq!(std::fs::read_to_string(captions).unwrap(),"Keep");
 std::fs::remove_dir_all(dir).unwrap();
}
#[test]
#[cfg(unix)]
fn requested_music_assembly_requires_the_documented_machine_plan() {
 let dir=std::env::temp_dir().join(format!("ac-video-plan-{}",std::process::id()));std::fs::create_dir_all(dir.join("hyperframes")).unwrap();
 let mut s=Session::default();s.execute("project.new",json!({"path":dir.join("Fixture.magic")})).unwrap();
 std::fs::write(dir.join("hyperframes/hyperframes.json"),"{}").unwrap();std::fs::write(dir.join("hyperframes/index.html"),"existing").unwrap();let cli=runtime(&dir,"exit 0");
 let result=s.execute("video.init",json!({"target":"hyperframes","hyperframes_cli":cli,"assemble":true}));
 assert!(result.is_err(),"An assembly request without the owning workflow plan must not silently succeed");assert_eq!(std::fs::read_to_string(dir.join("hyperframes/index.html")).unwrap(),"existing");
 std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn audio_analysis_rejects_missing_explicit_runtime_without_npx_fallback() {
 let dir=std::env::temp_dir().join(format!("ac-audio-runtime-{}",std::process::id()));std::fs::create_dir_all(&dir).unwrap();
 let mut s=Session::default();s.execute("project.new",json!({"path":dir.join("Fixture.magic")})).unwrap();s.video_target="hyperframes".into();
 let audio=dir.join("source.wav");std::fs::write(&audio,b"original fixture").unwrap();s.song_audio=Some(audio.clone());
 let error=s.execute("audio.analyze",json!({"hyperframes_cli":"/missing/explicit-hyperframes"})).unwrap_err().to_string();
 assert!(error.contains("HyperFrames executable"),"{error}");assert!(!dir.join("hyperframes/assets/bgm.mp3").exists());assert_eq!(std::fs::read(audio).unwrap(),b"original fixture");std::fs::remove_dir_all(dir).unwrap();
}
