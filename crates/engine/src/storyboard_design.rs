//! Scene composition drafts and non-destructive DeckCraft versions.
use crate::{EngineError,Result,Session};
use serde_json::{Value,json};
use std::path::Path;
fn invalid(s:&str)->EngineError{EngineError::InvalidParams(s.into())}
fn read(path:&Path)->Result<Value>{serde_json::from_slice(&std::fs::read(path)?).map_err(|e|invalid(&e.to_string()))}
fn write(path:&Path,value:&Value)->Result<()>{std::fs::write(path,serde_json::to_vec_pretty(value).map_err(|e|invalid(&e.to_string()))?)?;Ok(())}
fn draft(s:&Session)->Result<std::path::PathBuf>{Ok(crate::workshop::project_dir(s)?.join("storyboard/design-draft.json"))}
pub fn inspect(s:&mut Session,_:&Value)->Result<Value>{
 let file=draft(s)?;
 if file.exists(){let doc=read(&file)?;if doc["dirty"]==true || doc["placements"].as_array().is_some_and(|v|!v.is_empty()){return Ok(doc);}}
 let board=crate::workshop::project_dir(s)?.join("storyboard/movie-board.json");
 if board.exists(){let b=read(&board)?;if b["design"]["placements"].is_array(){return Ok(b["design"].clone());}}
 Ok(json!({"placements":[]}))
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
 let baseline=base(&board);
 let original=baseline["deck"].as_str().ok_or_else(||invalid("storyboard has no editable deck"))?.to_owned();
 let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
 let out=dir.join(format!("storyboard/design-{stamp}"));std::fs::create_dir_all(&out)?;
 let deck=out.join("movie-board.deckcraft");let pdf=out.join("movie-board.pdf");
 let mut args=vec!["run".into(),"--in".into(),original.clone()];
 for item in entries {
  args.extend(["--cmd".into(),format!("slide.go={}",json!({"index":item["page"].as_u64().unwrap_or(1)-1})),"--cmd".into(),format!("insert.picture={}",json!({"path":item["path"],"rect":item["rect"],"name":format!("Design · {}",item["asset"])}))]);
 }
 args.extend(["--save".into(),deck.to_string_lossy().into_owned(),"--export".into(),pdf.to_string_lossy().into_owned()]);
 crate::workspace::run_tool_in(&dir,"deckcraft-cli",&args)?;
 let pages=out.join("pages");crate::workspace::run_tool_in(&dir,"deckcraft-cli",&["render".into(),deck.to_string_lossy().into_owned(),"--all".into(),"--scale".into(),"1.5".into(),pages.to_string_lossy().into_owned()])?;
 let mut previews=std::fs::read_dir(&pages)?.filter_map(|e|e.ok().map(|e|e.path())).filter(|p|p.extension().is_some_and(|e|e=="png")).collect::<Vec<_>>();previews.sort();
 board["parent_deck"]=board["deck"].clone();board["deck"]=json!(deck);board["pdf"]=json!(pdf);board["previews"]=json!(previews);board["design_base"]=baseline;board["design"]=design;
 write(&out.join("manifest.json"),&board)?;write(&active,&board)?;write(&draft(s)?,&json!({"placements":[],"dirty":false}))?;
 Ok(json!({"ok":true,"deck":deck,"pdf":pdf,"previous_deck":original}))
}

pub fn remove(s:&mut Session,p:&Value)->Result<Value>{
 let id=p["placement"].as_str().ok_or_else(||invalid("placement id required"))?;
 let mut doc=inspect(s,&json!({}))?;let entries=doc["placements"].as_array_mut().ok_or_else(||invalid("invalid design draft"))?;
 let before=entries.len();entries.retain(|v|v["id"].as_str()!=Some(id));
 if before==entries.len(){return Err(invalid("placement not found"));}
 doc["dirty"]=json!(true);write(&draft(s)?,&doc)?;Ok(json!({"ok":true,"removed":id}))
}
