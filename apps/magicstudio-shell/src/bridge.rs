//! `--control <port>`: the TCP JSON-lines bridge that `magic-cli --bridge <port>` and its
//! MCP server use to drive this window. One request per line, `{"id": "...", "params": {...}}`;
//! one reply per line, `{"ok": true, "id", "result"}` or `{"ok": false, "id", "error"}`.
//!
//! `ui.*` and `app.quit` act on the window (the page answers through `ui/bridge.js`); every other
//! id is an engine command run on the window's own session, after which the page refreshes.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::sync::Mutex;
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

use crate::Engine;

/// Replies the page owes the bridge, by token.
#[derive(Default)]
pub struct Pending(Mutex<(u64, HashMap<u64, Sender<Value>>)>);

/// The page's answer to one `eval`.
#[tauri::command]
pub fn bridge_reply(pending: tauri::State<Pending>, token: u64, value: Value) {
    if let Some(tx) = pending.0.lock().ok().and_then(|mut p| p.1.remove(&token)) {
        let _ = tx.send(value);
    }
}

/// Runs `body` (the inside of an async function) in the project view and returns what it returns.
pub fn eval(app: &AppHandle, body: &str) -> Result<Value, String> {
    let view = app.get_webview("project").ok_or("no project view")?;
    let (tx, rx) = channel();
    let token = {
        let pending = app.state::<Pending>();
        let mut p = pending.0.lock().map_err(|e| e.to_string())?;
        p.0 += 1;
        let token = p.0;
        p.1.insert(token, tx);
        token
    };
    let reply = |v: &str| format!("window.__TAURI__.core.invoke(\"bridge_reply\", {{token: {token}, value: {v}}})");
    view.eval(format!(
        "(async () => {{ {body} }})().then(v => {}, e => {})",
        reply("{ok: true, value: v ?? null}"),
        reply("{ok: false, error: String((e && e.stack) || e)}")
    ))
    .map_err(|e| e.to_string())?;
    let v = rx.recv_timeout(Duration::from_secs(60)).map_err(|_| "the page did not answer".to_string())?;
    if v["ok"].as_bool() == Some(true) { Ok(v["value"].clone()) } else { Err(v["error"].as_str().unwrap_or("page error").to_string()) }
}

/// `magicBridge.<name>(args…)` in the page.
fn page(app: &AppHandle, name: &str, args: &[&Value]) -> Result<Value, String> {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    eval(app, &format!("return await magicBridge.{name}({});", args.join(", ")))
}

/// A png path for a picture: `output`, else `<project>/.agent/looks/` (projects stay self-contained).
fn output(app: &AppHandle, params: &Value, name: &str) -> Result<std::path::PathBuf, String> {
    if let Some(out) = params["output"].as_str() {
        return Ok(out.into());
    }
    let project = app.state::<Engine>().with(None, |s| Ok(s.project_path.clone()))?;
    let dir = project.as_deref().and_then(std::path::Path::parent).ok_or("no project open: pass output=<png>")?.join(".agent").join("looks");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    Ok(dir.join(format!("{name}-{stamp}.png")))
}

pub(crate) fn handle(app: &AppHandle, id: &str, params: &Value) -> Result<Value, String> {
    let none = Value::Null;
    let ui_project_request = crate::project_dispatch::is_ui_request(app,id,params);
    if let Some(expected)=params["_expected_project"].as_str(){
        let expected=magicstudio_engine::studio_sessions::canonical(std::path::Path::new(expected))?;
        // Read-only observation must not wait for the operation it is observing.
        let actual=if id=="ui.feedback"{crate::feedback::snapshot(app)?["active_project"].as_str().map(std::path::PathBuf::from)}else{app.state::<Engine>().with(None,|s|Ok(s.project_path.clone()))?};
        if actual.as_deref()!=Some(expected.as_path()){return Err("Pinned Studio changed project; request refused without switching".into());}
    }
    #[cfg(unix)]
    if let Some(expected)=params["_expected_instance"].as_str(){let socket=app.state::<LocalSocket>();if socket.0.file_name().and_then(|p|p.to_str())!=Some(expected){return Err("Studio instance changed; explicit reattach required".into());}}
    if params.get("_dry_run").and_then(Value::as_bool) == Some(true)
        && !magicstudio_engine::commands::is_read_only_command(id, params)
    {
        return Ok(json!({
            "ok": true,
            "dry_run": true,
            "route": "bridge",
            "command": id,
            "params": params,
        }));
    }
    if id=="project.open"{if let Some(result)=crate::feedback::reuse_open(app,params)?{return Ok(result);}}

    if matches!(id,"project.open"|"project.new"|"project.close"|"project.intake"|"app.quit"){
        eval(app,"await acFeedback.report(); return true;")?;
    }
    match id {
        "ui.eval" => eval(app, params["js"].as_str().ok_or("ui.eval needs js")?),
        "ui.feedback" => crate::feedback::snapshot(app),
        "ui.focus"=>{let window=app.get_window("main").ok_or("Studio window unavailable")?;window.set_focus().map_err(|e|e.to_string())?;Ok(json!({"ok":true,"focused":true,"reused_session":true}))},
        "ui.controls" => page(app,"controls", &[params]),
        "ui.attention" => page(app,"attention", &[params]),
        "ui.components" => page(app,"components", &[params]),
        "ui.unconfigured" => page(app,"unconfigured", &[]),
        "ui.state" => page(app, "state", &[]),
        "ui.elements" => page(app, "elements", &[params.get("region").unwrap_or(&none)]),
        "ui.click" => page(app, "click", &[params.get("id").or(params.get("target")).unwrap_or(&none), params.get("instance").unwrap_or(&none), params.get("files").unwrap_or(&none)]),
        "ui.drag" => page(app, "drag", &[params]),
        "ui.set" => page(app, "set", &[params.get("id").or(params.get("target")).unwrap_or(&none), params.get("value").unwrap_or(&none), params.get("instance").unwrap_or(&none)]),
        "ui.key" => page(app, "key", &[params.get("key").unwrap_or(&none), params.get("id").unwrap_or(&none), params.get("instance").unwrap_or(&none)]),
        "ui.screenshot" => {
            let out = output(app, params, "screenshot")?;
            crate::snapshot_to(app, None, &out)?;
            Ok(json!({"output": out}))
        }
        // One call: click (optional), wait for the page to settle, picture the region, report the
        // app state and the clickable ids inside it.
        "ui.look" => {
            if let Some(target) = params.get("click").filter(|v| !v.is_null()) {
                page(app, "click", &[target])?;
                std::thread::sleep(Duration::from_millis(params["settle_ms"].as_u64().unwrap_or(450)));
            }
            let region = params.get("region").unwrap_or(&none);
            let pad = params.get("pad").cloned().unwrap_or(json!(0));
            let rect: Option<[f64; 4]> = serde_json::from_value(page(app, "rect", &[region, &pad])?).map_err(|e| e.to_string())?;
            let out = output(app, params, "look")?;
            crate::snapshot_to(app, rect, &out)?;
            Ok(json!({"output": out, "state": page(app, "state", &[])?, "elements": page(app, "elements", &[region])?}))
        }
        "app.quit" => {
            crate::feedback::guard(app,id,false)?;
            magicstudio_engine::activity::append(json!({"stage":"close_requested","command":"app.quit","origin":params["_origin"],"client_label":params["_client"],"request_id":params["_request_id"],"project":crate::feedback::snapshot(app)?["active_project"]}));
            let close_app=app.clone();std::thread::spawn(move||{std::thread::sleep(Duration::from_millis(100));#[cfg(unix)] cleanup(&close_app);close_app.exit(0);});
            Ok(json!({"ok":true,"status":"close_requested","exited":false,"verification":"Check endpoint/process exit after this response; no force kill or silent discard."}))
        }
        _ => {
            if id == "project.save" {
                eval(app, "if (typeof saveStoryboard === 'function' && (projectView.textDrafts.size || state.data?.design?.dirty)) await saveStoryboard(); return true;")?;
            }
            crate::feedback::guard(app,id,ui_project_request)?;
            let mut supplied=params.clone();
            if supplied.is_object(){if supplied.get("_origin").is_none(){supplied["_origin"]=json!("CLI/MCP bridge");}}
            if supplied["_request_id"].is_null(){supplied["_request_id"]=json!(crate::feedback::new_request());}
            if id=="project.open"{supplied["claim_owner"]=json!(true);}
            let request_id=supplied["_request_id"].as_str().map(str::to_owned);
            supplied.as_object_mut().map(|p|p.remove("_ui_dispatch_token"));
            let mut result = app.state::<Engine>().with(None, |s| s.execute(id, supplied).map_err(|e| e.to_string()))?;
            if magicstudio_engine::find_command(id).is_some_and(|c|!c.journal){return Ok(result);}
            let revision=crate::feedback::command_finished(app,id,&result,request_id.as_deref());
            if result.is_object(){result["_ui_engine_revision"]=json!(revision);}
            // This command came directly from the UI. Its awaiting handler renders once;
            // refreshing here would call Open while that same transition is still in flight.
            if ui_project_request {return Ok(result);}
            // The page shows the change (and follows a project opened or created from outside).
            let project = app.state::<Engine>().with(None, |s| Ok(s.project_path.clone()))?;
            page(app, "refresh", &[&json!(id), &json!(project), &json!(revision), &result, &json!(request_id), params])?;
            Ok(result)
        }
    }
}

/// Listens on 127.0.0.1:`port`, one thread per connection.
pub fn serve(app: AppHandle, port: u16) -> Result<(),String> {
    let listener=std::net::TcpListener::bind(("127.0.0.1",port)).map_err(|e|format!("Studio port {port}: {e}"))?;
    *app.state::<crate::ControlPort>().0.lock().map_err(|e|e.to_string())?=Some(listener.local_addr().map_err(|e|e.to_string())?.port());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let app = app.clone();
            std::thread::spawn(move || {
                let Ok(mut writer) = stream.try_clone() else { return };
                for line in BufReader::new(stream).lines().map_while(Result::ok) {
                    if line.trim().is_empty() {
                        continue;
                    }
                    let reply = match serde_json::from_str::<Value>(line.trim()) {
                        Err(e) => json!({"ok": false, "error": e.to_string()}),
                        Ok(req) => {
                            let id = req["id"].as_str().unwrap_or("").to_string();
                            let params = req.get("params").cloned().unwrap_or_else(|| json!({}));
                            match handle(&app, &id, &params) {
                                Ok(result) => json!({"ok": true, "id": id, "result": result}),
                                Err(error) => json!({"ok": false, "id": id, "error": error}),
                            }
                        }
                    };
                    if writeln!(writer, "{reply}").is_err() {
                        break;
                    }
                }
            });
        }
    });Ok(())
}

/// Same-user window IPC. Files are private; discovery does not require visibility into host PIDs.
#[cfg(unix)]
pub fn serve_local(app:AppHandle)->Result<(),String>{
 use std::os::unix::{net::UnixListener,fs::{PermissionsExt,DirBuilderExt}};
 let runtime=std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR is missing; private UI feedback unavailable")?;
 let dir=std::path::PathBuf::from(&runtime).join("magicstudio-studio");
 magicstudio_engine::local_ipc::private_path(std::path::Path::new(&runtime),true)?;
 if !dir.exists(){std::fs::DirBuilder::new().mode(0o700).create(&dir).map_err(|e|e.to_string())?;}
 magicstudio_engine::local_ipc::private_path(&dir,true)?;
 let nonce=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_nanos();
 let socket=dir.join(format!("{}-{nonce}.sock",std::process::id()));let listener=UnixListener::bind(&socket).map_err(|e|e.to_string())?;
 app.manage(LocalSocket(socket.clone()));
 std::fs::set_permissions(&socket,std::fs::Permissions::from_mode(0o600)).map_err(|e|e.to_string())?;
 std::thread::spawn(move||{for stream in listener.incoming().flatten(){if magicstudio_engine::local_ipc::peer(&stream).is_err(){continue;}let app=app.clone();std::thread::spawn(move||{
  let _=stream.set_read_timeout(Some(Duration::from_secs(10)));let Ok(mut writer)=stream.try_clone() else{return;};
  for line in BufReader::new(stream).lines().map_while(Result::ok){
   let reply=match serde_json::from_str::<Value>(&line){Ok(v)=>{let id=v["id"].as_str().unwrap_or_default();let p=v.get("params").cloned().unwrap_or(json!({}));match handle(&app,id,&p){Ok(result)=>json!({"ok":true,"result":result}),Err(e)=>json!({"ok":false,"error":e})}},Err(e)=>json!({"ok":false,"error":e.to_string()})};
   if writeln!(writer,"{reply}").is_err(){break;}
  }
 });}});Ok(())
}

#[cfg(unix)]
pub struct LocalSocket(pub std::path::PathBuf);
#[cfg(unix)]
pub fn cleanup(app:&AppHandle){if let Some(socket)=app.try_state::<LocalSocket>(){if magicstudio_engine::local_ipc::private_path(&socket.0,false).is_ok(){let _=std::fs::remove_file(&socket.0);}}}
