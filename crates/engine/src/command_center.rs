//! Project-aware control state shared by terminal, MCP and Studio through the command registry.
use crate::workspace::{resolve_workspace_path, sanitize_asset_id};
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::{Path,PathBuf}};

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
            .join(format!("{name}.magic"))
    };
    if path
        .extension()
        .is_none_or(|v| !v.eq_ignore_ascii_case("magic"))
    {
        path.as_mut_os_string().push(".magic");
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
    let mut audio_setup=Value::Null;
    if let Some(audio) = p["audio"].as_str().filter(|v| !v.is_empty()) {
        match s.execute("workshop.set_audio", json!({"path":audio,"hyperframes_cli":p["hyperframes_cli"]})) {
            Ok(value) => {audio_setup=value;steps.push(json!({"command":"workshop.set_audio","status":"complete; HyperFrames project and Studio ready"}));},
            Err(e) => {let error=e.to_string();steps.push(json!({"command":"workshop.set_audio","status":"failed","error":error}));problems.push(error);},
        };
    }
    if p["prepare_only"] == true {
        let mut outputs=json!({"video.init":audio_setup});
        if problems.is_empty() {
            for (command,params) in [("storyboard.init",json!({"empty":true}))] {
                match s.execute(command,params) {
                    Ok(value) if value["ok"]!=false => {steps.push(json!({"command":command,"status":"complete"}));outputs[command]=value;},
                    Ok(value) => {let error=format!("{command}: {value}");steps.push(json!({"command":command,"status":"failed","error":error}));problems.push(error);},
                    Err(error) => {let error=error.to_string();steps.push(json!({"command":command,"status":"failed","error":error}));problems.push(error);}
                }
            }
        }
        s.execute("project.save", json!({}))?;
        let mut next_commands=Vec::new();
        if !problems.is_empty() {
            if steps.iter().any(|step|step["command"]=="workshop.set_audio" && step["status"]=="failed") {
                if let Some(audio)=p["audio"].as_str() {
                next_commands.push(json!({"command":"workshop.set_audio","params":{"path":audio},"reason":"Fix the reported audio or HyperFrames error, then retry"}));
                }
            } else {
                next_commands.push(json!({"command":"storyboard.init","params":{"empty":true},"reason":"Fix the reported storyboard error, then retry"}));
            }
        } else {
            next_commands.extend([
                json!({"command":"storyboard.template","params":{"path":"<your DeckCraft template or magic-studio>"},"reason":"Choose your storyboard template"}),
                json!({"command":"storyboard.init","params":{"empty":true},"reason":"Create a storyboard from the selected template"}),
                json!({"command":"project.add_asset","params":{"path":"<your image>"},"reason":"Add artwork to the project without placing it"}),
                json!({"command":"workshop.import_transcript","params":{"path":"<timed transcript JSON>"},"reason":"Import timed lyrics when you have them"}),
            ]);
        }
        let result = json!({"ok":true,"project":path,"location":location,"warnings":warnings,
            "steps":steps,"problems":problems,"setup_ready":problems.is_empty(),"intake_ready":false,"prepared_only":true,
            "storyboard":outputs["storyboard.init"],"video":outputs["video.init"],
            "next_commands":next_commands,
            "creative_review":"pending; no scene structure, artwork placement or lyric choices were inferred"});
        write_state(dir,&result)?;
        if !keep_owner { s.project_owner = None; }
        return Ok(result);
    }
    if let Some(t) = p["transcript"].as_str().filter(|v| !v.is_empty()) {
        let mut params =
            json!({"path":t,"replace":p["replace_transcript"].as_bool().unwrap_or(false)});
        for k in ["captions", "audiomap"] {
            if let Some(v) = p[k].as_str().filter(|v| !v.is_empty()) {
                params[k] = json!(v);
            }
        }
        match s.execute("workshop.import_transcript",params){Ok(_)=>steps.push(json!({"command":"workshop.import_transcript","status":"complete; reused existing words/timing"})),Err(e)=>{let error=e.to_string();steps.push(json!({"command":"workshop.import_transcript","status":"failed","error":error}));problems.push(error);}};
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

    // Register knowledge skills for this project: each becomes an info block in the
    // Skills view (researching / implemented / not_implemented) with where it plugs in.
    if let Some(skill_list) = p["skills"].as_array() {
        for skill in skill_list.iter().filter_map(Value::as_object) {
            let id = skill.get("id").and_then(Value::as_str).filter(|v|!v.trim().is_empty());
            let name = skill.get("name").and_then(Value::as_str).filter(|v|!v.trim().is_empty());
            match (id, name) {
                (Some(id), Some(name)) => {
                    let params = json!({
                        "id": id,
                        "name": name,
                        "description": skill.get("description").and_then(Value::as_str).unwrap_or(""),
                        "status": skill.get("status").and_then(Value::as_str).unwrap_or("researching"),
                        "source": skill.get("source").and_then(Value::as_str).unwrap_or(""),
                        "implemented_at": skill.get("implemented_at").cloned().unwrap_or(json!([])),
                        "implemented_in": skill.get("implemented_in").cloned().unwrap_or(json!([])),
                        "integration_notes": skill.get("integration_notes").and_then(Value::as_str).unwrap_or(""),
                    });
                    match s.execute("skills.add", params) {
                        Ok(_) => steps.push(json!({"command":"skills.add","id":id,"status":"registered"})),
                        Err(e) => {
                            let error = e.to_string();
                            steps.push(json!({"command":"skills.add","id":id,"status":"failed","error":error}));
                            problems.push(format!("skill {id}: {error}"));
                        }
                    }
                }
                _ => problems.push("skills[] entry missing id or name".to_string()),
            }
        }
    }

    let valid_transcript = || crate::workshop::transcript_files(dir).iter().any(|f| {
        crate::workshop::transcript_document(f).ok().is_some_and(|v|crate::workshop::transcript_problems(&v).is_empty())
    });
    let mut valid = valid_transcript();
    let mut next_commands = Vec::<Value>::new();
    // Supplied/previously imported timing is authoritative. Never fall back to ASR after an import error.
    if !valid && problems.is_empty() && s.song_audio.is_some() {
        let params=json!({"_request_id":p["_request_id"],"_client":p["_client"],"analyze_beats":false,"hyperframes_cli":p["hyperframes_cli"]});
        match s.execute("audio.analyze", params.clone()) {
            Ok(_) => steps.push(json!({"command":"audio.analyze","params":params,"status":"complete; native pipeline output imported"})),
            Err(e) => {
                let error=e.to_string();
                steps.push(json!({"command":"audio.analyze","params":params,"status":"failed","error":error}));
                problems.push(error);
                next_commands.push(json!({"command":"audio.analyze","params":{},"reason":"Correct the reported pipeline dependency/input failure, then retry; existing outputs are reused"}));
            }
        }
        for record in read_state(dir)["received"].as_array().into_iter().flatten() {
            if !received.iter().any(|v|v["path"]==record["path"]) {received.push(record.clone());}
        }
        valid=valid_transcript();
    }
    if !valid && problems.is_empty() {
        problems.push("No valid timed transcript or linked audio. Provide audio to project.intake, or import existing timed JSON with workshop.import_transcript.".into());
    }
    let mut storyboard=Value::Null;
    let mut storyboard_ready=false;
    if valid && problems.is_empty() {
        match s.execute("storyboard.init", json!({})) {
            Ok(board) => {
                storyboard_ready=board["deck"].as_str().is_some_and(|v|Path::new(v).is_file());
                storyboard=json!({"deck":board["deck"],"pdf":board["pdf"],"reused_existing_document":board["reused_existing_document"]});
                if storyboard_ready {
                    steps.push(json!({"command":"storyboard.init","status":"complete","deck":board["deck"],"reused_existing_document":board["reused_existing_document"]}));
                } else {
                    let error="storyboard.init returned no existing native DeckCraft document";
                    problems.push(error.into());
                    steps.push(json!({"command":"storyboard.init","status":"failed","error":error}));
                }
            }
            Err(e) => {
                let error=e.to_string();
                steps.push(json!({"command":"storyboard.init","status":"failed","error":error}));
                problems.push(error);
            }
        }
        if !storyboard_ready {
            next_commands.push(json!({"command":"storyboard.init","params":{},"reason":"Correct the reported native document/tool failure, then retry; existing timing is preserved"}));
        }
    }
    if !valid && next_commands.is_empty() {
        next_commands.push(json!({"command":"workshop.import_transcript","params":{"path":"<existing timed JSON>","captions":"<optional original SRT>"},"reason":"Correct or supply timed input; no ASR fallback was attempted after a supplied-input failure"}));
    }
    let mut video=Value::Null;
    let mut video_ready=s.video_target!="hyperframes" || s.song_audio.is_none();
    if valid && storyboard_ready && problems.is_empty() && !video_ready {
        match s.execute("video.init",json!({"target":"hyperframes","hyperframes_cli":p["hyperframes_cli"]})) {
            Ok(value)=>{video_ready=value["captions"]["ready"]==true;steps.push(json!({"command":"video.init","status":if video_ready{"complete; native audio/caption project retained"}else{"incomplete; native captions missing"},"captions":value["captions"]}));video=value;},
            Err(e)=>{let error=e.to_string();problems.push(error.clone());steps.push(json!({"command":"video.init","status":"failed","error":error}));}
        }
        if !video_ready {next_commands.push(json!({"command":"video.init","params":{"target":"hyperframes","hyperframes_cli":p["hyperframes_cli"]},"reason":"Correct the reported native video/caption handoff failure; existing timing and deck are preserved"}));}
    }
    let result = json!({"ok":true,"warnings":warnings,"location":location,"intake_ready":problems.is_empty()&&valid&&storyboard_ready&&video_ready,"video_ready":video_ready,"video":video,"storyboard_ready":storyboard_ready,"storyboard":storyboard,"studio_verified":false,"project":path,"steps":steps,"problems":problems,"received":received,"visual_review":"pending","transcript_valid":valid && !steps.iter().any(|v|v["command"]=="workshop.import_transcript"&&v["status"]=="failed"),"manual":"magic-cli commands project.intake","next_commands":next_commands,"next_command":format!("{} --project {} commands",shell_quote(&cli_program()),shell_quote(&path.to_string_lossy()))});
    write_state(dir, &result)?;
    s.execute("project.save", json!({}))?;
    if !keep_owner{s.project_owner=None;}
    Ok(result)
}

pub fn status(s: &mut Session, p: &Value) -> Result<Value> {
    let started=std::time::Instant::now();
    let project_path=s.project_path.clone();
    let dir = project_path.as_deref().and_then(Path::parent);
    let saved = dir.map(read_state).unwrap_or(json!({}));
    let board=dir.and_then(|d|fs::read(d.join("storyboard/movie-board.json")).ok()).and_then(|b|serde_json::from_slice::<Value>(&b).ok()).unwrap_or(Value::Null);
    let board_transcript = board["source_transcript"].as_str().map(PathBuf::from).filter(|p|p.is_file());
    let root_transcript = dir.and_then(|d| crate::workshop::transcript_files(d).into_iter().next());
    let transcript = root_transcript.or(board_transcript);
    let parsed = transcript.as_ref().and_then(|path|crate::workshop::transcript_document(path).ok());
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
  let dependency=if c.id=="storyboard.init"||c.id=="workshop.build_storyboard" {(!valid).then(||"Requires a valid transcript; import or repair received input".to_string())}else if c.id.starts_with("workshop.")||c.id=="project.save" {s.project_path.is_none().then(||"Open or create a project first".to_string())}else{None};
  let reason=registry_reason.or(dependency);
  json!({"id":c.id,"label":c.label,"params":c.params,"menu":c.menu,"enabled":reason.is_none(),"block_reason":reason,"long_running":c.long_running})
 }).collect();
    let mapping = json!([
 {"control":"Home / Open Project","command":"project.open","source":"apps/magicstudio-shell/src/main.rs:project_open","status":"source verified; UI transport separate"},
 {"control":"Home / New song","command":"project.intake","source":"apps/magicstudio-shell/src/main.rs:project_create","status":"source verified; shared intake command"},
 {"control":"Add transcript","command":"workshop.import_transcript","source":"apps/magicstudio-shell/ui/app.js","status":"existing UI omits captions picker"},
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
    let workshop=if dir.is_some(){crate::workshop::workshop_status(s)?}else{json!({})};
    let design=if dir.is_some(){crate::storyboard_design::inspect(s,&json!({}))?}else{json!({})};
    let creative_followup=json!({
        "instruction":"Review the creator's story and existing assets first. Ask for missing storyboard details. Propose links or image prompts for review; do not place artwork or generate assets without the creator choosing the action.",
        "missing_fields":design["readiness"]["missing_fields"],
        "progression":design["progression"],
        "available_assets":s.project_assets,
        "suggested_commands":[{"command":"project.list_assets"},{"command":"storyboard.read"},{"command":"storyboard.edit","input":"creator-selected page, field and text"},{"command":"storyboard.place_asset","input":"creator-selected asset, page and placement"}],
        "creative_approval":"The creator reviews the actual storyboard; content completeness is not creative approval"});
    let words=parsed.as_ref().and_then(|v|v["words"].as_array());
    let count=words.map(Vec::len).unwrap_or(0);
    let lines=parsed.as_ref().and_then(|v|v["paragraphs"].as_array()).map(Vec::len).unwrap_or(0);
    let last_word=words.and_then(|v|v.iter().filter_map(|w|w["end"].as_f64()).reduce(f64::max));
    let duration=parsed.as_ref().and_then(|v|v["recording_duration_sec"].as_f64());
    let audio_ready=s.song_audio.as_ref().is_some_and(|f|f.is_file());
    let assets=s.project_assets.iter().filter(|a|a.path.as_ref().is_some_and(|p|p.is_file())).count();
    let animated=workshop["stages"].as_array().into_iter().flatten().find(|v|v["id"]=="animate").and_then(|v|v["assets"].as_array()).map(|v|v.iter().filter(|a|a["animatable"]==true).count()).unwrap_or(0);
    let deck=design["deck"].as_str();
    let deck_ready=if board["pdf_fields"].is_array(){board["pdf"].as_str().is_some_and(|f|Path::new(f).is_file()) && design["text_fields"].as_array().is_some_and(|v|!v.is_empty())}else{deck.is_some_and(|f|Path::new(f).is_file())};
    let mut native=json!({"state":"missing","reason":"Native HyperFrames audio/caption project not observed"});
    if let Some(dir)=dir {
        let hf=dir.join("hyperframes");
        let host=fs::read_to_string(hf.join("index.html")).ok();
        let source=fs::read_to_string(hf.join("compositions/captions.html")).ok();
        let receipt=fs::read(hf.join(".magicstudio-captions.json")).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok());
        let current=words.map(|w|format!("{:x}",Sha256::digest(serde_json::to_vec(w).unwrap_or_default())));
        let matched=receipt.as_ref().is_some_and(|r|r["input_sha256"].as_str()==current.as_deref() && count>0);
        let bound=host.as_ref().is_some_and(|v|v.contains("<audio") && v.contains("compositions/captions.html")) && source.as_ref().is_some_and(|v|v.contains("caption-group"));
        native=json!({"state":if valid&&audio_ready&&matched&&bound{"ready"}else if host.is_some(){"blocked"}else{"missing"},"reason":if valid&&audio_ready&&matched&&bound{"Audio and caption source linked to this transcript; timing fingerprint matches. Creative/listening approval separate."}else{"Missing or mismatched audio, caption source, timing fingerprint or host track"},"path":hf,"caption_source":hf.join("compositions/captions.html")});
    }
    let item=|id:&str,label:&str,ready:bool,present:bool,reason:&str|json!({"id":id,"label":label,"state":if ready{"ready"}else if present{"blocked"}else{"missing"},"reason":reason});
    let transcript_detail=if let Some(source)=board["source_transcript"].as_str(){format!("Loaded storyboard consumes {source} directly ({count} raw words); no ASR run. Archive label: {} (producer executable/version not recorded in native array). {}",Path::new(source).parent().and_then(|p|p.file_name()).and_then(|n|n.to_str()).unwrap_or("unknown"),board["timing_warning"].as_str().unwrap_or(""))}else{parsed.as_ref().and_then(|v|v["source"].as_str().map(|tool|format!("Native output reused; recorded tool {tool}; model {}. Timing/word data consumed directly; not retranscribed.",v["model"].as_str().unwrap_or("unknown")))).unwrap_or_else(||"Native timing schema validated; producer unknown when not recorded".to_string())};
    let mut prerequisites=vec![
        item("song","Song",audio_ready,s.song_audio.is_some(),"Linked audio file exists; source is not copied or transcribed by this observation"),
        item("lyrics","Lyrics & notes",valid&&(lines>0||count>0),transcript.is_some(),"Timed lyric data available; optional notes and recognition accuracy are separate"),
        item("transcript","Transcription",valid,transcript.is_some(),&transcript_detail),
        item("animate","Animate",animated>0,!s.project_assets.is_empty(),"Registered assets with usable rig parts; optional for static scenes"),
        item("assets","Assets",assets>0,!s.project_assets.is_empty(),"Registered source asset files exist; artistic approval separate"),
        json!({"id":"storyboard","label":"Storyboard content","state":design["readiness"]["state"],"reason":design["readiness"]["reason"],"missing_fields":design["readiness"]["missing_fields"]}),
        item("deckcraft","Editable storyboard PDF",deck_ready,board["pdf"].is_string()||deck.is_some(),"Saved PDF fields inspected through PDFCraft; source timing/creative approval remain separate"),
    ];
    native["id"]=json!("hyperframes");native["label"]=json!("HyperFrames audio + captions");prerequisites.push(native);
    let activity=crate::activity::events(s,&json!({"limit":192}))?;
    let mut doctor=dir.and_then(|p|fs::read(p.join(".agent/storyboard-doctor.json")).ok()).and_then(|b|serde_json::from_slice::<Value>(&b).ok()).unwrap_or(Value::Null);
    if doctor.is_object(){doctor["stale"]=json!(dir.and_then(crate::storyboard::doctor_revision).is_none_or(|r|doctor["storyboard_revision"].as_str()!=Some(&r)));}
    let metrics=json!({"timed_words":count,"lyric_groups":lines,"recording_duration_sec":duration,"last_word_end_sec":last_word,"registered_assets":s.project_assets.len(),"available_assets":assets,"animatable_assets":animated,"native_deck":deck,"design_dirty":design["dirty"],"server":workshop["video_server"]});
    let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|v|v.as_micros()).unwrap_or(0);
    let observation=json!({"id":format!("{}-{now}",std::process::id()),"project_id":s.project_path,"command":"center.status","observed_unix_micros":now,"elapsed_ms":started.elapsed().as_millis(),"source":"shared engine inspection; not inferred from process exit or UI revision"});
    let next_commands=prerequisites.iter().filter(|v|v["state"]!="ready" && (v["id"]!="animate" || assets>0)).map(|v|{
        let target=match v["id"].as_str(){Some("transcript"|"lyrics")=>"workshop.import_transcript",Some("deckcraft")=>"storyboard.init",Some("storyboard")=>"storyboard.edit",Some("hyperframes")=>"video.init",Some("song")=>"workshop.set_audio",_=>"project.add_asset"};
        let label=if v["id"]=="assets"{"See asset import arguments".into()}else{format!("See {} arguments",target)};
        let reason=if v["id"]=="assets" && s.project_assets.is_empty(){json!("No reference art is registered; inspect the import command before choosing an asset.")}else{v["reason"].clone()};
        json!({"label":label,"command":"command.list","params":{"filter":target},"subject_command":target,"operation":"exec","reason":reason})
    }).collect::<Vec<_>>();
    if p["trace"]==true {
        let _context=crate::activity::enter("center.status",p);
        crate::activity::command(s.project_path.as_deref(),"center.status",p,"dispatch_running",None);
        crate::activity::result(s.project_path.as_deref(),"center.status",&json!({"observation":observation,"prerequisites":prerequisites,"metrics":metrics}),started.elapsed().as_millis());
        crate::activity::command(s.project_path.as_deref(),"center.status",p,"engine_success",None);
    }
    Ok(
        json!({"ok":true,"creative_followup":creative_followup,"observation":observation,"prerequisites":prerequisites,"metrics":metrics,"next_commands":next_commands,"studio_acknowledgement":"unobserved; query ui.feedback separately","warnings":saved["warnings"],"project":s.project_path,"audio":s.song_audio,"target":s.video_target,"transcript":{"path":transcript,"valid":valid,"words":parsed.as_ref().and_then(|v|v["words"].as_array()).map(Vec::len)},"problems":problems,"received":saved["received"],"steps":saved["steps"],"provenance":provenance,"actions":actions,"commands":commands,"ui_mapping":mapping,"activity":activity,"storyboard_doctor":doctor,"visual_review":"pending","studio":"Not launched or switched. Attach only to an explicitly selected existing control bridge."}),
    )
}

pub fn requires_ui(_: &mut Session,_:&Value)->Result<Value>{Err(EngineError::InvalidParams("Use the canonical Studio connection: magic-cli exec ui.feedback; no UI exists in a headless engine session".into()))}

pub(crate) fn shell_quote(value:&str)->String{format!("'{}'",value.replace('\'',"'\\''"))}

pub(crate) fn cli_program()->String {
 std::env::current_exe().ok().map(|p|p.with_file_name("magic-cli"))
  .filter(|p|p.is_file()).map(|p|p.to_string_lossy().into_owned())
  .unwrap_or_else(||"magic-cli".into())
}
