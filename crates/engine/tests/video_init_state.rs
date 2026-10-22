use animationcraft_engine::Session;
use serde_json::json;
#[test]
fn hyperframes_init_must_not_claim_a_nonexistent_project(){
 let dir=std::env::temp_dir().join(format!("ac-video-init-{}",std::process::id()));std::fs::create_dir_all(dir.join("hyperframes")).unwrap();
 let mut session=Session::default();session.execute("project.new",json!({"path":dir.join("Fixture.acproj")})).unwrap();session.video_target="hyperframes".into();
 let captions=dir.join("hyperframes/transcript.srt");std::fs::write(&captions,"Original timed captions\n").unwrap();
 let result=session.execute("video.init",json!({"target":"hyperframes"}));
 assert!(result.is_err(),"A directory and nonexistent filename are not an initialized video: {result:?}");
 assert!(result.unwrap_err().to_string().contains("did not create an editable HyperFrames project"));
 assert_eq!(std::fs::read_to_string(&captions).unwrap(),"Original timed captions\n");assert!(!dir.join("hyperframes/hyperframes.json").exists());
 std::fs::remove_dir_all(dir).unwrap();
}
