use animationcraft_engine::Session;
use serde_json::json;
#[test]
fn hyperframes_init_must_not_claim_a_nonexistent_project(){
 let dir=std::env::temp_dir().join(format!("ac-video-init-{}",std::process::id()));std::fs::create_dir_all(dir.join("hyperframes")).unwrap();
 let mut session=Session::default();session.execute("project.new",json!({"path":dir.join("Fixture.acproj")})).unwrap();session.video_target="hyperframes".into();
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
fn official_initializer_outputs_are_merged_without_overwriting_existing_material() {
 let dir=std::env::temp_dir().join(format!("ac-video-official-{}",std::process::id()));std::fs::create_dir_all(dir.join("hyperframes")).unwrap();
 let mut s=Session::default();s.execute("project.new",json!({"path":dir.join("Fixture.acproj")})).unwrap();
 let captions=dir.join("hyperframes/transcript.srt");std::fs::write(&captions,"Keep original timing").unwrap();
 let cli=runtime(&dir,"[ \"$1\" = init ] && [ \"$3\" = --non-interactive ] && [ \"$4\" = --skip-transcribe ] && [ \"$HYPERFRAMES_SKIP_SKILLS\" = 1 ] || exit 9\nmkdir -p \"$2\"\nprintf '%s' '{}' > \"$2/hyperframes.json\"\nprintf '%s' '<div data-composition-id=fixture></div>' > \"$2/index.html\"");
 let r=s.execute("video.init",json!({"target":"hyperframes","hyperframes_cli":cli})).unwrap();
 assert!(std::path::Path::new(r["initialized"].as_str().unwrap()).is_file());assert!(dir.join("hyperframes/index.html").is_file());
 assert_eq!(r["video_complete"],false);assert_eq!(std::fs::read_to_string(captions).unwrap(),"Keep original timing");
 // Idempotency must not reinitialize or modify the existing document.
 std::fs::write(&cli,"#!/bin/sh\nexit 9\n").unwrap();
 let repeated=s.execute("video.init",json!({"target":"hyperframes","hyperframes_cli":cli})).unwrap();assert_eq!(repeated["reused_existing_project"],true);
 std::fs::remove_dir_all(dir).unwrap();
}
#[test]
#[cfg(unix)]
fn initializer_success_without_files_is_failure_and_existing_files_survive() {
 let dir=std::env::temp_dir().join(format!("ac-video-empty-{}",std::process::id()));std::fs::create_dir_all(dir.join("hyperframes")).unwrap();
 let mut s=Session::default();s.execute("project.new",json!({"path":dir.join("Fixture.acproj")})).unwrap();
 let captions=dir.join("hyperframes/transcript.srt");std::fs::write(&captions,"Keep").unwrap();let cli=runtime(&dir,"exit 0");
 let r=s.execute("video.init",json!({"target":"hyperframes","hyperframes_cli":cli}));assert!(r.is_err(),"exit0 alone is not initialization");assert_eq!(std::fs::read_to_string(captions).unwrap(),"Keep");
 std::fs::remove_dir_all(dir).unwrap();
}
