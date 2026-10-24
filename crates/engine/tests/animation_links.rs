use magicstudio_engine::Session;
use serde_json::json;
use std::{fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

fn fixture() -> (Session, PathBuf) {
    let root=std::env::temp_dir().join(format!("animation-links-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&root).unwrap();
    let mut s=Session::default();
    s.execute("project.new",json!({"path":root.join("generic.magic")})).unwrap();
    fs::write(root.join("source.sif"),"<canvas/>").unwrap();
    (s,root)
}

#[test]
fn source_registration_is_revision_guarded_and_survives_explicit_project_save() {
    let (mut s,root)=fixture();
    let project=s.project_path.clone().unwrap();
    let p=json!({"expected_project":project,"id":"source-one","backend":"synfig","source":root.join("source.sif"),"expected_revision":0});
    let before=fs::read(&project).unwrap();
    let added=s.execute("animation.source.set",p.clone()).unwrap();
    assert_eq!(added["source"]["revision"],1);
    assert_eq!(added["project_saved"],false);
    assert_eq!(fs::read(&project).unwrap(),before,"link registration never silently saves project");
    let snapshot=s.execute("animation.sources",json!({})).unwrap();
    assert_eq!(snapshot["unsaved"],true);
    assert!(s.execute("animation.source.set",p.clone()).is_err());
    assert_eq!(s.execute("animation.sources",json!({})).unwrap(),snapshot);
    let mut wrong=p.clone();wrong["expected_project"]=json!(root.join("other.magic"));wrong["expected_revision"]=json!(1);
    assert!(s.execute("animation.source.set",wrong).is_err());
    assert!(s.execute("project.save",json!({"expected_project":root.join("other.magic")})).is_err());
    assert_eq!(fs::read(&project).unwrap(),before);
    s.execute("project.save",json!({"expected_project":project})).unwrap();
    assert_eq!(s.execute("animation.sources",json!({})).unwrap()["unsaved"],false);
    let mut reopened=Session::default();
    reopened.execute("project.open",json!({"path":project})).unwrap();
    let restored=reopened.execute("animation.sources",json!({})).unwrap();
    assert_eq!(restored["sources"],snapshot["sources"]);
    assert_eq!(restored["unsaved"],false);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn registered_render_cannot_override_source_or_submit_a_stale_revision() {
    let (mut s,root)=fixture();
    let project=s.project_path.clone().unwrap();
    s.execute("animation.source.set",json!({"expected_project":project,"id":"source-one","backend":"synfig","source":root.join("source.sif"),"expected_revision":0})).unwrap();
    for options in [json!({"source":"/other.sif"}),json!({"bundle":"/"}),json!({"source_id":"another"}),json!({"character":"other"})] {
        let e=s.execute("animation.source.render",json!({"expected_project":project,"id":"source-one","expected_revision":1,"options":options})).unwrap_err().to_string();
        assert!(e.contains("Unsupported render option"),"{e}");
    }
    let e=s.execute("animation.source.render",json!({"expected_project":project,"id":"source-one","expected_revision":0,"options":{}})).unwrap_err().to_string();
    assert!(e.contains("revision"),"{e}");
    assert!(!root.join("animation").exists(),"invalid requests must not submit any worker");
    fs::remove_dir_all(root).unwrap();
}
