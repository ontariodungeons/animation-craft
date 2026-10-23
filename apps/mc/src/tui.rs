//! Local control of a specific live TUI. Replies are sent after the resulting frame is drawn.
use serde_json::{Value,json};
use std::{fs,io::{BufRead,BufReader,Read,Write},os::unix::{fs::{FileTypeExt,MetadataExt,PermissionsExt},net::{UnixListener,UnixStream}},path::PathBuf,time::Duration};

fn root_directory()->Result<PathBuf,String>{
 let uid=fs::metadata("/proc/self").map_err(|e|e.to_string())?.uid();
 let root=std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(||std::env::temp_dir().join(format!("magicstudio-{uid}")));
 let dir=root.join("magicstudio-tui");
 fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
 let meta=fs::symlink_metadata(&dir).map_err(|e|e.to_string())?;
 if meta.file_type().is_symlink()||!meta.is_dir()||meta.uid()!=uid{return Err("TUI runtime directory must be an owned directory, not a symlink".into());}
 fs::set_permissions(&dir,fs::Permissions::from_mode(0o700)).map_err(|e|e.to_string())?;
 Ok(dir)
}

fn private_directory(path:PathBuf)->Result<PathBuf,String>{
 match fs::create_dir(&path){Ok(())=>{},Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists=>{},Err(e)=>return Err(e.to_string())}
 let meta=fs::symlink_metadata(&path).map_err(|e|e.to_string())?;let uid=fs::metadata("/proc/self").map_err(|e|e.to_string())?.uid();
 if !meta.is_dir()||meta.file_type().is_symlink()||meta.uid()!=uid{return Err("TUI scope directory must be owned and not a symlink".into());}
 fs::set_permissions(&path,fs::Permissions::from_mode(0o700)).map_err(|e|e.to_string())?;Ok(path)
}
fn directory()->Result<PathBuf,String>{
 let root=root_directory()?;if let Some(scope)=crate::scope::current()?{let booths=private_directory(root.join("booths"))?;private_directory(booths.join(scope.key()))}else{Ok(root)}
}
fn sockets(directory:&std::path::Path)->Vec<PathBuf>{
 fs::read_dir(directory).into_iter().flatten().filter_map(Result::ok).map(|e|e.path()).filter(|p|p.extension().is_some_and(|e|e=="sock"))
 .filter(|p|p.file_stem().and_then(|v|v.to_str()).and_then(|v|v.parse::<u32>().ok()).is_some_and(|pid|std::path::Path::new(&format!("/proc/{pid}")).exists())).collect()
}

pub struct Server{listener:UnixListener,path:PathBuf,pub generation:u128}
pub struct Pending{pub stream:UnixStream,pub request:Value}
impl Server{
 pub fn bind()->Result<Self,String>{
  let path=directory()?.join(format!("{}.sock",std::process::id()));
  if let Ok(meta)=fs::symlink_metadata(&path){if !meta.file_type().is_socket(){return Err("TUI endpoint exists and is not a socket".into());}fs::remove_file(&path).map_err(|e|e.to_string())?;}
  let listener=UnixListener::bind(&path).map_err(|e|e.to_string())?;
  fs::set_permissions(&path,fs::Permissions::from_mode(0o600)).map_err(|e|e.to_string())?;
  listener.set_nonblocking(true).map_err(|e|e.to_string())?;
  Ok(Self{listener,path,generation:std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|v|v.as_micros()).unwrap_or(0)})
 }
 pub fn receive(&self)->Vec<Pending>{
  let mut pending=Vec::new();
  for _ in 0..1{
   let Ok((mut stream,_))=self.listener.accept()else{break;};
   let _=stream.set_read_timeout(Some(Duration::from_millis(150)));
   let _=stream.set_write_timeout(Some(Duration::from_secs(2)));
   let mut text=String::new();
   let read=BufReader::new((&mut stream).take(65537)).read_line(&mut text);
   match read.and_then(|_|if text.len()>65536{Err(std::io::Error::other("request exceeds 64KiB"))}else{serde_json::from_str(&text).map_err(std::io::Error::other)}){
    Ok(request)=>pending.push(Pending{stream,request}),
    Err(error)=>{let _=writeln!(stream,"{}",json!({"ok":false,"error":error.to_string()}));}
   }
  }
  pending
 }
 pub fn endpoint(&self)->String{self.path.to_string_lossy().into_owned()}
}
impl Drop for Server{fn drop(&mut self){let _=fs::remove_file(&self.path);}}
impl Pending{pub fn reply(mut self,value:&Value){let _=writeln!(self.stream,"{value}");}}

fn request(path:&std::path::Path,action:&str,params:&Value)->Result<Value,String>{
 let mut stream=UnixStream::connect(path).map_err(|e|format!("{}: {e}",path.display()))?;
 stream.set_read_timeout(Some(Duration::from_secs(30))).map_err(|e|e.to_string())?;
 stream.set_write_timeout(Some(Duration::from_secs(3))).map_err(|e|e.to_string())?;
 writeln!(stream,"{}",json!({"action":action,"params":params})).map_err(|e|e.to_string())?;
 let mut text=String::new();
 BufReader::new(stream.take(8*1024*1024)).read_line(&mut text).map_err(|e|format!("Live TUI did not reply: {e}. Nothing was resent; use look before retrying a mutation."))?;
 serde_json::from_str(&text).map_err(|e|format!("Invalid live TUI reply: {e}"))
}
pub fn call(action:&str,params:&Value)->Result<Value,String>{
 if params.get("section").is_some()&&!matches!(action,"look"|"read"|"status"|"controls"){return Err("section= filters observations only; read the resulting frame with tui look".into());}
 let dir=directory()?;
 let mut endpoints=sockets(&dir);
 if crate::scope::current()?.is_none(){
  let booths=dir.join("booths");
  for entry in fs::read_dir(booths).into_iter().flatten().filter_map(Result::ok){if entry.file_type().is_ok_and(|t|t.is_dir()){endpoints.extend(sockets(&entry.path()));}}
 }
 endpoints.sort();
 if action=="instances"{
  let values:Vec<_>=endpoints.iter().map(|path|match request(path,"look",&json!({})){Ok(v)=>json!({"instance":v["instance"],"view":v["view"],"project":v["project"],"role":v["access"]["role"],"endpoint":path}),Err(e)=>json!({"endpoint":path,"error":e})}).collect();
  return Ok(json!({"ok":true,"instances":values}));
 }
 let selected=params["instance"].as_u64().map(|v|v.to_string()).or_else(||params["instance"].as_str().map(str::to_owned));
 if params.get("instance").is_some()&&selected.is_none(){return Err("instance= must be a TUI process ID".into());}
 let path=if let Some(pid)=selected{
  if pid.parse::<u32>().is_err(){return Err("instance must be a TUI process ID".into());}
  endpoints.iter().find(|p|p.file_stem().and_then(|s|s.to_str())==Some(pid.as_str())).cloned().ok_or_else(||format!("TUI {pid} is outside this context or not running; use mc tui instances"))?
 }else{let masters:Vec<_>=endpoints.iter().filter(|p|p.parent()==Some(dir.as_path())).cloned().collect();let choices=if masters.is_empty(){&endpoints}else{&masters};match choices.as_slice(){[one]=>one.clone(),[]=>return Err("No controllable TUI is running. Start the updated mc booth in your terminal.".into()),_=>return Err("Several TUIs are running. Use mc tui instances, then instance=<pid>. Nothing was changed.".into())}};
 let mut value=request(&path,action,params)?;
 if action=="reload"&&value["ok"]==true{
  let old=value["generation"].clone();
  let until=std::time::Instant::now()+Duration::from_secs(30);
  loop{
   if let Ok(mut fresh)=request(&path,"look",&json!({})){
    if fresh["generation"]!=old{fresh["reloaded"]=json!(true);value=fresh;break;}
   }
   if std::time::Instant::now()>=until{return Err("Reload accepted, but a fresh TUI frame was not observed within 30 seconds".into());}
   std::thread::sleep(Duration::from_millis(100));
  }
 }
 if let Some(section)=params["section"].as_str().or(if action=="controls"{Some("controls")}else{None}){
  let selected=value.get(section).cloned().ok_or_else(||format!("Unknown look section {section}"))?;
  let mut reply=json!({"ok":value["ok"],"error":value["error"],"instance":value["instance"],"frame":value["frame"],"generation":value["generation"],"view":value["view"],"project":value["project"]});
  reply[section]=selected;return Ok(reply);
 }
 Ok(value)
}

pub const CONTROLS:&[(&str,&str)]=&[
 ("look","Rendered terminal text, structured state and this control list"),
 ("click id=<visible control id>","Click a visible button using the same hit target as the mouse; look section=elements lists IDs and rectangles"),
 ("view name=workshop|booth|mops|animal","F1/F2/F3/F4"),
 ("agent pane=<id> | name=<name> | index=<n>","Select an agent and show MOPS; G opens MOPS, 1–9/0 select, left/right cycle"),
 ("booth.prepare [project=<file>] [pane=<id> name=<name> task=<text>] [allow=<command IDs>]","Master only: prepare an assignment and onboarding command (B in Workshop); never launches or messages an agent"),
 ("project.select path=<file> | index=<n>","Select a Workshop card without attaching; arrows/j/k move, Enter/Open enters Booth"),
 ("project.open-selected","Open the selected Workshop project in Booth; does not switch a Studio"),
 ("project.inactive [show=true|false]","Show/hide inactive project cards (I)"),
 ("project path=<file> | index=<n>","Choose a catalog project and observe it in Booth"),
 ("stage name=<label> | index=<n>","Select a project stage"),
 ("event sequence=<id> | index=<n>","Select and inspect an event"),
 ("events","Focus the event list (E)"),
 ("agent.next | agent.previous","Cycle the selected agent (, / .)"),
 ("remind [pane=<id> | name=<name>]","Request status from an available agent; refuse working agents (P)"),
 ("command index=<n>","Select an offered command"),
 ("copy","Copy the selected literal command; never execute it"),
 ("note text=<text> [append=true]","Compose a note; does not send"),
 ("note.send","Submit the current note if the recipient is available"),
 ("note.cancel","Cancel the note composer"),
 ("chat [text=<text>] [append=true]","Open F9 Sproket project chat; typing does not send"),
 ("chat.close","Close F9 Sproket chat and retain its draft (F9/Esc)"),
 ("chat.send","Submit project chat"),
 ("input field=<name> value=<text>","Edit an intake field"),
 ("intake.open | intake.new | intake.create | intake.resume","F6 / form F2 / F5 / F6"),
 ("project.open | project.save | storyboard.open","Form F3 / Ctrl+S / form F7"),
 ("doctor | details | refresh","F10 / D / F8"),
 ("scroll rows=<signed count>","Scroll the current panel"),
 ("key key=<key>","Any keyboard control: F1–F12, ctrl+s, arrows, enter, esc, tab, pageup/pagedown, or one character"),
 ("text text=<text>","Type into the active composer or intake field without submitting"),
 ("cancel | submit","Escape / Enter in the current context"),
 ("reload | quit","Reload this exact TUI / close it; drafts must be resolved first"),
];
