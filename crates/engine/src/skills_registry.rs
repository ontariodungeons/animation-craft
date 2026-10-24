//! Learning uses project-local Markdown, with the old global registry retained as read-only input.
//! UI and CLI use these same guarded commands. Imported references are never executed.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs::{self, File, OpenOptions}, io::Write, net::{IpAddr, ToSocketAddrs}, path::{Path, PathBuf}, process::Command};

fn invalid(message: impl Into<String>) -> EngineError { EngineError::InvalidParams(message.into()) }
fn id(p: &Value) -> Result<&str> {
    let id=p["id"].as_str().unwrap_or("");
    if id.is_empty() || id.len()>96 || id.contains("..") || !id.bytes().all(|b|b.is_ascii_alphanumeric()||b"-_.".contains(&b)) {return Err(invalid("Learning id must be 1–96 letters, digits, - _ or .; no path components"));}
    Ok(id)
}
fn root(s: &Session) -> PathBuf {
    s.project_path.as_ref().and_then(|p|p.parent()).map(Path::to_path_buf)
        .unwrap_or_else(||crate::workspace::workspace_root(s)).join("learning")
}
fn legacy_file() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
        .or_else(||std::env::var_os("HOME").map(|p|PathBuf::from(p).join(".config")))
        .map(|p|p.join("magicstudio/skills.json"))
}
fn read_json(p: &Path) -> Result<Value> {
    match fs::read(p) {
        Ok(b)=>serde_json::from_slice::<Value>(&b).ok().filter(Value::is_object).ok_or_else(||invalid(format!("Invalid {}; preserved for explicit repair",p.display()))),
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(json!({})), Err(e)=>Err(e.into())
    }
}
fn seeds() -> Value {
    let mut v=json!({});
    for (id, md) in TASKS {
        let (_, meta)=frontmatter(md);let mut record=meta.unwrap_or_else(||json!({}));
        record["id"]=json!(id);record["name"]=json!(title(md).unwrap_or(id));
        v[*id]=record;
    }
    v["ffmpeg-skill"]=json!({"id":"ffmpeg-skill","name":"ffmpeg-skill","category":"reference","status":"researching","source":"https://github.com/kajisho5/ffmpeg-skill"});
    v
}
const TASKS: &[(&str,&str)] = &[
    ("resolve-project",include_str!("../manual/resolve-project.md")),
    ("choose-audio",include_str!("../manual/choose-audio.md")),
    ("timing-skill",include_str!("../manual/timing-skill.md")),
    ("import-timing",include_str!("../manual/import-timing.md")),
    ("native-storyboard",include_str!("../manual/native-storyboard.md")),
    ("prepare-art",include_str!("../manual/prepare-art.md")),
    ("author-motion",include_str!("../manual/author-motion.md")),
    ("review-release",include_str!("../manual/review-release.md")),
    ("report-note",include_str!("../manual/report-note.md")),
    ("approve-alternative",include_str!("../manual/approve-alternative.md")),
    ("fix-source",include_str!("../manual/fix-source.md")),
    ("recheck-fix",include_str!("../manual/recheck-fix.md")),
    ("full-review",include_str!("../manual/full-review.md")),
    ("delayed-type",include_str!("../manual/delayed-type.md")),
    ("matte-window",include_str!("../manual/matte-window.md")),
    ("polar-tunnel",include_str!("../manual/polar-tunnel.md")),
];
fn records(s: &Session) -> Result<Value> {
    let mut all=seeds();
    if let Some(p)=legacy_file() {for (k,v) in read_json(&p)?.as_object().unwrap(){all[k]=v.clone();}}
    for (k,v) in read_json(&root(s).join("index.json"))?.as_object().unwrap(){all[k]=v.clone();}
    match fs::read_dir(root(s)) {
        Ok(entries)=>for entry in entries {
            let path=entry?.path();if path.extension().and_then(|v|v.to_str())!=Some("md"){continue;}
            let Some(ident)=path.file_stem().and_then(|v|v.to_str()) else {continue;};
            if id(&json!({"id":ident})).is_err(){continue;}
            let md=fs::read_to_string(&path)?;let (_,meta)=frontmatter(&md);
            if let Some(meta)=meta {let mut record=all.get(ident).cloned().unwrap_or_else(||json!({}));for (k,v) in meta.as_object().unwrap(){record[k]=v.clone();}record["name"]=json!(title(&md).unwrap_or(ident));all[ident]=record;}
        },
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{},Err(e)=>return Err(e.into())
    }
    Ok(all)
}
fn builtin(id: &str) -> Option<&'static str> {TASKS.iter().find(|(k,_)|*k==id).map(|(_,v)|*v)}
fn markdown(s: &Session, record: &Value, ident: &str) -> Result<String> {
    if id(&json!({"id":ident})).is_ok() {
        match fs::read_to_string(root(s).join(format!("{ident}.md"))) {
            Ok(v)=>return Ok(v),Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{},Err(e)=>return Err(e.into())
        }
    }
    if let Some(v)=builtin(ident) {return Ok(v.into());}
    Ok(format!("# {}\n\n{}\n\n{}\n\nSource: {}\n",record["name"].as_str().unwrap_or(ident),record["description"].as_str().unwrap_or(""),record["integration_notes"].as_str().unwrap_or(""),record["source"].as_str().unwrap_or("Not recorded")))
}
fn revision(record: &Value, markdown: &str) -> String {
    let mut hash=Sha256::new();hash.update(record.to_string());hash.update([0]);hash.update(markdown);format!("{:x}",hash.finalize())
}
fn rendered(s: &Session, ident: &str, record: &Value) -> Result<Value> {
    let md=markdown(s,record,ident)?;let mut v=record.clone();
    v["id"]=json!(ident);v["revision"]=json!(revision(record,&md));v["markdown"]=json!(&md);
    v["block"]=parse_task(&md);v["limits"]=limits(s)?;v["validation"]=json!(validate_task(&v["block"],&v["limits"]));
    v["document_path"]=json!(root(s).join(format!("{ident}.md")));v["removed"]=json!(record["removed"].as_bool().unwrap_or(false));
    Ok(v)
}
fn read_lock(s: &Session) -> Result<Option<File>> {
    match File::open(root(s).join("index.lock")) {Ok(f)=>{f.lock_shared()?;Ok(Some(f))},Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(None),Err(e)=>Err(e.into())}
}
pub fn cmd_skills_list(s: &mut Session, _: &Value) -> Result<Value> {
    let _lock=read_lock(s)?;let all=records(s)?;let mut out=vec![];
    for (id,v) in all.as_object().unwrap(){if v["removed"]!=true {out.push(rendered(s,id,v)?);}}
    out.sort_by_key(|v|(v["id"]!="resolve-project",v["name"].as_str().unwrap_or("").to_lowercase()));
    Ok(json!({"ok":true,"count":out.len(),"skills":out,"directory":root(s)}))
}
pub fn cmd_skills_info(s: &mut Session, p: &Value) -> Result<Value> {
    let ident=id(p)?;let _lock=read_lock(s)?;let all=records(s)?;
    let record=all.get(ident).ok_or_else(||invalid(format!("Learning block '{ident}' not found")))?;
    Ok(json!({"ok":true,"skill":rendered(s,ident,record)?}))
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp=path.with_extension(format!("{}.{}.tmp",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()));
    let result=(||{let mut f=OpenOptions::new().write(true).create_new(true).open(&tmp)?;f.write_all(bytes)?;f.sync_all()?;fs::rename(&tmp,path)})();// same-directory rename
    if result.is_err(){let _=fs::remove_file(&tmp);}result.map_err(Into::into)
}
fn title(md: &str) -> Option<&str> {
    let mut lines=md.trim_start_matches('\u{feff}').lines().peekable();
    if lines.peek().is_some_and(|v|*v=="---") {lines.next();for l in lines.by_ref(){if l=="---"{break;}}}
    lines.find_map(|l|l.strip_prefix("# ").map(str::trim).filter(|s|!s.is_empty()))
}
fn mutate(s: &mut Session,p: &Value,action: &str) -> Result<Value> {
    let ident=id(p)?.to_string();let dir=root(s);
    if p["_expected_directory"].as_str().is_some_and(|v|Path::new(v)!=dir){return Err(invalid("Learning project changed; draft retained"));}fs::create_dir_all(&dir)?;
    let lock=OpenOptions::new().create(true).truncate(false).read(true).write(true).open(dir.join("index.lock"))?;lock.lock()?;
    let all=records(s)?;let previous=all.get(&ident);let mut record=previous.cloned().unwrap_or_else(||json!({"id":ident,"status":"researching","verification":"draft"}));
    let old_md=markdown(s,&record,&ident)?;
    // Managed writes require the revision from info/list; legacy metadata-only adds remain compatible.
    let needs_guard=previous.is_some()&&(action!="add"||dir.join(format!("{ident}.md")).exists());
    if needs_guard && p["expected_revision"].as_str()!=Some(revision(&record,&old_md).as_str()) {return Err(invalid("Learning conflict: content changed or expected_revision missing; read skills.info and compare before retrying"));}
    let mut md=old_md;
    match action {
        "remove"=>{record["removed"]=json!(true);},
        "restore"=>{if record["removed"]!=true{return Err(invalid("Only a removed Learning block can be restored"));}record["removed"]=json!(false);},
        _=>{
            for (k,v) in p.as_object().ok_or_else(||invalid("Expected object"))? {
                if !k.starts_with('_') && !["id","markdown","expected_revision"].contains(&k.as_str()) {record[k]=v.clone();}
            }
            if let Some(content)=p["markdown"].as_str() {if content.len()>512*1024{return Err(invalid("Learning Markdown exceeds 512 KiB"));}md=content.into();}
            else if previous.is_none(){md=format!("# {}\n\n{}\n",p["name"].as_str().unwrap_or(&ident),p["description"].as_str().unwrap_or(""));}
            if let Some(t)=title(&md){record["name"]=json!(t);}else if record["name"].as_str().is_none(){record["name"]=json!(ident);}
            record["removed"]=json!(false);
        }
    }
    let (_, header)=frontmatter(&md);
    let structured=record["kind"]=="task" || header.as_ref().is_some_and(|v|v["kind"]=="task");
    if let Some(meta)=header {for (k,v) in meta.as_object().unwrap(){if !["removed","name"].contains(&k.as_str()) && !p.as_object().unwrap().contains_key(k){record[k]=v.clone();}}}
    if structured {
        record["kind"]=json!("task");
        if action!="remove" && action!="restore" {
            let errors=validate_task(&parse_task(&md),&limits(s)?);if !errors.is_empty(){return Err(invalid(errors.join("; ")));}
        }
        let (body, _)=frontmatter(&md);let mut metadata=record.clone();
        for k in ["name","description","markdown","revision","document_path","block","limits","validation"]{metadata.as_object_mut().unwrap().remove(k);}
        md=format!("---\n{}\n---\n{}",serde_json::to_string_pretty(&metadata).map_err(|e|invalid(e.to_string()))?,body);
        atomic_write(&dir.join(format!("{ident}.md")),md.as_bytes())?;
        return Ok(json!({"ok":true,"skill":rendered(s,&ident,&record)?}));
    }
    let mut index=read_json(&dir.join("index.json"))?;index[&ident]=record.clone();
    // Refuse corrupt index before changing the Markdown file.
    atomic_write(&dir.join(format!("{ident}.md")),md.as_bytes())?;
    atomic_write(&dir.join("index.json"),&serde_json::to_vec_pretty(&index).map_err(|e|invalid(e.to_string()))?)?;
    Ok(json!({"ok":true,"skill":rendered(s,&ident,&record)?}))
}
pub fn cmd_skills_add(s:&mut Session,p:&Value)->Result<Value>{mutate(s,p,"add")}
pub fn cmd_skills_edit(s:&mut Session,p:&Value)->Result<Value>{mutate(s,p,"edit")}
pub fn cmd_skills_remove(s:&mut Session,p:&Value)->Result<Value>{mutate(s,p,"remove")}
pub fn cmd_skills_undo(s:&mut Session,p:&Value)->Result<Value>{mutate(s,p,"restore")}

fn public(ip:IpAddr)->bool {match ip {IpAddr::V4(v)=>!v.is_private()&&!v.is_loopback()&&!v.is_link_local()&&!v.is_unspecified()&&!v.is_multicast()&&v.octets()[0]!=0&&v.octets()[0]<224&&!(v.octets()[0]==100&&(64..128).contains(&v.octets()[1])),IpAddr::V6(v)=>v.to_ipv4_mapped().map(|a|public(IpAddr::V4(a))).unwrap_or_else(||v.segments()[0]&0xe000==0x2000)}}
fn fetch_source(source:&str)->Result<String>{
    if source.starts_with("https://") {
        let authority=source.trim_start_matches("https://").split('/').next().unwrap_or("");
        if authority.is_empty()||authority.bytes().any(|b|!b.is_ascii_alphanumeric()&&!b".-".contains(&b)) {return Err(invalid("Import needs an https URL with a public DNS hostname; no credentials or custom port"));}
        let ips:Vec<_>=(authority,443).to_socket_addrs().map_err(|e|invalid(format!("Source lookup failed: {e}")))?.map(|v|v.ip()).collect();
        if ips.is_empty()||ips.iter().any(|v|!public(*v)){return Err(invalid("Source must resolve only to public IPv4 addresses"));}
        let chosen=ips.iter().find(|v|v.is_ipv4()).unwrap_or(&ips[0]);let pin=format!("{authority}:443:{}",if chosen.is_ipv6(){format!("[{chosen}]")}else{chosen.to_string()});
        // No redirects: each new destination needs its own source review, avoiding private-host redirects.
        let out=Command::new("curl").args(["--fail","--silent","--show-error","--max-time","15","--max-filesize","262144","--proto","=https","--location","--max-redirs","0","--resolve",&pin,source]).output()?;
        if !out.status.success(){return Err(invalid(format!("Source fetch failed: {}",String::from_utf8_lossy(&out.stderr).chars().take(240).collect::<String>())));}
        if out.stdout.len()>262144{return Err(invalid("Source exceeds 256 KiB"));}
        return String::from_utf8(out.stdout).map_err(|_|invalid("Source must be UTF-8 text"));
    }
    if source.contains("://"){return Err(invalid("Only reviewed https URLs or existing local skill/package paths are imported"));}
    let p=Path::new(source);let p=if p.is_dir(){[p.join("SKILL.md"),p.join("README.md")].into_iter().find(|p|p.is_file()).ok_or_else(||invalid("Local package has no SKILL.md or README.md"))?}else{p.into()};
    if fs::metadata(&p)?.len()>262144{return Err(invalid("Source exceeds 256 KiB"));}Ok(fs::read_to_string(p)?)
}
pub fn cmd_skills_import(s:&mut Session,p:&Value)->Result<Value>{
    id(p)?;let source=p["source"].as_str().ok_or_else(||invalid("Missing source URL or existing package path"))?;
    let excerpt=fetch_source(source)?;
    let name=p["name"].as_str().or_else(||title(&excerpt)).unwrap_or("Imported reference");
    let mut params=p.clone();params["name"]=json!(name);params["status"]=json!("researching");params["verification"]=json!("draft");
    params["kind"]=json!("task");params["category"]=p.get("category").cloned().unwrap_or(json!("reference"));
    params["markdown"]=json!(format!("# {name}\n\n## Input\nReference source below.\n\n## Source\n{source}\n\n## Output\nDraft knowledge block. Outcome and prerequisites are not yet verified.\n\n## Check\nReview the source and replace missing details before application.\n\n## Fallback\nAsk for missing facts; no package installation or source command execution occurs.\n"));
    mutate(s,&params,"add")
}

// JSON is valid YAML frontmatter. Unknown fields remain part of the same task file.
fn frontmatter(md:&str)->(&str,Option<Value>){
    let plain=md.trim_start_matches('\u{feff}');
    if let Some(rest)=plain.strip_prefix("---\n") {if let Some(end)=rest.find("\n---\n") {let meta=serde_json::from_str::<Value>(&rest[..end]).ok().filter(Value::is_object);return (&rest[end+5..],meta);}}
    (md,None)
}
fn parse_task(md:&str)->Value {
    let (body,_)=frontmatter(md);let mut parts=std::collections::BTreeMap::<String,String>::new();let mut key="notes".to_string();
    for line in body.lines(){if let Some(h)=line.strip_prefix("## "){key=h.trim().to_lowercase();parts.entry(key.clone()).or_default();}else{let value=parts.entry(key.clone()).or_default();value.push_str(line);value.push('\n');}}
    let clean=|k:&str|parts.get(k).map(|v|v.trim()).unwrap_or("").to_string();
    let mut block=json!({"title":title(md).unwrap_or(""),"input":clean("input"),"output":clean("output"),"check":clean("check"),"condition":clean("condition"),"fallback":clean("fallback")});
    let notes=parts.iter().filter(|(k,_)|!["input","output","check","condition","fallback","command","skill","source"].contains(&k.as_str())).map(|(_,v)|v.lines().filter(|l|!l.starts_with("# ")).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join(" ");block["notes"]=json!(notes);
    for kind in ["command","skill","source"]{let mut value=clean(kind);if !value.is_empty(){if value.starts_with("```"){value=value.lines().skip(1).take_while(|l|!l.starts_with("```")).collect::<Vec<_>>().join("\n");}block["action"]=json!({"kind":kind,"ref":value});break;}}
    let words=["title","input","output","check","condition","fallback","notes"].iter().map(|k|block[*k].as_str().unwrap_or("").split_whitespace().count()).sum::<usize>();
    let steps=body.lines().filter(|l|{let l=l.trim_start();let n=l.bytes().take_while(u8::is_ascii_digit).count();n>0&&l[n..].starts_with(". ")}).count();
    block["word_count"]=json!(words);block["step_count"]=json!(steps);block
}
fn validate_task(block:&Value,policy:&Value)->Vec<String>{
    let mut errors=vec![];
    for (key,label) in [("title","title"),("input","Input"),("output","Output"),("check","observable Check")]{if block[key].as_str().unwrap_or("").is_empty(){errors.push(format!("A task needs {label}"));}}
    if block["action"]["ref"].as_str().unwrap_or("").is_empty(){errors.push("A task needs an exact Command, Skill or Source".into());}
    if block["action"]["ref"].as_str().is_some_and(|v|v.chars().count()>2048){errors.push("Action reference exceeds 2048 characters".into());}
    for (actual,limit,label) in [("word_count","max_words","word"),("step_count","max_steps","step")] {if let Some(max)=policy[limit].as_u64(){if block[actual].as_u64().unwrap_or(0)>max{errors.push(format!("Task exceeds {max} {label} limit; shorten explicitly. Nothing was truncated"));}}}
    errors
}
fn limits(s:&Session)->Result<Value>{let v=read_json(&root(s).join("limits.json"))?;if v["max_words"].is_u64()&&v["max_steps"].is_u64(){Ok(v)}else{Ok(json!({"confirmed":false,"max_words":null,"max_steps":null}))}}
pub fn cmd_skills_limits(s:&mut Session,p:&Value)->Result<Value>{
    if p.get("max_words").is_some()||p.get("max_steps").is_some(){
        let words=p["max_words"].as_u64().filter(|v|(1..=1000).contains(v)).ok_or_else(||invalid("Set max_words explicitly between 1 and 1000"))?;
        let steps=p["max_steps"].as_u64().filter(|v|(1..=100).contains(v)).ok_or_else(||invalid("Set max_steps explicitly between 1 and 100"))?;
        let dir=root(s);fs::create_dir_all(&dir)?;atomic_write(&dir.join("limits.json"),serde_json::to_string_pretty(&json!({"max_words":words,"max_steps":steps,"confirmed":true})).unwrap().as_bytes())?;
    }
    Ok(json!({"ok":true,"limits":limits(s)?}))
}

/// Both Archify views are projections of the same task IDs, titles and `next` relations.
pub fn cmd_skills_diagram(s:&mut Session,p:&Value)->Result<Value>{
    let kind=p["kind"].as_str().unwrap_or("production");
    let ids:Vec<&str>=match kind {"production"=>vec!["resolve-project","choose-audio","timing-skill","import-timing","native-storyboard","prepare-art","author-motion","review-release"],"correction"=>vec!["report-note","approve-alternative","fix-source","recheck-fix","full-review"],_=>return Err(invalid("Diagram kind is production or correction"))};
    let _lock=read_lock(s)?;let all=records(s)?;let mut nodes=vec![];let mut edges=vec![];let mut present=vec![];
    for (i,ident) in ids.iter().enumerate(){
        let Some(record)=all.get(*ident).filter(|v|v["removed"]!=true) else {continue;};
        let md=markdown(s,record,ident)?;let block=parse_task(&md);
        let col=0usize;let lane="tasks";let y_offset=(i as f64-(ids.len()-1) as f64/2.0)*80.0;
        nodes.push(json!({"id":ident,"lane":lane,"col":col,"yOffset":y_offset,"type":if record["blocked"].is_string(){"security"}else{"backend"},"label":block["title"],"sublabel":block["action"]["kind"],"width":180usize.max(block["title"].as_str().unwrap_or("").chars().count()*8+32)}));present.push(*ident);
    }
    for ident in &present {if let Some(next)=all[*ident]["next"].as_array(){for target in next.iter().filter_map(Value::as_str).filter(|n|present.contains(n)){
        let mut edge=json!({"id":format!("{ident}-to-{target}"),"from":ident,"to":target,"variant":"default"});
        if target=="report-note"{edge["variant"]=json!("dashed");edge["role"]=json!("branch");edge["route"]=json!("outside-right");edge["fromSide"]=json!("right");edge["toSide"]=json!("right");}
        else {edge["fromSide"]=json!("bottom");edge["toSide"]=json!("top");}
        edges.push(edge);
    }}}
    let lanes=vec![json!({"id":"tasks","label":if kind=="production"{"Production path — manual steps"}else{"Approved correction loop"}})];
    let output=format!("{kind}.html");let spec=json!({"schema_version":2,"diagram_type":"workflow","meta":{"title":if kind=="production"{"Music-video production tasks"}else{"Approved correction loop"},"subtitle":"Editable task records drive this manual view; no automatic execution or approval is implied.","quality_profile":"showcase","animation":"none","output":output},"lanes":lanes,"mainPath":present,"nodes":nodes,"edges":edges});
    if p["render"]!=true{return Ok(json!({"ok":true,"spec":spec,"editable_format":"Archify workflow schema v2 JSON; task Markdown is the instruction source"}));}
    let dir=root(s);if p["_expected_directory"].as_str().is_some_and(|v|Path::new(v)!=dir){return Err(invalid("Learning project changed; diagram request refused"));}fs::create_dir_all(&dir)?;
    let script=std::env::var_os("ARCHIFY_SCRIPT").map(PathBuf::from).or_else(||std::env::var_os("HOME").map(|h|PathBuf::from(h).join(".codex/skills/archify/bin/archify.mjs"))).ok_or_else(||invalid("Installed Archify script not configured"))?;
    if !script.is_file(){return Err(invalid("Archify is unavailable. Set ARCHIFY_SCRIPT to the existing installed bin/archify.mjs; nothing was installed"));}
    let candidate=dir.join(format!("{kind}.workflow.json"));let bytes=serde_json::to_vec_pretty(&spec).map_err(|e|invalid(e.to_string()))?;atomic_write(&candidate,&bytes)?;
    let evidence=dir.join(".archify-evidence").join(kind).join(format!("{:x}",Sha256::digest(&bytes)));
    let result=Command::new("node").arg(&script).args(["finalize","workflow"]).arg(candidate.file_name().unwrap()).arg(&output).args(["--quality","showcase","--json","--out-dir"]).arg(&evidence).current_dir(&dir).output()?;
    if !result.status.success(){return Err(invalid(format!("Archify finalize failed; candidate retained at {}. {} {}",candidate.display(),String::from_utf8_lossy(&result.stdout).chars().take(1600).collect::<String>(),String::from_utf8_lossy(&result.stderr).chars().take(400).collect::<String>())));}
    let html=fs::read_to_string(dir.join(&output))?;
    Ok(json!({"ok":true,"spec":spec,"source":candidate,"output":dir.join(output),"html":html,"verified":"Archify finalize exit 0; visual review remains separate"}))
}
