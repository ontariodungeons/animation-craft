//! `--control <port>`: the TCP JSON-lines bridge that `animationcraft-cli --bridge <port>` and its
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

/// `acBridge.<name>(args…)` in the page.
fn page(app: &AppHandle, name: &str, args: &[&Value]) -> Result<Value, String> {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    eval(app, &format!("return await acBridge.{name}({});", args.join(", ")))
}

/// A png path for a picture: `output`, or one in the scratch folder.
fn output(params: &Value, name: &str) -> std::path::PathBuf {
    params["output"].as_str().map(std::path::PathBuf::from).unwrap_or_else(|| {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        std::env::temp_dir().join(format!("animationcraft-{name}-{stamp}.png"))
    })
}

fn handle(app: &AppHandle, id: &str, params: &Value) -> Result<Value, String> {
    let none = Value::Null;
    match id {
        "ui.eval" => eval(app, params["js"].as_str().ok_or("ui.eval needs js")?),
        "ui.state" => page(app, "state", &[]),
        "ui.elements" => page(app, "elements", &[params.get("region").unwrap_or(&none)]),
        "ui.click" => page(app, "click", &[params.get("id").or(params.get("target")).unwrap_or(&none)]),
        "ui.set" => page(app, "set", &[params.get("id").or(params.get("target")).unwrap_or(&none), params.get("value").unwrap_or(&none)]),
        "ui.key" => page(app, "key", &[params.get("key").unwrap_or(&none), params.get("id").unwrap_or(&none)]),
        "ui.screenshot" => {
            let out = output(params, "screenshot");
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
            let out = output(params, "look");
            crate::snapshot_to(app, rect, &out)?;
            Ok(json!({"output": out, "state": page(app, "state", &[])?, "elements": page(app, "elements", &[region])?}))
        }
        "app.quit" => {
            app.exit(0);
            Ok(json!({"ok": true}))
        }
        _ => {
            let result = app.state::<Engine>().with(None, |s| s.execute(id, params.clone()).map_err(|e| e.to_string()))?;
            // The page shows the change (and follows a project opened or created from outside).
            let project = app.state::<Engine>().with(None, |s| Ok(s.project_path.clone()))?;
            page(app, "refresh", &[&json!(id), &json!(project)])?;
            Ok(result)
        }
    }
}

/// Listens on 127.0.0.1:`port`, one thread per connection.
pub fn serve(app: AppHandle, port: u16) {
    std::thread::spawn(move || {
        let listener = match std::net::TcpListener::bind(("127.0.0.1", port)) {
            Ok(l) => l,
            Err(e) => return eprintln!("--control {port}: {e}"),
        };
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
    });
}
