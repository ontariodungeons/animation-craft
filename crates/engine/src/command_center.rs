//! Project-aware control state shared by terminal, MCP and Studio through the command registry.
use crate::workspace::{resolve_workspace_path, sanitize_asset_id};
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

pub(crate) fn receive(dir: &Path, source: &Path) -> Result<Value> {
    let bytes = fs::read(source)?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let dest = dir.join("received").join(&hash[..16]).join(
        source
            .file_name()
            .ok_or_else(|| EngineError::InvalidParams("input has no filename".into()))?,
    );
    if !dest.exists() {
        fs::create_dir_all(
            dest.parent()
                .ok_or_else(|| EngineError::InvalidParams("input has no parent".into()))?,
        )?;
        fs::write(&dest, &bytes)?;
    } else if fs::read(&dest)? != bytes {
        return Err(EngineError::InvalidParams(
            "received-file hash collision; existing bytes retained".into(),
        ));
    }
    Ok(
        json!({"source":source,"path":dest,"sha256":hash,"bytes":bytes.len(),"status":"received; validation separate"}),
    )
}
fn read_state(dir: &Path) -> Value {
    fs::read(dir.join("intake-state.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(json!({}))
}
pub(crate) fn write_state(dir: &Path, v: &Value) -> Result<()> {
    fs::write(
        dir.join("intake-state.json"),
        serde_json::to_vec_pretty(v).map_err(|e| EngineError::InvalidParams(e.to_string()))?,
    )?;
    Ok(())
}

pub fn intake(s: &mut Session, p: &Value) -> Result<Value> {
    let production_root = crate::project_settings::root()?;
    let mut location = json!({"source":"explicit path override","production_root":production_root,"setting_changed":false});
    let mut path = if let Some(v) = p["path"].as_str() {
        resolve_workspace_path(s, v)
    } else {
        let root = match p.get("root") {
            Some(value) => resolve_workspace_path(s, value.as_str().filter(|v|!v.trim().is_empty())
                .ok_or_else(||EngineError::InvalidParams("Explicit root must be a nonempty directory path".into()))?),
            None => production_root.clone(),
        };
        let is_production = root == production_root;
        if is_production && !root.is_dir() {
            return Err(EngineError::InvalidParams("Production project root is unavailable; mount the configured directory or deliberately pass root= for this request".into()));
        }
        location["source"] = json!(if is_production {"production root"} else {"explicit root override"});
        location["root"] = json!(root);
        let name = p["name"]
            .as_str()
            .filter(|x| !x.trim().is_empty() && !x.contains(['/', '\\']) && *x != "." && *x != "..")
            .ok_or_else(|| {
                EngineError::InvalidParams("Choose a song name without path separators".into())
            })?;
        root
            .join(sanitize_asset_id(name).replace('_', "-"))
            .join("project")
            .join(format!("{name}.acproj"))
    };
    if path
        .extension()
        .is_none_or(|v| !v.eq_ignore_ascii_case("acproj"))
    {
        path.as_mut_os_string().push(".acproj");
    }
    if path.exists(){path=fs::canonicalize(&path)?;}
    let keep_owner=s.project_owner.is_some()&&s.project_path.as_ref()==Some(&path);
    let exists = path.exists();
    let intake_owner=if exists{Some(crate::project::claim(s,&path)?)}else{None};
    if !exists
        && path.parent().is_some_and(|d| {
            d.is_dir() && fs::read_dir(d).is_ok_and(|mut entries| entries.next().is_some())
        })
    {
        return Err(EngineError::InvalidParams("New destination contains existing files. Choose a fresh song/project location; no files were overwritten.".into()));
    }
    if exists && p["resume"] != true {
        return Err(EngineError::InvalidParams(format!(
            "Project exists: {}. Inspect it or pass resume=true; no overwrite performed",
            path.display()
        )));
    }
    let lifecycle=s.execute(
        if exists {
            "project.open"
        } else {
            "project.new"
        },
        json!({"path":path,"overwrite":false}),
    )?;
    if let Some(owner)=intake_owner{s.project_owner=Some(owner);}
    let dir = path
        .parent()
        .ok_or_else(|| EngineError::InvalidParams("project needs a parent folder".into()))?;
    let warnings=lifecycle["warnings"].as_array().cloned().unwrap_or_default();
    let mut problems = Vec::<String>::new();
    let mut steps =
        vec![json!({"command":if exists{"project.open"}else{"project.new"},"status":"complete"})];
    if !exists {
        s.execute("workshop.set_target", json!({"target":"hyperframes"}))?;
    }
    if let Some(audio) = p["audio"].as_str().filter(|v| !v.is_empty()) {
        match s.execute("workshop.set_audio", json!({"path":audio})) {
            Ok(_) => steps
                .push(json!({"command":"workshop.set_audio","status":"complete; local reference"})),
            Err(e) => problems.push(e.to_string()),
        };
    }
    if let Some(t) = p["transcript"].as_str().filter(|v| !v.is_empty()) {
        let mut params =
            json!({"path":t,"replace":p["replace_transcript"].as_bool().unwrap_or(false)});
        for k in ["captions", "audiomap"] {
            if let Some(v) = p[k].as_str().filter(|v| !v.is_empty()) {
                params[k] = json!(v);
            }
        }
        match s.execute("workshop.import_transcript",params){Ok(_)=>steps.push(json!({"command":"workshop.import_transcript","status":"complete; reused existing words/timing"})),Err(e)=>problems.push(e.to_string())};
    }
    let mut received = read_state(dir)["received"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for v in p["materials"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        let source = resolve_workspace_path(s, v);
        match receive(dir, &source) {
            Ok(record) => {
                received.push(record.clone());
                match s.execute("workshop.add",json!({"kind":"liner_notes","paths":[record["path"]]})){Ok(_)=>steps.push(json!({"command":"workshop.add","status":"material preserved; creative role review pending","file":record["path"]})),Err(e)=>problems.push(e.to_string())};
            }
            Err(e) => problems.push(format!("material {v}: {e}")),
        }
    }
    let valid = crate::workshop::transcript_files(dir).iter().any(|f| {
        fs::read(f)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .is_some_and(|v| crate::workshop::transcript_problems(&v).is_empty())
    });
    if !valid && problems.is_empty() {
        problems.push("No valid transcript yet. Import existing HyperFrames words (JSON) with optional original SRT; audio processing is never automatic.".into());
    }
    let result = json!({"ok":true,"warnings":warnings,"location":location,"intake_ready":problems.is_empty()&&valid,"storyboard_ready":false,"studio_verified":false,"project":path,"steps":steps,"problems":problems,"received":received,"visual_review":"pending","transcript_valid":valid && !problems.iter().any(|v|v.contains("transcript")||v.contains("JSON")),"manual":"animationcraft-cli commands workshop.import_transcript; crates/engine/src/workshop.rs","next_command":format!("{} center {}",shell_quote(&cli_program()),shell_quote(&format!("project={}",path.display())))});
    write_state(dir, &result)?;
    s.execute("project.save", json!({}))?;
    if !keep_owner{s.project_owner=None;}
    Ok(result)
}

pub fn status(s: &mut Session, _: &Value) -> Result<Value> {
    let dir = s.project_path.as_deref().and_then(Path::parent);
    let saved = dir.map(read_state).unwrap_or(json!({}));
    let transcript = dir.and_then(|d| crate::workshop::transcript_files(d).into_iter().next());
    let parsed = transcript
        .as_ref()
        .and_then(|f| fs::read(f).ok())
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
    let valid = parsed
        .as_ref()
        .is_some_and(|v| crate::workshop::transcript_problems(v).is_empty())
        && saved["transcript_valid"] != false
        && saved["valid"] != false;
    let mut problems = saved["problems"].as_array().cloned().unwrap_or_default();
    if !valid {
        problems.push(json!("Storyboard requires valid timed words/captions; received inputs remain available for repair."));
    }
    let commands:Vec<Value>=crate::command_specs().iter().map(|c|{
  let registry_reason=(c.enabled)(s).err();
  let dependency=if c.id.starts_with("storyboard.")||c.id=="workshop.build_storyboard" {(!valid).then(||"Requires a valid transcript; import or repair received input".to_string())}else if c.id.starts_with("workshop.")||c.id=="project.save" {s.project_path.is_none().then(||"Open or create a project first".to_string())}else{None};
  let reason=registry_reason.or(dependency);
  json!({"id":c.id,"label":c.label,"params":c.params,"menu":c.menu,"enabled":reason.is_none(),"block_reason":reason,"long_running":c.long_running})
 }).collect();
    let mapping = json!([
 {"control":"Home / Open Project","command":"project.open","source":"apps/animationcraft-shell/src/main.rs:project_open","status":"source verified; UI transport separate"},
 {"control":"Home / New song","command":"project.intake","source":"apps/animationcraft-shell/src/main.rs:project_create","status":"source verified; shared intake command"},
 {"control":"Add transcript","command":"workshop.import_transcript","source":"apps/animationcraft-shell/ui/app.js","status":"existing UI omits captions picker"},
 {"control":"Save","command":"project.save","source":"crates/engine/src/commands.rs","status":"backend command; not every UI save control verified"}]);
    let provenance = dir
        .and_then(|d| fs::read(d.join("transcript-provenance.json")).ok())
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .unwrap_or(json!({"mode":"unknown","provider":"unknown","model":"unknown"}));
    if let Some(warning)=provenance["duration_warning"].as_str(){problems.push(json!(warning));}
    let actions = json!([
 {"label":"Inspect / refresh","command":"center.status","enabled":true},
 {"label":"New / resume intake","command":"project.intake","enabled":true},
 {"label":"Create starting storyboard","command":"storyboard.init","enabled":valid},
 {"label":"Read storyboard contents","command":"storyboard.toc","enabled":valid},
 {"label":"Save project","command":"project.save","enabled":s.project_path.is_some()}]);
    Ok(
        json!({"ok":true,"warnings":saved["warnings"],"project":s.project_path,"audio":s.song_audio,"target":s.video_target,"transcript":{"path":transcript,"valid":valid,"words":parsed.as_ref().and_then(|v|v["words"].as_array()).map(Vec::len)},"problems":problems,"received":saved["received"],"steps":saved["steps"],"provenance":provenance,"actions":actions,"commands":commands,"ui_mapping":mapping,"activity":crate::activity::events(s,&json!({"limit":12}))?,"visual_review":"pending","studio":"Not launched or switched. Attach only to an explicitly selected existing control bridge."}),
    )
}

pub fn requires_ui(_: &mut Session,_:&Value)->Result<Value>{Err(EngineError::InvalidParams("Use the canonical Studio connection: animationcraft-cli exec ui.feedback; no UI exists in a headless engine session".into()))}

fn shell_quote(value:&str)->String{format!("'{}'",value.replace('\'',"'\\''"))}

fn cli_program()->String{
 std::env::current_exe().ok().and_then(|p|{
  if p.file_name().is_some_and(|v|v.to_string_lossy().starts_with("animationcraft-cli")){Some(p)}else{p.parent().map(|p|p.join("animationcraft-cli")).filter(|p|p.is_file())}
 }).map(|p|p.to_string_lossy().into_owned()).unwrap_or_else(||"animationcraft-cli".into())
}
