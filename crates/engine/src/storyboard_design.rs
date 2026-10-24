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

fn pdf_run(s:&Session,steps:Value)->Result<Value>{
 let dir=crate::workshop::project_dir(s)?;let folder=dir.join("storyboard/.pdfcraft");std::fs::create_dir_all(&folder)?;
 let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
 let script=folder.join(format!("{stamp}-{}.json",std::process::id()));write(&script,&steps)?;
 let output=crate::workspace::run_tool_in(&dir,"pdfcraft-cli",&["run".into(),"--script".into(),script.to_string_lossy().into_owned(),"--report".into()]);
 let _=std::fs::remove_file(&script);
 let output=output?;
 let report:Value=serde_json::from_str(&output).map_err(|e|invalid(&format!("Invalid PDFCraft report: {e}")))?;
 if report["ok"]!=true{return Err(invalid(&format!("PDFCraft failed: {report}")));}
 Ok(report)
}

fn file_cache_key(path:&str,extra:&str)->Option<(String,u128,u64,String)>{
 let meta=std::fs::metadata(path).ok()?;
 let mtime=meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
 Some((path.to_owned(),mtime,meta.len(),extra.to_owned()))
}

fn native_fields_cache()->&'static std::sync::Mutex<std::collections::HashMap<(String,u128,u64,String),Value>>{
 static CACHE:std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<(String,u128,u64,String),Value>>>=std::sync::OnceLock::new();
 CACHE.get_or_init(||std::sync::Mutex::new(std::collections::HashMap::new()))
}
fn pdf_previews(s:&Session,pdf:&Path,folder:&Path,count:usize)->Result<Vec<std::path::PathBuf>>{
 std::fs::create_dir_all(folder)?;let mut paths=Vec::new();
 for page in 1..=count{let path=folder.join(format!("slide-{page:03}.png"));crate::workspace::run_tool_in(&crate::workshop::project_dir(s)?,"pdfcraft-cli",&["render".into(),pdf.to_string_lossy().into_owned(),"--page".into(),page.to_string(),"--dpi".into(),"72".into(),"--out".into(),path.to_string_lossy().into_owned()])?;paths.push(path);}
 Ok(paths)
}
fn save_pdf(s:&mut Session,mut board:Value,design:&Value)->Result<Value>{
 let dir=crate::workshop::project_dir(s)?;let mut baseline=base(&board);let original=baseline["pdf"].as_str().ok_or_else(||invalid("Storyboard PDF is missing"))?;
 let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();let out=dir.join(format!("storyboard/design-{stamp}"));std::fs::create_dir_all(&out)?;
 let pdf=out.join("movie-board.pdf");let text_base=out.join("text-base.pdf");
 let mut values=serde_json::Map::new();for edit in design["text_edits"].as_array().into_iter().flatten(){values.insert(edit["name"].as_str().ok_or_else(||invalid("Missing PDF field name"))?.into(),edit["value"].clone());}
 let mut steps=vec![json!({"tool":"doc_open","args":{"path":original}})];
 if !values.is_empty(){steps.push(json!({"tool":"form_fill","args":{"doc":1,"values":values}}));}
 steps.push(json!({"tool":"doc_save","args":{"doc":1,"path":text_base}}));
 for item in design["placements"].as_array().into_iter().flatten(){let r=&item["rect"];let x=r[0].as_f64().unwrap_or(0.0);let y=r[1].as_f64().unwrap_or(0.0);let w=r[2].as_f64().unwrap_or(0.0);let h=r[3].as_f64().unwrap_or(0.0);steps.push(json!({"tool":"page_add_image","args":{"doc":1,"page":item["page"],"path":item["path"],"rect":[x,y,x+w,y+h]}}));}
 steps.push(json!({"tool":"doc_save","args":{"doc":1,"path":pdf}}));pdf_run(s,json!(steps))?;
 let count=board["previews"].as_array().map_or(0,Vec::len);let previews=pdf_previews(s,&pdf,&out.join("pages"),count)?;
 baseline["previews"]=json!(pdf_previews(s,&text_base,&out.join("text-pages"),count)?);
 board["parent_pdf"]=board["pdf"].clone();board["pdf"]=json!(pdf);board["previews"]=json!(previews);board["design_base"]=baseline;board["design"]=design.clone();board["design"]["dirty"]=json!(false);
 write(&out.join("manifest.json"),&board)?;write(&dir.join("storyboard/movie-board.json"),&board)?;write(&draft(s)?,&json!({"placements":[],"dirty":false}))?;
 Ok(json!({"ok":true,"pdf":pdf,"dirty":false,"placements":design["placements"],"editable_pdf":true}))
}

/// Inspect the existing native document; role names and boxes come from its objects.
pub(crate) fn native_fields(s:&Session,board:&Value)->Result<Value>{
 if let Some(fields)=board["pdf_fields"].as_array(){
  let pdf=board["pdf"].as_str().ok_or_else(||invalid("Editable PDF is missing"))?;
  let key=file_cache_key(pdf,&board["pdf_fields"].to_string());
  if let Some(ref k)=key && let Ok(guard)=native_fields_cache().lock() && let Some(cached)=guard.get(k){
   return Ok(cached.clone());
  }
  let report=pdf_run(s,json!([{"tool":"doc_open","args":{"path":pdf}},{"tool":"form_fields","args":{"doc":1}}]))?;
  let actual=report["steps"][1]["result"][0]["fields"].as_array().ok_or_else(||invalid("PDFCraft returned no form fields"))?;
  let mut result=Vec::new();for metadata in fields{let field=actual.iter().find(|f|f["name"]==metadata["name"]).ok_or_else(||invalid("Saved PDF field missing; refusing stale metadata"))?;let mut item=metadata.clone();item["value"]=field["value"].clone();result.push(item);}
  let val=json!(result);
  if let Some(k)=key && let Ok(mut guard)=native_fields_cache().lock(){if guard.len()>32{guard.clear();}guard.insert(k,val.clone());}
  return Ok(val);
 }

 let Some(deck)=board["deck"].as_str().filter(|p|Path::new(p).is_file()) else{return Ok(json!([]));};
 let count=board["previews"].as_array().map_or(0,Vec::len);
 let first_scene=board["first_scene_page"].as_u64().unwrap_or(u64::MAX);
 let key=file_cache_key(deck,&format!("{count}:{first_scene}"));
 if let Some(ref k)=key && let Ok(guard)=native_fields_cache().lock() && let Some(cached)=guard.get(k){
  return Ok(cached.clone());
 }
 let dir=crate::workshop::project_dir(s)?;
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
    let scene=page.checked_sub(first_scene).map(|n|n+1);
    fields.push(json!({"page":page,"scene":scene,"field":field,"role":role,"id":shape["id"],"rect":shape["box"],"value":shape["text"].as_str().unwrap_or_default()}));
   }
  }
 }
 if count>0 && fields.is_empty(){return Err(invalid("Native storyboard has no recognized editable text fields"));}
 let val=json!(fields);
 if let Some(k)=key && let Ok(mut guard)=native_fields_cache().lock(){if guard.len()>32{guard.clear();}guard.insert(k,val.clone());}
 Ok(val)
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
 // save=false keeps the edit in the draft so a batch ends with one storyboard.save_version: each saved
 // version is a whole document (tens of MB), so saving per field multiplied it per edit.
 if p["save"]==json!(false){return Ok(json!({"ok":true,"page":page,"field":field,"value":value,"saved":false,"dirty":true}));}
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
pub fn save(s:&mut Session,p:&Value)->Result<Value>{
 if let Some(path)=p["draft"].as_str(){
  let dir=crate::workshop::project_dir(s)?;let path=crate::workspace::resolve_workspace_path(s,path).canonicalize()?;
  if !path.starts_with(dir.join("storyboard").canonicalize()?) || path.file_name().and_then(|n|n.to_str())!=Some("manifest.json"){return Err(invalid("Draft must be this project's generated storyboard manifest"));}
  let mut board=read(&path)?;
  if board["project"]!=json!(s.project_path) || board["draft"]!=true{return Err(invalid("Not a draft for this open project"));}
  if board["mapping"]["needs_correction"]==true{return Err(invalid(&format!("Draft mapping needs correction; expected template and unmapped source: {}. Resume: {}",board["mapping"],board["resume_command"])));}
  if !board["deck"].as_str().is_some_and(|p|Path::new(p).is_file()) || board["previews"].as_array().is_none_or(|xs|xs.is_empty()||xs.iter().any(|p|!p.as_str().is_some_and(|v|Path::new(v).is_file()))){return Err(invalid("Draft native deck or rendered previews are missing"));}
  if inspect(s,&json!({}))?["dirty"]==true{return Err(invalid("Save pending design edits before adopting a generated draft"));}
  board["draft"]=json!(false);board["status"]=json!("saved after draft review");
  write(&dir.join("storyboard/movie-board.json"),&board)?;
  return Ok(json!({"ok":true,"deck":board["deck"],"pdf":board["pdf"],"dirty":false,"saved_draft":path}));
 }
 let dir=crate::workshop::project_dir(s)?;let active=dir.join("storyboard/movie-board.json");let mut board=read(&active)?;
 let design=inspect(s,&json!({}))?;if board["pdf_fields"].is_array(){return save_pdf(s,board,&design);}
 let entries=design["placements"].as_array().ok_or_else(||invalid("Invalid placements"))?;
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
