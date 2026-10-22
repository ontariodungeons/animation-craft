//! Scene composition drafts and non-destructive DeckCraft versions.
use crate::{EngineError,Result,Session};
use serde_json::{Value,json};
use std::path::Path;
fn invalid(s:&str)->EngineError{EngineError::InvalidParams(s.into())}
fn read(path:&Path)->Result<Value>{serde_json::from_slice(&std::fs::read(path)?).map_err(|e|invalid(&e.to_string()))}
fn write(path:&Path,value:&Value)->Result<()>{std::fs::write(path,serde_json::to_vec_pretty(value).map_err(|e|invalid(&e.to_string()))?)?;Ok(())}
fn draft(s:&Session)->Result<std::path::PathBuf>{Ok(crate::workshop::project_dir(s)?.join("storyboard/design-draft.json"))}
pub fn inspect(s:&mut Session,_:&Value)->Result<Value>{
 let board=crate::workshop::project_dir(s)?.join("storyboard/movie-board.json");
 let board=if board.exists(){read(&board)?}else{json!({})};
 // The saved snapshot is already committed to a deck. Older versions copied the draft's
 // dirty flag into it; only an outstanding draft can represent unsaved design work.
 let mut doc=if board["design"]["placements"].is_array(){board["design"].clone()}else{json!({"placements":[]})};
 doc["dirty"]=json!(false);
 let file=draft(s)?;
 if file.exists(){let mut pending=read(&file)?;if pending["dirty"]==true || pending["placements"].as_array().is_some_and(|v|!v.is_empty()){pending["dirty"]=json!(true);doc=pending;}}
 doc["dirty"]=json!(doc["dirty"]==true);
 doc["deck"]=board["deck"].clone();doc["pdf"]=board["pdf"].clone();
 doc["text_fields"]=native_fields(s,&board)?;
 Ok(doc)
}

/// Inspect the existing native document; role names and boxes come from its objects.
pub(crate) fn native_fields(s:&Session,board:&Value)->Result<Value>{
 let Some(deck)=board["deck"].as_str().filter(|p|Path::new(p).is_file()) else{return Ok(json!([]));};
 let dir=crate::workshop::project_dir(s)?;
 let count=board["previews"].as_array().map_or(0,Vec::len);
 let mut args=vec!["run".into(),"--in".into(),deck.into()];
 for i in 0..count{args.extend(["--cmd".into(),format!("slide.inspect={}",json!({"index":i}))]);}
 args.push("--print".into());
 let output=crate::workspace::run_tool_in(&dir,"deckcraft-cli",&args)?;
 let mut fields=Vec::new();
 for line in output.lines(){
  let Some(raw)=line.strip_prefix("slide.inspect → ") else{continue;};
  let slide:Value=serde_json::from_str(raw).map_err(|e|invalid(&format!("Invalid native slide inspection: {e}")))?;
  let page=slide["index"].as_u64().ok_or_else(||invalid("Native slide has no index"))?+1;
  for shape in slide["shapes"].as_array().into_iter().flatten(){
   let role=shape["altText"].as_str().unwrap_or_default();
   let field=role.strip_prefix("user.scene.").or_else(||role.strip_prefix("user.")).or_else(||(role=="auto.scene.lyrics").then_some("lyrics"));
   if let Some(field)=field{
    let scene=page.checked_sub(board["first_scene_page"].as_u64().unwrap_or(u64::MAX)).map(|n|n+1);
    fields.push(json!({"page":page,"scene":scene,"field":field,"role":role,"id":shape["id"],"rect":shape["box"],"value":shape["text"].as_str().unwrap_or_default()}));
   }
  }
 }
 if count>0 && fields.is_empty(){return Err(invalid("Native storyboard has no recognized editable text fields"));}
 Ok(json!(fields))
}

/// Stage a native text replacement in the existing design draft, then use its Save path.
pub(crate) fn edit_text(s:&mut Session,p:&Value)->Result<Value>{
 let dir=crate::workshop::project_dir(s)?;let board=read(&dir.join("storyboard/movie-board.json"))?;
 let page=p["page"].as_u64().or_else(||p["scene"].as_u64().and_then(|n|n.checked_sub(1)).map(|n|n+board["first_scene_page"].as_u64().unwrap_or(1))).ok_or_else(||invalid("scene or page is required"))?;
 let field=p["field"].as_str().unwrap_or("action");
 let value=p["value"].as_str().ok_or_else(||invalid("value must be text"))?;
 let fields=native_fields(s,&board)?;
 let target=fields.as_array().into_iter().flatten().find(|f|f["page"]==page && f["field"]==field).ok_or_else(||invalid("No such native text field on this page; inspect storyboard.read text_fields"))?;
 let mut doc=inspect(s,&json!({}))?;
 if !doc["text_edits"].is_array(){doc["text_edits"]=json!([]);}
 let edits=doc["text_edits"].as_array_mut().ok_or_else(||invalid("Invalid text draft"))?;
 let mut edit=target.clone();edit["value"]=json!(value);
 if let Some(old)=edits.iter_mut().find(|e|e["page"]==page && e["id"]==target["id"]){*old=edit;}else{edits.push(edit);}
 doc["dirty"]=json!(true);write(&draft(s)?,&doc)?;
 let result=save(s,&json!({}))?;
 Ok(json!({"ok":true,"page":page,"field":field,"value":value,"saved":true,"deck":result["deck"],"pdf":result["pdf"],"dirty":false}))
}

pub(crate) fn base(board:&Value)->Value{
 if board["design_base"].is_object(){return board["design_base"].clone();}
 if let Some(parent)=board["parent_deck"].as_str(){
  if let Some(folder)=Path::new(parent).parent(){if let Ok(b)=read(&folder.join("manifest.json")){return b;}}
 }
 board.clone()
}

pub fn place(s:&mut Session,p:&Value)->Result<Value>{
 let dir=crate::workshop::project_dir(s)?;let board=read(&dir.join("storyboard/movie-board.json"))?;
 let requested=p["scene"].as_u64().map(|scene|board["first_scene_page"].as_u64().unwrap_or(1)+scene.saturating_sub(1)).or_else(||p["page"].as_u64());
 let page=requested.filter(|n|*n>0&&*n<=board["previews"].as_array().map_or(0,Vec::len) as u64).ok_or_else(||invalid("page must identify an existing storyboard page"))?;
 let id=p["asset"].as_str().ok_or_else(||invalid("asset id required"))?;
 let asset=s.project_assets.iter().find(|a|a.id==id).ok_or_else(||invalid("asset is not connected to this project"))?;
 let path=asset.path.clone().ok_or_else(||invalid("asset has no artwork path"))?;
 let (iw,ih)=image::image_dimensions(&path)?;
 let pw=board["page_size"][0].as_f64().unwrap_or(960.0);let ph=board["page_size"][1].as_f64().unwrap_or(540.0);
 let width=p["width"].as_f64().unwrap_or(160.0).clamp(20.0,pw);let height=(width*ih as f64/iw.max(1) as f64).min(ph);
 let width=height*iw as f64/ih.max(1) as f64;
 let x=p["x"].as_f64().filter(|v|v.is_finite()).ok_or_else(||invalid("x must be a page coordinate"))?.clamp(0.0,pw-width);
 let y=p["y"].as_f64().filter(|v|v.is_finite()).ok_or_else(||invalid("y must be a page coordinate"))?.clamp(0.0,ph-height);
 let mut doc=inspect(s,&json!({}))?;
 let entries=doc["placements"].as_array_mut().ok_or_else(||invalid("invalid design draft"))?;
 let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
 let key=p["placement"].as_str().map(str::to_owned).unwrap_or_else(||format!("placement-{stamp}"));
 let item=json!({"id":key,"asset":id,"path":path,"page":page,"scene":page.checked_sub(board["first_scene_page"].as_u64().unwrap_or(1)).map(|n|n+1),"rect":[x,y,width,height]});
 if p["placement"].is_string(){let entry=entries.iter_mut().find(|e|e["id"]==key).ok_or_else(||invalid("placement not found"))?;*entry=item.clone();}else{entries.push(item.clone());}
 let history=dir.join("storyboard/design-history");std::fs::create_dir_all(&history)?;
 write(&history.join(format!("{stamp}.json")),&inspect(s,&json!({}))?)?;
 doc["dirty"]=json!(true);write(&draft(s)?,&doc)?;Ok(json!({"ok":true,"placement":item,"draft":draft(s)?}))
}
pub fn save(s:&mut Session,_:&Value)->Result<Value>{
 let dir=crate::workshop::project_dir(s)?;let active=dir.join("storyboard/movie-board.json");let mut board=read(&active)?;
 let design=inspect(s,&json!({}))?;let entries=design["placements"].as_array().ok_or_else(||invalid("Invalid placements"))?;
 let mut baseline=base(&board);
 let original=baseline["deck"].as_str().ok_or_else(||invalid("storyboard has no editable deck"))?.to_owned();
 let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
 let out=dir.join(format!("storyboard/design-{stamp}"));std::fs::create_dir_all(&out)?;
 let deck=out.join("movie-board.deckcraft");let pdf=out.join("movie-board.pdf");
 let edits=design["text_edits"].as_array().cloned().unwrap_or_default();
 let mut args=vec!["run".into(),"--in".into(),original.clone()];
 for edit in &edits{
  args.extend(["--cmd".into(),format!("slide.go={}",json!({"index":edit["page"].as_u64().unwrap_or(1)-1})),"--cmd".into(),format!("text.set={}",json!({"id":edit["id"],"text":edit["value"]}))]);
 }
 if !edits.is_empty(){
  // Render the same document with text edits, before draft picture overlays are inserted.
  // This keeps existing image drag/drop behaviour and avoids duplicating saved pictures.
  let text_base=out.join("text-base.deckcraft");
  args.extend(["--save".into(),text_base.to_string_lossy().into_owned()]);
  crate::workspace::run_tool_in(&dir,"deckcraft-cli",&args)?;
  let pages=out.join("text-pages");
  crate::workspace::run_tool_in(&dir,"deckcraft-cli",&["render".into(),text_base.to_string_lossy().into_owned(),"--all".into(),"--scale".into(),"1.5".into(),pages.to_string_lossy().into_owned()])?;
  let mut previews=std::fs::read_dir(&pages)?.filter_map(|e|e.ok().map(|e|e.path())).filter(|p|p.extension().is_some_and(|e|e=="png")).collect::<Vec<_>>();previews.sort();
  baseline["previews"]=json!(previews);
  args=vec!["run".into(),"--in".into(),text_base.to_string_lossy().into_owned()];
 }
 for item in entries {
  args.extend(["--cmd".into(),format!("slide.go={}",json!({"index":item["page"].as_u64().unwrap_or(1)-1})),"--cmd".into(),format!("insert.picture={}",json!({"path":item["path"],"rect":item["rect"],"name":format!("Design · {}",item["asset"])}))]);
 }
 args.extend(["--save".into(),deck.to_string_lossy().into_owned(),"--export".into(),pdf.to_string_lossy().into_owned()]);
 crate::workspace::run_tool_in(&dir,"deckcraft-cli",&args)?;
 let pages=out.join("pages");crate::workspace::run_tool_in(&dir,"deckcraft-cli",&["render".into(),deck.to_string_lossy().into_owned(),"--all".into(),"--scale".into(),"1.5".into(),pages.to_string_lossy().into_owned()])?;
 let mut previews=std::fs::read_dir(&pages)?.filter_map(|e|e.ok().map(|e|e.path())).filter(|p|p.extension().is_some_and(|e|e=="png")).collect::<Vec<_>>();previews.sort();
 board["parent_deck"]=board["deck"].clone();board["deck"]=json!(deck);board["pdf"]=json!(pdf);board["previews"]=json!(previews);board["design_base"]=baseline;board["design"]=json!({"placements":entries,"text_edits":edits,"dirty":false});
 write(&out.join("manifest.json"),&board)?;write(&active,&board)?;write(&draft(s)?,&json!({"placements":[],"dirty":false}))?;
 Ok(json!({"ok":true,"deck":deck,"pdf":pdf,"previous_deck":board["parent_deck"],"dirty":false,"placements":entries}))
}

pub fn remove(s:&mut Session,p:&Value)->Result<Value>{
 let id=p["placement"].as_str().ok_or_else(||invalid("placement id required"))?;
 let mut doc=inspect(s,&json!({}))?;let entries=doc["placements"].as_array_mut().ok_or_else(||invalid("invalid design draft"))?;
 let before=entries.len();entries.retain(|v|v["id"].as_str()!=Some(id));
 if before==entries.len(){return Err(invalid("placement not found"));}
 doc["dirty"]=json!(true);write(&draft(s)?,&doc)?;Ok(json!({"ok":true,"removed":id}))
}
