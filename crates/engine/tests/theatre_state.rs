use magicstudio_engine::{ProjectAsset, Session};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

struct Fixture { root: PathBuf, project: PathBuf, rig: PathBuf, session: Session }
impl Fixture {
    fn new() -> Self {
        let root=std::env::temp_dir().join(format!("theatre-state-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&root).unwrap();
        let project=root.join("Bee.magic");let rig=root.join("original-rig.json");
        fs::write(&rig,b"{\"original\":\"rig and source coordinates stay unchanged\"}").unwrap();
        let mut session=Session::default();session.cwd=root.clone();
        session.execute("project.new",json!({"path":project})).unwrap();
        session.project_assets.push(ProjectAsset{id:"bee".into(),name:"Bee".into(),preset:None,path:Some(rig.clone()),kind:"cutout_rig".into(),parts:Vec::new()});
        session.execute("project.save",json!({})).unwrap();
        Self{root,project,rig,session}
    }
    fn save(&mut self,state:Value)->magicstudio_engine::Result<Value>{self.session.execute("motion.theatre.save",json!({"expected_project":self.project,"state":state}))}
}
impl Drop for Fixture {fn drop(&mut self){let _=fs::remove_dir_all(&self.root);}}
fn native_state()->Value {json!({"definitionVersion":"0.4.0","revisionHistory":["native-revision"],"sheetsById":{"bee":{"sequence":{"type":"PositionalSequence","length":10,"subUnitsPerUnit":30,"tracksByObject":{"antenna-l":{"trackData":{"native-track":{"type":"BasicKeyframedTrack","keyframes":[{"id":"native-key","type":"bezier","position":1.25,"value":12.125,"handles":[0.5,1,0.5,0],"connectedRight":true}]}},"trackIdByPropPath":{"[\"rotation\"]":"native-track"}}}},"staticOverrides":{"byObject":{}},"futureNativeSheetField":{"preserve":[true,null,123]}}},"futureNativeRootField":{"opaque":"retain me"}})}

#[test]
fn opaque_native_state_saves_reuses_and_reopens_without_touching_rig(){
    let mut f=Fixture::new();let rig_before=fs::read(&f.rig).unwrap();let state=native_state();
    let legacy:Value=serde_json::from_slice(&fs::read(&f.project).unwrap()).unwrap();assert!(legacy.get("theatre").is_none());
    let before=f.session.execute("motion.theatre.read",json!({})).unwrap();assert_eq!(before["status"],"no_saved_state");assert_eq!(before["available"],true);assert_eq!(before["saved"],false);
    let result=f.save(state.clone()).unwrap();assert_eq!(result["reused"],false);assert_eq!(result["state"],state);
    let saved=fs::read(&f.project).unwrap();let disk:Value=serde_json::from_slice(&saved).unwrap();assert_eq!(disk["theatre"],state);assert_eq!(disk["assets"],legacy["assets"]);assert_eq!(disk["workshop"],legacy["workshop"]);
    assert_eq!(f.save(state.clone()).unwrap()["reused"],true);assert_eq!(fs::read(&f.project).unwrap(),saved);
    // Release the original lease before reopening as a different engine session.
    f.session=Session::default();
    f.session.execute("project.open",json!({"path":f.project,"record_recent":false})).unwrap();
    assert_eq!(f.session.execute("motion.theatre.read",json!({"expected_project":f.project})).unwrap()["state"],state);
    f.session.execute("project.save",json!({})).unwrap();assert_eq!(fs::read(&f.project).unwrap(),saved);
    assert_eq!(fs::read(&f.rig).unwrap(),rig_before);
}

#[test]
fn invalid_missing_or_wrong_project_state_does_not_change_memory_or_disk(){
    let mut f=Fixture::new();f.save(native_state()).unwrap();let original=fs::read(&f.project).unwrap();let prior=f.session.theatre_state.clone();
    let mut bad_sheet=native_state();bad_sheet["sheetsById"]["fish"]=bad_sheet["sheetsById"]["bee"].clone();
    for state in [Value::Null,json!({}),json!({"definitionVersion":"future","revisionHistory":[],"sheetsById":{"bee":{}}}),json!({"definitionVersion":"0.4.0","revisionHistory":"wrong","sheetsById":{"bee":{}}}),json!({"definitionVersion":"0.4.0","revisionHistory":[],"sheetsById":{}}),bad_sheet]{
        assert!(f.save(state).is_err());assert_eq!(f.session.theatre_state,prior);assert_eq!(fs::read(&f.project).unwrap(),original);
    }
    for params in [json!({"state":native_state()}),json!({"expected_project":f.project}),json!({"expected_project":f.root.join("Missing.magic"),"state":native_state()})]{assert!(f.session.execute("motion.theatre.save",params).is_err());}
    let other=f.root.join("Other.magic");fs::write(&other,&original).unwrap();
    assert!(f.session.execute("motion.theatre.save",json!({"expected_project":other,"state":native_state()})).unwrap_err().to_string().contains("differs"));
    assert!(f.session.execute("motion.theatre.read",json!({"expected_project":other})).is_err());
    assert_eq!(f.session.theatre_state,prior);assert_eq!(fs::read(&f.project).unwrap(),original);assert_eq!(fs::read(&other).unwrap(),original);
    let mut empty=Session::default();let unavailable=empty.execute("motion.theatre.read",json!({})).unwrap();assert_eq!(unavailable["available"],false);assert_eq!(unavailable["status"],"unavailable");
    assert!(empty.execute("motion.theatre.save",json!({"expected_project":f.project,"state":native_state()})).is_err());
}

#[test]
fn unavailable_destination_preserves_session_state_and_existing_lease(){
    let mut f=Fixture::new();f.save(native_state()).unwrap();let prior=f.session.theatre_state.clone();let original=fs::read(&f.project).unwrap();
    // A path replaced by a directory is deterministically unwritable, even under privileged tests.
    let moved=f.root.join("original.magic");fs::rename(&f.project,&moved).unwrap();fs::create_dir(&f.project).unwrap();
    let mut changed=native_state();changed["revisionHistory"]=json!(["different-revision"]);
    assert!(f.save(changed).is_err());assert_eq!(f.session.theatre_state,prior);assert_eq!(fs::read(&moved).unwrap(),original);
    fs::remove_dir(&f.project).unwrap();fs::rename(&moved,&f.project).unwrap();
    let mut other=Session::default();other.execute("project.open",json!({"path":f.project,"record_recent":false})).unwrap();
    assert!(other.execute("motion.theatre.save",json!({"expected_project":f.project,"state":native_state()})).unwrap_err().to_string().contains("owned by another"));
    assert_eq!(fs::read(&f.project).unwrap(),original);
}

#[test]
fn clear_removes_one_characters_keyframes_and_keeps_its_sheets(){
    let mut f=Fixture::new();
    f.session.project_assets.push(ProjectAsset{id:"fish".into(),name:"Fish".into(),preset:None,path:Some(f.rig.clone()),kind:"cutout_rig".into(),parts:Vec::new()});
    let keyed=json!({"tail":{"trackData":{"t":{"type":"BasicKeyframedTrack","keyframes":[{"id":"k","type":"bezier","position":0.0,"value":7.3,"handles":[0.5,1,0.5,0],"connectedRight":true}]}},"trackIdByPropPath":{"[\"rotation\"]":"t"}}});
    let sheet=|tracks:&Value|json!({"sequence":{"type":"PositionalSequence","length":6,"subUnitsPerUnit":30,"tracksByObject":tracks},"staticOverrides":{"byObject":{}}});
    f.save(json!({"definitionVersion":"0.4.0","revisionHistory":["r"],"sheetsById":{"fish / swim":sheet(&keyed),"fish / hover":sheet(&keyed),"bee / walk":sheet(&keyed)}})).unwrap();
    let one=f.session.execute("motion.theatre.clear",json!({"expected_project":f.project,"character":"fish","track":"swim"})).unwrap();
    assert_eq!(one["removed_tracks"],1);
    let state=f.session.execute("motion.theatre.read",json!({})).unwrap()["state"].clone();
    assert_eq!(state["sheetsById"]["fish / swim"]["sequence"]["tracksByObject"],json!({}),"the cleared clip plays the engine motion");
    assert_eq!(state["sheetsById"]["fish / hover"]["sequence"]["tracksByObject"],keyed,"other clips keep their edits");
    let all=f.session.execute("motion.theatre.clear",json!({"expected_project":f.project,"character":"fish"})).unwrap();
    assert_eq!(all["removed_tracks"],1);
    let disk:Value=serde_json::from_slice(&fs::read(&f.project).unwrap()).unwrap();
    assert_eq!(disk["theatre"]["sheetsById"]["fish / hover"]["sequence"]["tracksByObject"],json!({}));
    assert_eq!(disk["theatre"]["sheetsById"]["bee / walk"]["sequence"]["tracksByObject"],keyed,"another character is untouched");
    assert!(disk["theatre"]["sheetsById"].get("fish / swim").is_some(),"sheets stay; only keyframes go");
    assert!(f.session.execute("motion.theatre.clear",json!({"character":"fish"})).is_err(),"clearing needs the expected project");
}
