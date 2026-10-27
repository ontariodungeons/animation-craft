//! Project-owned links between editable native sources and frozen render receipts.
//! Native editors save their own documents; project.save persists these associations.
use std::{fs, io::Read, path::{Path, PathBuf}};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use crate::{EngineError, Result, Session};

fn bad(message: impl Into<String>) -> EngineError { EngineError::InvalidParams(message.into()) }
fn text<'a>(p: &'a Value, key: &str) -> Result<&'a str> {
    p[key].as_str().filter(|v| !v.is_empty()).ok_or_else(||bad(format!("{key} is required")))
}
fn id(p: &Value) -> Result<&str> {
    let id=text(p,"id")?;
    if id.len()>80 || !id.bytes().all(|c|c.is_ascii_alphanumeric() || b"_-".contains(&c)) {return Err(bad("source id must use 1–80 letters, numbers, underscores or hyphens"));}
    Ok(id)
}
fn expected(p: &Value) -> Result<u64> {p["expected_revision"].as_u64().ok_or_else(||bad("expected_revision must be an integer; use 0 for a new source"))}
fn current(s: &Session, p: &Value) -> Result<Value> {
    crate::motion_authoring::project_guard(s,p)?;
    let source=s.animation_sources.get(id(p)?).ok_or_else(||bad("Source is not registered"))?;
    if source["revision"].as_u64()!=Some(expected(p)?) {return Err(bad("Source revision changed; refresh before continuing"));}
    Ok(source.clone())
}
fn next_revision(revision: u64) -> Result<u64> {revision.checked_add(1).ok_or_else(||bad("Source revision exhausted"))}
fn read_json(path: &Path) -> Result<Value> {
    let mut bytes=Vec::new();fs::File::open(path)?.take(16*1024*1024+1).read_to_end(&mut bytes)?;
    if bytes.len()>16*1024*1024 {return Err(bad("Source receipt exceeds 16 MiB"));}
    serde_json::from_slice(&bytes).map_err(|e|bad(e.to_string()))
}
fn mutation(source: Value) -> Value {json!({"ok":true,"revision":source["revision"],"source":source,"project_saved":false,"unsaved":true})}

pub fn sources(s: &mut Session, p: &Value) -> Result<Value> {
    let project=s.project_path.as_ref().ok_or_else(||bad("No project"))?;
    if p.get("expected_project").is_some(){crate::motion_authoring::project_guard(s,p)?;}
    let disk=read_json(project)?;
    let stored=disk.get("animation_sources").filter(|v|!v.is_null()).cloned().unwrap_or_else(||json!({}));
    Ok(json!({"project":project,"sources":s.animation_sources.values().collect::<Vec<_>>(),
        "unsaved":stored!=json!(s.animation_sources),"save_destinations":{"project":project,"native":"source","film":"video.place_track"}}))
}

pub fn set(s: &mut Session, p: &Value) -> Result<Value> {
    crate::motion_authoring::project_guard(s,p)?;
    let id=id(p)?.to_owned();let revision=expected(p)?;
    let actual=s.animation_sources.get(&id).and_then(|v|v["revision"].as_u64()).unwrap_or(0);
    if actual!=revision {return Err(bad("Source revision changed; refresh before continuing"));}
    if actual==0 && s.animation_sources.len()>=256 {return Err(bad("Project source limit is 256"));}
    let backend=text(p,"backend")?;
    let extensions:&[&str]=match backend {"blender"=>&["blend"],"synfig"=>&["sif","sifz"],"opentoonz"=>&["tnz"],"mlt"=>&["json"],_=>return Err(bad("backend must be blender, synfig, opentoonz or mlt"))};
    let source=crate::workspace::resolve_workspace_path(s,text(p,"source")?).canonicalize()?;
    if !source.is_file() || !source.extension().and_then(|v|v.to_str()).is_some_and(|v|extensions.contains(&v)) {return Err(bad("Source extension does not match backend; MLT uses assembly JSON"));}
    let bundle=if matches!(backend,"synfig"|"opentoonz") {
        let root=if let Some(raw)=p["bundle"].as_str().filter(|v|!v.is_empty()) {crate::workspace::resolve_workspace_path(s,raw)}
            else if backend=="opentoonz" {crate::opentoonz::project_root(&source).unwrap_or_else(||source.parent().unwrap_or(&source).to_path_buf())}
            else {source.parent().unwrap_or(&source).to_path_buf()};
        let root=root.canonicalize()?;
        if !root.is_dir() || !source.starts_with(&root){return Err(bad("Source must be inside its dependency bundle"));}
        Some(root)
    } else {None};
    let character=p["character"].as_str().filter(|v|!v.is_empty());let track=p["track"].as_str().filter(|v|!v.is_empty());
    if character.is_some()!=track.is_some(){return Err(bad("character and track must be supplied together"));}
    if let Some(character)=character {
        let actions=crate::blender::cmd_blender_actions(s,&json!({"character":character}))?;
        if !actions["tracks"].as_array().is_some_and(|tracks|tracks.iter().any(|t|t["id"].as_str()==track)){return Err(bad("Source association requires a saved character action"));}
    }
    let value=json!({"id":id,"backend":backend,"source":source,"bundle":bundle,"character":character,"track":track,"revision":next_revision(revision)?,"accepted":Value::Null});
    s.animation_sources.insert(id,value.clone());
    Ok(mutation(value))
}

fn render_params(source: &Value, p: &Value) -> Result<Value> {
    let backend=text(source,"backend")?;
    let options=p.get("options").cloned().unwrap_or_else(||json!({}));
    let options=options.as_object().ok_or_else(||bad("options must be an object"))?;
    let native:&[&str]=match backend {"blender"=>&["max_frames","resolution_percent"],"synfig"=>&["first_frame","last_frame","fps","width","height"],"opentoonz"=>&["first","last","step","shrink","nthreads"],"mlt"=>&[],_=>return Err(bad("Unsupported registered backend"))};
    for key in options.keys(){if !native.contains(&key.as_str()) && !["memory_mb","timeout_seconds"].contains(&key.as_str()){return Err(bad(format!("Unsupported render option: {key}")));}}
    let mut request=Value::Object(options.clone());
    let key=match backend {"blender"=>"blend","synfig"=>"source","opentoonz"=>"scene",_=>"timeline"};
    request[key]=source["source"].clone();
    for key in ["bundle","character","track"]{if !source[key].is_null(){request[key]=source[key].clone();}}
    request["source_id"]=source["id"].clone();request["source_revision"]=source["revision"].clone();
    Ok(request)
}
pub fn render(s: &mut Session, p: &Value) -> Result<Value> {
    let source=current(s,p)?;
    let request=render_params(&source,p)?;
    let mut job=match text(&source,"backend")? {
        "blender"=>crate::blender::cmd_blender_start(s,&request)?,
        "synfig"=>crate::synfig::start(s,&request)?,
        "opentoonz"=>crate::opentoonz::cmd_opentoonz_start(s,&request)?,
        "mlt"=>crate::mlt::start(s,&request)?,_=>return Err(bad("Unsupported registered backend")),
    };
    job["source_id"]=source["id"].clone();job["source_revision"]=source["revision"].clone();
    Ok(job)
}
fn accepted_result(source: &Value, result: &Value, receipt: &Path) -> Result<Value> {
    if result["state"]!="completed" || result["partial"]!=false {return Err(bad("Accept requires a completed full render; partial previews are not accepted"));}
    if result["backend"]!=source["backend"] || result["source_id"]!=source["id"] || result["source_revision"]!=source["revision"] {return Err(bad("Render belongs to a different or stale source revision"));}
    if result.pointer("/alpha_evidence/clip_max").and_then(Value::as_u64)==Some(0) {return Err(bad("Rendered clip is fully transparent; inspect the native scene before accepting"));}
    let clip=PathBuf::from(text(result,"clip")?);
    let metadata=fs::metadata(&clip)?;
    if !metadata.is_file() || metadata.len()>512*1024*1024 {return Err(bad("Accepted clip must be a regular file no larger than 512 MiB"));}
    let mut digest=Sha256::new();std::io::copy(&mut fs::File::open(&clip)?,&mut digest)?;
    if result["clip_sha256"]!=format!("{:x}",digest.finalize()){return Err(bad("Rendered clip hash changed; render again before accepting"));}
    let mut accepted=json!({"receipt":receipt,"provenance":{}});
    for key in ["job","clip","clip_sha256","backend","clip_seconds","fps","canvas","world_rect","partial"] {accepted[key]=result[key].clone();}
    for key in ["rig_sha256","spec","character","track","source_blend_sha256"] {accepted["provenance"][key]=result[key].clone();}
    if result["source_bundle"].is_object(){
        let mut bundle=json!({});
        for key in ["path","bundle","original_path","sha256","bundle_sha256"] {bundle[key]=result["source_bundle"][key].clone();}
        accepted["provenance"]["source_bundle"]=bundle;
    } else {accepted["provenance"]["source_bundle"]=Value::Null;}
    Ok(accepted)
}
pub fn accept(s: &mut Session, p: &Value) -> Result<Value> {
    let mut source=current(s,p)?;
    let job=json!({"job":text(p,"job")?});
    let result=crate::blender::cmd_blender_status(s,&job)?;
    let (_,directory)=crate::blender::job(s,&job)?;
    source["accepted"]=accepted_result(&source,&result,&directory.join("result.json"))?;
    source["revision"]=json!(next_revision(expected(p)?)?);
    s.animation_sources.insert(text(&source,"id")?.into(),source.clone());
    Ok(mutation(source))
}
