//! Storyboard page grouping. Recorded words and their times are read-only inputs.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::path::Path;

fn invalid(message: &str) -> EngineError { EngineError::InvalidParams(message.into()) }
fn load(path: &Path) -> Result<Value> {
    serde_json::from_slice(&std::fs::read(path)?).map_err(|e| invalid(&e.to_string()))
}
fn board(s: &Session) -> Result<Value> {
    load(&crate::workshop::project_dir(s)?.join("storyboard/movie-board.json"))
}
pub(crate) fn words(board: &Value) -> Result<Vec<Value>> {
    if board["empty_template"]==true { return Ok(Vec::new()); }
    let path = board["source_transcript"].as_str().ok_or_else(|| invalid("Storyboard has no project timing source"))?;
    let native = board["project"].as_str().and_then(|project|Path::new(project).parent()).map(|dir|dir.join("hyperframes/transcript.json")).filter(|p|p.is_file());
    let doc = crate::workshop::transcript_document(native.as_deref().unwrap_or(Path::new(path)))?;
    let source = doc.as_array().or_else(|| doc["words"].as_array()).ok_or_else(|| invalid("Project timing has no words"))?;
    let mut ids = std::collections::HashSet::new();
    source.iter().enumerate().map(|(i, word)| {
        let mut word = word.clone();
        if word["id"].is_null() { word["id"] = json!(format!("w{i}")); }
        if !word["id"].is_string() { return Err(invalid("Project word IDs must be strings")); }
        if !ids.insert(word["id"].to_string()) { return Err(invalid("Project timing contains duplicate word IDs")); }
        Ok(word)
    }).collect()
}
pub(crate) fn packets(s: &mut Session) -> Result<(Value, Value, Vec<Value>)> {
    let board = board(s)?;
    let design = crate::storyboard_design::inspect(s, &json!({}))?;
    let mut source=board.clone();
    if source["source_transcript"].is_null(){source["source_transcript"]=json!(crate::workshop::transcript_files(&crate::workshop::project_dir(s)?).first());}
    let words=if source["source_transcript"].is_null(){Vec::new()}else{words(&source)?};
    let mut packets = Vec::new();
    if !board["page_plan"].is_array() && board["empty_template"]!=true {
        let first=board["first_scene_page"].as_u64().unwrap_or(1);
        for (i,mut scene) in crate::storyboard::storyboard_scenes(&crate::workshop::project_dir(s)?)?.into_iter().enumerate(){
            let page=first+i as u64;
            scene["target"]=json!(format!("scene-{}",i+1));scene["scene"]=json!(i+1);scene["page"]=json!(page);
            scene["title"]=board["scene_titles"][i].as_str().map(|v|json!(v)).unwrap_or_else(||json!(format!("Scene {}",i+1)));
            scene["text_fields"]=json!(design["text_fields"].as_array().into_iter().flatten().filter(|f|f["page"]==page).collect::<Vec<_>>());
            scene["sections"]=json!(scene["text_fields"].as_array().into_iter().flatten().filter_map(|f|f["field"].as_str().filter(|key|!key.starts_with("object.")).map(|key|(key.to_owned(),f["value"].clone()))).collect::<serde_json::Map<String,Value>>());
            scene["saved_placements"]=json!(design["placements"].as_array().into_iter().flatten().filter(|f|f["page"]==page).collect::<Vec<_>>());
            scene["words"]=json!(words.iter().filter(|w|w["start"].as_f64().is_some_and(|t|t>=scene["start"].as_f64().unwrap_or(0.0)&&t<scene["end"].as_f64().unwrap_or(0.0))).collect::<Vec<_>>());
            scene["preview"]=board["previews"][page as usize-1].clone();packets.push(scene);
        }
        return Ok((board,design,packets));
    }
    for (page, plan) in board["page_plan"].as_array().into_iter().flatten().enumerate() {
        let group = &plan["group"];
        if !group.is_object() { continue; }
        let fields: Vec<Value> = design["text_fields"].as_array().into_iter().flatten().filter(|f| f["page"] == page + 1).cloned().collect();
        let placements: Vec<Value> = design["placements"].as_array().into_iter().flatten().filter(|p| p["page"] == page + 1).cloned().collect();
        let selected: Vec<Value> = words.iter().filter(|w| {
            w["start"].as_f64().is_some_and(|t| t >= group["span_start"].as_f64().unwrap_or(0.0) && t < group["span_end"].as_f64().unwrap_or(0.0))
        }).cloned().collect();
        let lyric = selected.iter().filter_map(|w|w["text"].as_str()).collect::<Vec<_>>().join(" ");
        packets.push(json!({"target":group["id"],"scene":packets.len()+1,"page":page+1,"title":plan["title"],"kind":group["kind"],
            "start":group["span_start"],"end":group["span_end"],"status":group["status"],"section_label":group["section_label"],
            "panels":group["panels"],"words":selected,"lines":[{"line":1,"start":group["span_start"],"lyric":lyric}],
            "text_fields":fields,"saved_placements":placements,"notes":group["notes"].as_array().cloned().unwrap_or_default(),"sections":{},"preview":board["previews"][page]}));
    }
    if packets.is_empty() && board["empty_template"]!=true { return Err(invalid("Create the native storyboard first")); }
    Ok((board, design, packets))
}
fn scene_index(packets: &[Value], p: &Value) -> Result<usize> {
    if let Some(id) = p["scene"].as_str() {
        return packets.iter().position(|s| s["target"] == id).ok_or_else(|| invalid("Unknown storyboard scene ID"));
    }
    p["scene"].as_u64().and_then(|n| n.checked_sub(1)).map(|n| n as usize).filter(|n| *n < packets.len()).ok_or_else(|| invalid("scene must be an existing scene number or ID"))
}
/// Only images actually assigned to the native Backgrounds & locations section.
pub fn backgrounds(s:&mut Session,_:&Value)->Result<Value>{
    let board=board(s)?;
    let mut images=Vec::new();let mut seen=std::collections::HashSet::new();
    for (page,plan) in board["page_plan"].as_array().into_iter().flatten().enumerate(){
        if plan["kind"]!="locations"{continue;}
        let sheet=plan["sheet"].as_array().into_iter().flatten().map(|record|(record["id"].as_str(),record["file"].as_str()));
        let placed=board["design"]["placements"].as_array().into_iter().flatten().filter(|p|p["page"]==page+1).map(|p|(p["asset"].as_str(),p["path"].as_str()));
        for (id,path) in sheet.chain(placed){
            if let Some(asset)=s.project_assets.iter().find(|a|Some(a.id.as_str())==id && a.path.as_ref().is_some_and(|p|p.is_file() && p.to_str()==path)){
                if seen.insert(asset.id.clone()){images.push(json!({"asset_id":asset.id,"name":asset.name,"path":asset.path}));}
            }
        }
    }
    Ok(json!({"images":images}))
}

pub fn read(s: &mut Session, p: &Value) -> Result<Value> {
    let board=board(s)?;
    let design=crate::storyboard_design::inspect(s,&json!({}))?;
    if let Some(page) = p["page"].as_u64() {
        if page == 0 || page > board["previews"].as_array().map_or(0,Vec::len) as u64 { return Err(invalid("Unknown storyboard page")); }
        return Ok(json!({"ok":true,"page":page,"title":board["page_titles"][page as usize-1],"revision":board["pdf"],
            "text_fields":design["text_fields"].as_array().into_iter().flatten().filter(|f|f["page"]==page).collect::<Vec<_>>()}));
    }
    let (board, design, packets) = packets(s)?;
    if !p["scene"].is_null() {
        let mut scene = packets[scene_index(&packets, p)?].clone();
        scene["ok"] = json!(true); scene["revision"] = board["pdf"].clone(); scene["page_size"] = board["page_size"].clone(); scene["deck"] = board["deck"].clone();
        return Ok(scene);
    }

    Ok(json!({"ok":true,"revision":board["pdf"],"pdf":board["pdf"],"chapters":packets,"pages":board["page_plan"],"page_count":board["previews"].as_array().map_or(0,Vec::len),"readiness":design["readiness"]}))
}
fn check_revision(board: &Value, design: &Value, p: &Value) -> Result<()> {
    if design["dirty"] == true { return Err(invalid("Save or undo pending storyboard edits first")); }
    if p["expected_revision"].as_str() != board["pdf"].as_str() { return Err(invalid("Storyboard changed; read it again and use its revision")); }
    Ok(())
}
/// Change grouping only: split, combine, or move a cut to an existing recorded word.
fn regroup(packets: &mut Vec<Value>, words: &[Value], p: &Value, operation: &str, new_id: &str) -> Result<()> {
    let i = scene_index(packets,p)?;
    if operation == "delete" {
        if packets.len() == 1 { return Err(invalid("Keep at least one scene covering the song")); }
        let into = p["into"].as_str().unwrap_or(if i == 0 {"next"} else {"previous"});
        let j = match into {"previous" if i > 0 => i-1, "next" if i+1 < packets.len() => i+1, _ => return Err(invalid("into must name an adjacent previous or next scene"))};
        let deleted = packets[i].clone();
        let edge = if j < i {"end"} else {"start"};
        packets[j][edge] = deleted[edge].clone();
        // The previous version retains the removed page, artwork and every field for Restore.
        packets[j]["status"] = json!("needs review"); packets.remove(i);
    } else {
        let id = p["word"].as_str().ok_or_else(|| invalid("word must identify an existing project timing word"))?;
        let at = words.iter().find(|w| w["id"] == id).and_then(|w| w["start"].as_f64()).ok_or_else(|| invalid("Unknown recorded word"))?;
        if operation == "create" {
            let start=packets[i]["start"].as_f64().unwrap_or(0.0);let end=packets[i]["end"].as_f64().unwrap_or(0.0);
            if at <= start || at >= end { return Err(invalid("Split at a word inside the selected scene")); }
            let mut next = packets[i].clone();next["target"]=json!(new_id);next["start"]=json!(at);next["status"]=json!("needs review");
            packets[i]["end"]=json!(at);packets[i]["status"]=json!("needs review");packets.insert(i+1,next);
        } else {
            let (edge, other, j) = match p["edge"].as_str() {
                Some("start") if i>0 => ("start","end",i-1),
                Some("end") if i+1<packets.len() => ("end","start",i+1),
                _ => return Err(invalid("Trim a shared start or end boundary; the song endpoints are fixed"))
            };
            let low=packets[i.min(j)]["start"].as_f64().unwrap_or(0.0);let high=packets[i.max(j)]["end"].as_f64().unwrap_or(0.0);
            if at<=low || at>=high { return Err(invalid("The boundary must stay inside the two adjacent scenes")); }
            packets[i][edge]=json!(at);packets[j][other]=json!(at);packets[i]["status"]=json!("needs review");packets[j]["status"]=json!("needs review");
        }
    }
    Ok(())
}
fn rebuild(s:&mut Session, board:&Value, design:&Value, packets:Vec<Value>, backgrounds:Option<Vec<Value>>)->Result<Value>{
    let result=crate::storyboard::cmd_workshop_build_storyboard(s,&json!({"draft":true,"_scenes":packets,"_page_content":design["text_fields"],"_previous_board":board,"_backgrounds":backgrounds}))?;
    if result["ok"]!=true || result["mapping"]["needs_correction"]==true { return Err(invalid(&format!("Storyboard draft could not be saved; current version unchanged: {result}"))); }
    let mut next=result;next["draft"]=json!(false);next["status"]=json!("saved; needs review");
    let design=json!({"placements":next["design"]["placements"].as_array().cloned().unwrap_or_default(),"text_edits":[],"text_fields":crate::storyboard_design::native_fields(s,&next)?,"dirty":false});
    let history=crate::workshop::project_dir(s)?.join("storyboard/history");std::fs::create_dir_all(&history)?;
    let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    let version=history.join(stamp.to_string());std::fs::create_dir_all(&version)?;let previous=version.join("manifest.json");
    std::fs::write(&previous,serde_json::to_vec_pretty(board).map_err(|e|invalid(&e.to_string()))?)?;
    let mut saved=crate::storyboard_design::save_deck(s,next,&design)?;
    saved["previous_version"]=json!(previous);saved["source_timing_changed"]=json!(false);
    Ok(saved)
}
pub fn create(s:&mut Session,p:&Value)->Result<Value>{
    if p["scene"].is_null(){ return crate::storyboard::cmd_storyboard_init(s,p); }
    change(s,p,"create")
}
pub fn delete(s:&mut Session,p:&Value)->Result<Value>{change(s,p,"delete")}
pub fn update(s:&mut Session,p:&Value)->Result<Value>{
    if let Some(ids)=p["backgrounds"].as_array(){
        let (board,design,scenes)=packets(s)?;check_revision(&board,&design,p)?;
        let mut records=Vec::new();let mut seen=std::collections::HashSet::new();
        for id in ids{
            let id=id.as_str().ok_or_else(||invalid("backgrounds must contain registered image IDs"))?;
            if !seen.insert(id){return Err(invalid("Duplicate background image"));}
            let asset=s.project_assets.iter().find(|a|a.id==id && a.kind=="image" && a.path.as_ref().is_some_and(|p|p.is_file())).ok_or_else(||invalid("Select an existing registered background image"))?;
            records.push(json!({"id":asset.id,"asset_id":asset.id,"name":asset.name,"file":asset.path,"category":"Backgrounds & locations"}));
        }
        return rebuild(s,&board,&design,scenes,Some(records));
    }
    if p["edge"].is_string(){return change(s,p,"update");}
    if p["field"]=="status"{
        let (board,design,mut scenes)=packets(s)?;check_revision(&board,&design,p)?;
        let status=p["value"].as_str().filter(|s|crate::storyboard::SCENE_STATUSES.contains(s)).ok_or_else(||invalid("Unknown storyboard status"))?;
        let i=scene_index(&scenes,p)?;scenes[i]["status"]=json!(status);return rebuild(s,&board,&design,scenes,None);
    }
    if let Some(path)=p["restore"].as_str(){
        let (current,design,_)=packets(s)?;check_revision(&current,&design,p)?;
        let root=crate::workshop::project_dir(s)?.join("storyboard").canonicalize()?;let path=Path::new(path).canonicalize()?;
        if !path.starts_with(root) || path.file_name().and_then(|s|s.to_str())!=Some("manifest.json"){return Err(invalid("Restore a saved version from this storyboard"));}
        let old=load(&path)?;if old["project"]!=json!(s.project_path){return Err(invalid("That version belongs to another project"));}
        let mut design=old["design"].clone();design["text_fields"]=crate::storyboard_design::native_fields(s,&old)?;
        return crate::storyboard_design::save_deck(s,old,&design);
    }
    if p["scene"].is_string(){
        let (_,_,scenes)=packets(s)?;let mut p=p.clone();p["page"]=scenes[scene_index(&scenes,&p)?]["page"].clone();
        return crate::storyboard_design::edit_text(s,&p);
    }
    crate::storyboard_design::edit_text(s,p)
}
fn change(s:&mut Session,p:&Value,operation:&str)->Result<Value>{
    let (board,design,mut scenes)=packets(s)?;check_revision(&board,&design,p)?;
    let words=words(&board)?;
    let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    regroup(&mut scenes,&words,p,operation,&format!("scene-{stamp}"))?;
    rebuild(s,&board,&design,scenes,None)
}
pub(crate) fn search(s:&mut Session,query:&str)->Result<Value>{
    let design=crate::storyboard_design::inspect(s,&json!({}))?;let query=query.to_lowercase();
    Ok(json!({"hits":design["text_fields"].as_array().into_iter().flatten().filter(|f|f["value"].as_str().is_some_and(|v|v.to_lowercase().contains(&query))).collect::<Vec<_>>()}))
}
