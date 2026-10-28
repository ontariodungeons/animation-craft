//! Receipt for the existing audio.analyze chain; no transcription or tool replacement here.
use crate::{EngineError, Result};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::{Path,PathBuf}};

pub(crate) struct AudioTrace { path:PathBuf, pub data:Value }
fn digest(path:&Path)->Option<String>{fs::read(path).ok().map(|b|format!("{:x}",Sha256::digest(b)))}
fn resolve(tool:&str)->Option<PathBuf>{
    std::env::var_os("PATH").and_then(|p|std::env::split_paths(&p).map(|d|d.join(tool)).find(|p|p.is_file()))
}
fn summary(stdout:&str)->Value{
    let parsed=stdout.lines().rev().find_map(|line|serde_json::from_str::<Value>(line).ok());
    let mut selected=json!({});
    if let Some(v)=parsed {for k in ["ok","engine","model","detectedLanguage","wordCount","durationSeconds","speechOnsetSeconds","transcriptPath","outputPath","format","reason","error","skipped"] {
        if let Some(value)=v.get(k){selected[k]=value.clone();}
    }}selected
}
impl AudioTrace {
    pub fn new(project:&Path,audio:&Path,p:&Value)->Result<Self>{
        let mut trace=Self{path:project.join("transcript-provenance.json"),data:json!({
            "status":"running","operation":"audio.analyze","audio_source":audio,
            "request_id":p["_request_id"],"client_label":p["_client"],
            "manual":"magic-cli commands audio.analyze; magic-cli commands workshop.import_transcript",
            "steps":[],"producer_status":"unknown until an actual transcription result is recorded"})};
        trace.data["transcript_path"]=json!(project.join("hyperframes/transcript.json"));trace.persist()?;Ok(trace)
    }
    fn persist(&mut self)->Result<()> {
        let mut v=fs::read(&self.path).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).filter(Value::is_object).unwrap_or(json!({}));
        let transcript=self.data["transcript_path"].as_str().map(Path::new);
        let hash=transcript.and_then(digest);
        self.data["transcript_sha256"]=json!(hash);
        let asr=self.data["steps"].as_array().and_then(|rows|rows.iter().find(|r|r["stage"]=="transcription"&&r["status"]=="success"));
        if let Some(step)=asr {
            v["producer"]=json!({"tool":"HyperFrames transcribe","transcript_sha256":hash,
                "engine":step["result"]["engine"],"model":step["result"]["model"],
                "version_probe":step["version_probe"],"resolved_executable":step["resolved_executable"],
                "argv":step["argv"],"result":step["result"],"source":"actual subprocess stdout; missing values remain null"});
            self.data["producer_status"]=json!("actual transcription result recorded");
        }else if hash.is_some()&&v["producer"]["transcript_sha256"]==json!(hash)&&v["producer"]["tool"].is_string(){
            self.data["producer_status"]=json!("matching previously recorded producer retained; no transcription this run");
        }else{
            v["producer"]=json!({"engine":null,"model":null,"tool":null,"transcript_sha256":hash,"source":"unknown; existing file has no matching producer receipt"});
            self.data["producer_status"]=json!(if hash.is_some(){"unknown original producer; existing outputs reused without ASR"}else{"unknown; no transcript file exists yet"});
        }
        v["analysis_attempt"]=self.data.clone();
        fs::write(&self.path,serde_json::to_vec_pretty(&v).map_err(|e|EngineError::InvalidParams(e.to_string()))?)
            .map_err(|e|EngineError::InvalidParams(format!("Cannot save audio receipt {}: {e}",self.path.display())))?;Ok(())
    }
    pub fn run(&mut self,stage:&str,needed:bool,cwd:&Path,tool:&str,args:&[String],outputs:&[PathBuf])->Result<Option<String>>{
        let program=resolve(tool);
        let mut row=json!({"stage":stage,"requested_executable":tool,"resolved_executable":program,
            "real_executable":program.as_ref().and_then(|p|p.canonicalize().ok()),"argv":args,"cwd":cwd,
            "outputs":outputs,"status":if needed{"running"}else{"skipped_existing"}});
        if needed {
            let version_args:Vec<String>=match tool {
                "ffmpeg"=>vec!["-version".into()],
                "npx"=>vec!["--offline".into(),"--no-install".into(),"hyperframes".into(),"--version".into()],
                _=>vec!["--version".into()],
            };
            row["version_probe"]=match crate::sheets::run_agent(tool,&version_args,cwd,5){
                Ok(run)=>json!({"argv":version_args,"exit_code":run.code,"timed_out":run.timed_out,
                    "output":if run.succeeded(){run.stdout.lines().next().map(str::to_owned)}else{None}}),
                Err(e)=>json!({"argv":version_args,"error":e.to_string(),"output":null}),
            };
            if tool=="python3" {row["script"]=args.first().map(|p|json!({"path":p,"sha256":digest(Path::new(p))})).unwrap_or(Value::Null);}
        }
        let index=self.data["steps"].as_array().map(Vec::len).unwrap_or(0);
        self.data["steps"].as_array_mut().ok_or_else(||EngineError::InvalidParams("Invalid audio receipt steps".into()))?.push(row);
        self.persist()?;if !needed{return Ok(None);}
        match crate::workspace::run_tool_in(cwd,tool,args){
            Ok(output)=>{self.data["steps"][index]["status"]=json!("success");self.data["steps"][index]["result"]=summary(&output);self.persist()?;Ok(Some(output))},
            Err(error)=>{self.data["steps"][index]["status"]=json!("error");self.data["steps"][index]["error"]=json!(error.to_string());self.data["failed_stage"]=json!(stage);self.data["status"]=json!("failed");self.persist()?;
                Err(EngineError::InvalidParams(format!("audio.analyze stage={stage}; executable={tool}; cwd={}; {error}; receipt={}. Inspect this receipt and `magic-cli commands audio.analyze` before retrying; no alternate provider or automatic retry was used.",cwd.display(),self.path.display())))},
        }
    }
    pub fn read_json(&mut self,stage:&str,path:&Path)->Result<Value>{
        let result=fs::read(path).map_err(EngineError::from).and_then(|bytes|serde_json::from_slice::<Value>(&bytes).map_err(|e|EngineError::InvalidParams(e.to_string())));
        if let Err(error)=&result{self.data["status"]=json!("failed");self.data["failed_stage"]=json!(stage);self.data["error"]=json!({"path":path,"error":error.to_string()});self.persist()?;}
        result.map_err(|e|EngineError::InvalidParams(format!("audio.analyze stage={stage}; file={}; {e}; receipt={}",path.display(),self.path.display())))
    }
    pub fn import(&mut self,s:&mut crate::Session,p:Value)->Result<Value>{
        let index=self.data["steps"].as_array().map(Vec::len).unwrap_or(0);
        self.data["steps"].as_array_mut().ok_or_else(||EngineError::InvalidParams("Invalid audio receipt steps".into()))?.push(json!({"stage":"import","command":"workshop.import_transcript","params":p,"status":"running","expected_format":"native word array [{text,start,end,id?}], or object with words; optional original SRT"}));self.persist()?;
        let result=s.execute("workshop.import_transcript",p);
        self.data["steps"][index]["status"]=json!(if result.is_ok(){"success"}else{"error"});
        self.data["status"]=json!(if result.is_ok(){"complete"}else{"failed"});
        if let Err(e)=&result{self.data["steps"][index]["error"]=json!(e.to_string());self.data["failed_stage"]=json!("import");}
        self.persist()?;
        result.map_err(|e|EngineError::InvalidParams(format!("audio.analyze stage=import; {e}; receipt={}. Read `magic-cli commands workshop.import_transcript`; received files remain preserved and no ASR retry was attempted.",self.path.display())))
    }
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn producer_summary_uses_only_actual_output_and_keeps_unknowns(){
        let known=summary("progress\n{\"ok\":true,\"engine\":\"synthetic-engine\",\"model\":\"synthetic-model\",\"words\":[\"not copied\"]}");
        assert_eq!(known["engine"],"synthetic-engine");assert!(known.get("words").is_none());
        assert!(summary("not a producer receipt").get("engine").is_none());
    }
}
