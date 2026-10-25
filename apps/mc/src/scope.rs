//! Project workflow permissions. Shared-user filesystem access is outside this boundary.
use serde_json::{Value,json};
use std::{path::{Path,PathBuf},sync::OnceLock,os::unix::fs::{MetadataExt,PermissionsExt}};

pub const ENV:&str="MAGICSTUDIO_ASSIGNMENT";
const READ:&[&str]=&["project.identity","project.attach","center.status","backstage.status","workshop.status","center.events","backstage.events","center.snapshot","center.tail","booth.snapshot","booth.tail","backstage.snapshot","backstage.tail","ui.feedback","agent.session","agent.log","blender.list","blender.status","command.list","skills.list","skills.info","skills.limits"];
#[derive(Clone,Debug)]
pub struct Scope{pub file:PathBuf,pub project:PathBuf,pub members:Vec<Value>,pub allow:Vec<String>}
impl Scope{
 pub fn from_value(file:PathBuf,value:&Value)->Result<Self,String>{
  if value["schema"]!="magicstudio.assignment/1"{return Err("Unknown Booth assignment schema".into());}
  let project=std::fs::canonicalize(value["project"].as_str().ok_or("Assignment requires a project")?).map_err(|e|e.to_string())?;
  if !project.is_file(){return Err("Assignment project must be an existing file".into());}
  let members=value["members"].as_array().cloned().unwrap_or_default();
  if members.iter().any(|m|m["pane"].as_str().is_none_or(str::is_empty)||m["task"].as_str().is_none()){return Err("Each project member requires pane and task".into());}
  let allow=value["allow"].as_array().into_iter().flatten().map(|v|v.as_str().map(str::to_owned).ok_or("allow entries must be command IDs")).collect::<Result<Vec<_>,_>>()?;
  if allow.iter().any(|id|master_command(id)||magicstudio_engine::find_command(id).is_none()){return Err("Assignment cannot grant master/global commands or unknown command IDs".into());}
  Ok(Self{file,project,members,allow})
 }
 pub fn load(file:&Path)->Result<Self,String>{
  let meta=std::fs::symlink_metadata(file).map_err(|e|e.to_string())?;
  let uid=std::fs::metadata("/proc/self").map_err(|e|e.to_string())?.uid();
  if !meta.is_file()||meta.file_type().is_symlink()||meta.uid()!=uid||meta.permissions().mode()&0o077!=0{return Err("Assignment must be an owned private regular file (0600)".into());}
  let bytes=std::fs::read(file).map_err(|e|e.to_string())?;if bytes.len()>65536{return Err("Assignment exceeds 64KiB".into());}
  Self::from_value(std::fs::canonicalize(file).map_err(|e|e.to_string())?,&serde_json::from_slice(&bytes).map_err(|e|e.to_string())?)
 }
 pub fn permits(&self,id:&str)->bool{!master_command(id)&&(READ.contains(&id)||self.allow.iter().any(|v|v==id))}
 pub fn params(&self,id:&str,mut params:Value)->Result<Value,String>{
  if !self.permits(id){return Err(format!("Restricted: {id} is not granted to this project Booth"));}
  if !params.is_object(){return Err("Command parameters must be an object".into());}
  for field in ["project","_expected_project"]{
   if let Some(value)=params.get(field){let path=value.as_str().ok_or("Project context must be a path")?;if std::fs::canonicalize(path).map_err(|_|"Restricted: project identity is unavailable")?!=self.project{return Err("Restricted: another project is outside this Booth".into());}}
  }
  if matches!(id,"project.open"|"project.attach"|"project.identity"|"project.save"){
   if let Some(value)=params.get("path"){let path=value.as_str().ok_or("Project path must be text")?;if std::fs::canonicalize(path).map_err(|_|"Restricted: project path is unavailable")?!=self.project{return Err("Restricted: cannot switch this Booth's project".into());}}
   params["path"]=json!(self.project);
  }
  params["project"]=json!(self.project);params["_expected_project"]=json!(self.project);Ok(params)
 }
 pub fn commands(&self,filter:&str)->Value{
  let commands:Vec<_>=magicstudio_engine::command_specs().iter().filter(|s|self.permits(s.id)&&s.id.contains(filter)).map(|s|json!({"id":s.id,"label":s.label,"params":s.params})).collect();
  json!({"ok":true,"project":self.project,"role":"project","commands":commands})
 }
 pub fn catalog(&self)->Value{json!({"projects":[{"name":self.project.file_stem().and_then(|v|v.to_str()),"path":self.project,"project_id":self.project,"exists":true}],"scope":"assigned project only"})}
 pub fn member(&self,pane:&str)->Option<&Value>{self.members.iter().find(|m|m["pane"].as_str()==Some(pane))}
 pub fn roster(&self,agents:Vec<Value>)->Vec<Value>{
  self.members.iter().map(|member|{let pane=member["pane"].as_str().unwrap_or("");let live=agents.iter().find(|a|a["pane_id"].as_str()==Some(pane));
   json!({"pane_id":pane,"name":member["name"].as_str().unwrap_or(pane),"agent":live.map(|a|a["agent"].clone()),"agent_status":live.map(|a|a["agent_status"].clone()).unwrap_or(json!("unknown")),"terminal_title_stripped":member["task"],"task_source":"Workshop assignment","cwd":self.project.parent()})
  }).collect()
 }
 pub fn redact(&self,value:Value)->Value{
  match value{
   Value::Object(mut map)=>{
    if ["project","active_project","canonical_project"].iter().any(|key|map.get(*key).and_then(Value::as_str).is_some_and(|p|Path::new(p)!=self.project)){return Value::Null;}
    for key in ["worker_owner","worker_owner_error","discovery_errors","other_instances"]{map.remove(key);}
    if map.contains_key("connection"){map.retain(|key,_|["ok","connection","project","active_project","instance","endpoint","ui_ready","engine_unsaved","unsaved_edits","dirty_reasons","engine_revision","ack_engine_revision","frontend_report_age_ms","project_guard_version"].contains(&key.as_str()));}
    for key in ["commands","next_commands"]{if let Some(Value::Array(rows))=map.get_mut(key){rows.retain(|row|row["id"].as_str().or(row["command"].as_str()).is_some_and(|id|self.permits(id)));}}
    if map.contains_key("catalog"){map.insert("catalog".into(),self.catalog());}
    if map.contains_key("projects"){map.insert("projects".into(),self.catalog()["projects"].clone());}
    Value::Object(map.into_iter().map(|(k,v)|(k,self.redact(v))).collect())
   },
   Value::Array(rows)=>Value::Array(rows.into_iter().map(|v|self.redact(v)).filter(|v|!v.is_null()).collect()),other=>other,
  }
 }
 pub fn inherit(&self,command:&mut std::process::Command){command.env(ENV,&self.file);}
 pub fn launch(&self)->String{format!("{}={} mc booth",ENV,quote(&self.file.to_string_lossy()))}
 pub fn key(&self)->String{project_key(&self.project)}
}
fn master_command(id:&str)->bool{
 if READ.contains(&id)||matches!(id,"project.open"|"project.save"|"agent.send"){return false;}
 let project_namespace=["workshop.","storyboard.","motion.","image.","asset.","assets.","audio.","video.","blender.","character.","rig.","clip.","caption.","transcript."];
 !project_namespace.iter().any(|prefix|id.starts_with(prefix))||[".settings",".configure",".provider",".login",".logout",".install",".global"].iter().any(|part|id.contains(part))
}

fn quote(value:&str)->String{format!("'{}'",value.replace('\'',"'\\''"))}
fn project_key(path:&Path)->String{
 let hash=path.as_os_str().as_encoded_bytes().iter().fold(0xcbf29ce484222325u64,|hash,b|(hash^u64::from(*b)).wrapping_mul(0x100000001b3));format!("{hash:016x}")
}
static ACTIVE:OnceLock<Result<Option<Scope>,String>>=OnceLock::new();
pub fn current()->Result<Option<&'static Scope>,String>{
 ACTIVE.get_or_init(||{
  let file=std::env::var_os(ENV).map(PathBuf::from).or_else(||{
   let args=crate::cli_args();if !args.iter().any(|a|matches!(a.as_str(),"booth"|"center"|"backstage")){return None;}
   args.iter().find_map(|a|a.strip_prefix("assignment=").map(PathBuf::from))
  });
  file.map(|file|Scope::load(&file)).transpose()
 }).as_ref().map(|s|s.as_ref()).map_err(Clone::clone)
}
pub fn prepare(project:&Path,params:&Value)->Result<Value,String>{
 if current()?.is_some(){return Err("Restricted: only the master Workshop prepares agent Booths".into());}
 let project=std::fs::canonicalize(project).map_err(|e|e.to_string())?;if !project.is_file(){return Err("Choose an existing project".into());}
 let root=std::env::var_os("XDG_STATE_HOME").map(PathBuf::from).or_else(||std::env::var_os("HOME").map(|p|PathBuf::from(p).join(".local/state"))).ok_or("State directory unavailable")?;
 let directory=root.join("magic-studio/assignments");std::fs::create_dir_all(&directory).map_err(|e|e.to_string())?;let meta=std::fs::symlink_metadata(&directory).map_err(|e|e.to_string())?;if !meta.is_dir()||meta.file_type().is_symlink(){return Err("Assignment directory must not be a symlink".into());}std::fs::set_permissions(&directory,std::fs::Permissions::from_mode(0o700)).map_err(|e|e.to_string())?;
 let file=directory.join(format!("{}.json",project_key(&project)));
 let mut value=if file.exists(){let existing=Scope::load(&file)?;json!({"schema":"magicstudio.assignment/1","project":project,"members":existing.members,"allow":existing.allow})}else{json!({"schema":"magicstudio.assignment/1","project":project,"members":[],"allow":[]})};
 if let Some(pane)=params["pane"].as_str(){
  let task=params["task"].as_str().filter(|v|!v.trim().is_empty()).ok_or("Assigning a member requires task=<description>")?;
  let rows=value["members"].as_array_mut().ok_or("Invalid members")?;rows.retain(|m|m["pane"]!=pane);rows.push(json!({"pane":pane,"name":params["name"].as_str().unwrap_or(pane),"task":task}));
 }
 if let Some(allow)=params.get("allow"){value["allow"]=if let Some(text)=allow.as_str(){json!(text.split(',').filter(|s|!s.trim().is_empty()).map(|s|s.trim()).collect::<Vec<_>>())}else if allow.is_array(){allow.clone()}else{return Err("allow= must be command IDs separated by commas or a JSON array".into());};}
 let scope=Scope::from_value(file.clone(),&value)?;
 let temporary=file.with_extension(format!("{}.tmp",std::process::id()));
 use std::io::Write;use std::os::unix::fs::OpenOptionsExt;
 let mut output=std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temporary).map_err(|e|e.to_string())?;
 output.write_all(serde_json::to_string_pretty(&value).map_err(|e|e.to_string())?.as_bytes()).map_err(|e|e.to_string())?;drop(output);std::fs::rename(&temporary,&file).map_err(|e|e.to_string())?;
 Ok(json!({"assignment":file,"project":project,"members":scope.members,"allow":scope.allow,"launch":scope.launch(),"environment":{"MAGICSTUDIO_ASSIGNMENT":file},"onboarded":false,"note":"Configure the agent process with this environment; all mc CLI/MCP/TUI calls inherit project scope. No agent was launched or messaged."}))
}

#[cfg(test)]
pub(crate) fn fixture()->Scope{
 let root=std::env::temp_dir().join(format!("mc-scope-fixture-{}-{}",std::process::id(),crate::worker::now_ms()));std::fs::create_dir_all(&root).unwrap();
 let project=root.join("Assigned.magic");std::fs::write(&project,r#"{"format":"magicstudio.project","assets":[],"workshop":{}}"#).unwrap();
 Scope::from_value(root.join("assignment.json"),&json!({"schema":"magicstudio.assignment/1","project":project,"members":[{"pane":"assigned-pane","name":"beaker","task":"Only this project"}],"allow":["project.save"]})).unwrap()
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]
 fn grants_bind_every_project_and_save_destination_before_execution(){
  let scope=fixture();let other=scope.project.with_file_name("Other.magic");std::fs::write(&other,"other content").unwrap();
  assert!(scope.params("project.save",json!({"path":other})).is_err());assert_eq!(std::fs::read_to_string(&other).unwrap(),"other content");
  assert!(scope.params("project.save",json!({"path":scope.project.with_file_name("New.magic")})).is_err());
  assert!(scope.params("center.status",json!({"project":other})).is_err());assert!(scope.params("center.status",json!({"_expected_project":other})).is_err());
  assert_eq!(scope.params("project.save",json!({})).unwrap()["path"],json!(scope.project));
  for command in ["project.catalog","project.new","app.quit","studio.sessions","agent.select"]{assert!(!scope.permits(command));}
 }
 #[test]
 fn discovery_and_roster_do_not_expose_global_commands_or_terminal_context(){
  let scope=fixture();let commands=scope.commands("").to_string();assert!(!commands.contains("project.catalog"));assert!(!commands.contains("app.quit"));assert!(commands.contains("project.save"));
  let rows=scope.roster(vec![json!({"pane_id":"assigned-pane","name":"wrong title","terminal_title_stripped":"OTHER_PROJECT_SECRET","cwd":"/other/project","agent":"codex","agent_status":"working"}),json!({"pane_id":"outside-pane","name":"outsider"})]);
  assert_eq!(rows.len(),1);assert_eq!(rows[0]["terminal_title_stripped"],"Only this project");assert!(!json!(rows).to_string().contains("OTHER_PROJECT_SECRET"));assert!(!json!(rows).to_string().contains("outsider"));
 }
 #[test]
 fn foreign_worker_and_event_rows_are_removed(){
  let scope=fixture();let value=scope.redact(json!({"project":scope.project,"worker_owner":{"project":"/unrelated.magic","job":"SECRET_JOB"},"activity":[{"project":"/unrelated.magic","text":"SECRET"},{"project":scope.project,"command":"project.save"}],"catalog":{"projects":[{"name":"SECRET"}]}}));
  assert!(!value.to_string().contains("SECRET"));assert_eq!(value["activity"].as_array().unwrap().len(),1);assert_eq!(value["catalog"]["projects"].as_array().unwrap().len(),1);
 }
 #[test]
 fn scoped_cli_and_mcp_reject_master_commands_and_filter_discovery(){
  use std::io::Write;use std::process::{Command,Stdio};
  let scope=fixture();std::fs::write(&scope.file,serde_json::to_vec(&json!({"schema":"magicstudio.assignment/1","project":scope.project,"members":scope.members,"allow":["project.save"]})).unwrap()).unwrap();std::fs::set_permissions(&scope.file,std::fs::Permissions::from_mode(0o600)).unwrap();
  let current=std::env::current_exe().unwrap();let binary=current.parent().unwrap().parent().unwrap().join("mc");assert!(binary.is_file(),"build mc before the commit hooks");
  for args in [vec!["exec","app.quit"],vec!["exec","project.catalog"],vec!["workshop"]]{let out=Command::new(&binary).env(ENV,&scope.file).args(args).output().unwrap();assert!(!out.status.success());}
  let out=Command::new(&binary).env(ENV,&scope.file).args(["--bridge","auto","exec","command.list"]).output().unwrap();assert!(out.status.success(),"{}",String::from_utf8_lossy(&out.stderr));let listed:String=String::from_utf8(out.stdout).unwrap();assert!(!listed.contains("project.catalog"));assert!(listed.contains("project.save"));
  let mut child=Command::new(binary).env(ENV,&scope.file).arg("mcp").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
  let mut input=child.stdin.take().unwrap();writeln!(input,"{}",json!({"jsonrpc":"2.0","id":1,"method":"tools/list"})).unwrap();writeln!(input,"{}",json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"project_catalog","arguments":{}}})).unwrap();drop(input);
  let out=child.wait_with_output().unwrap();let rows:Vec<Value>=String::from_utf8(out.stdout).unwrap().lines().map(|line|serde_json::from_str(line).unwrap()).collect();assert_eq!(rows.len(),2);
  assert!(rows[0]["result"]["tools"].as_array().unwrap().iter().all(|t|t["name"]!="project_catalog"&&t["name"]!="app_quit"));assert!(rows[1]["error"].is_object());
 }
}
