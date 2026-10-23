//! Magic Studio CLI: every engine command is one shell call away, for scripts and AI agents.
//!
//! Run `mc help` for the full reference.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unimplemented,
        clippy::todo,
        clippy::unreachable
    )
)]

mod args;
mod center;
mod local_ui;
mod snapshot;

use animationcraft_engine::Session;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};

use std::io::Write as _;

static STDOUT_GONE: AtomicBool = AtomicBool::new(false);
static STDOUT_ERROR: AtomicBool = AtomicBool::new(false);

fn stdout_gone() -> bool {
    STDOUT_GONE.load(Ordering::Relaxed)
}

fn stdout_failed(e: std::io::Error) {
    if STDOUT_GONE.swap(true, Ordering::Relaxed) || e.kind() == std::io::ErrorKind::BrokenPipe {
        return;
    }
    diag(format_args!(
        "mc: can't write to stdout: {e}"
    ));
    STDOUT_ERROR.store(true, Ordering::Relaxed);
}

macro_rules! outln {
    ($($arg:tt)*) => {{
        if !$crate::stdout_gone() {
            if let Err(e) = writeln!(std::io::stdout(), $($arg)*) {
                $crate::stdout_failed(e);
            }
        }
    }};
}

fn diag(msg: std::fmt::Arguments) {
    let _ = writeln!(&mut std::io::stderr(), "{msg}");
}

fn usage(msg: impl std::fmt::Display) -> ! {
    diag(format_args!(
        "mc: {msg}\nRun `mc help` for usage."
    ));
    std::process::exit(2)
}

fn fail(msg: impl std::fmt::Display) -> ! {
    diag(format_args!("mc: {msg}"));
    std::process::exit(1)
}

const HELP: &str = "\
mc: drive Magic Studio from the shell (headless engine or live GUI bridge)

USAGE
  mc [--bridge <port|host:port>] <SUBCOMMAND> [ARGS]

OPTIONS
  --bridge <port|host:port|auto|unix:path>  send commands to a running `animationcraft-shell --control <port>` GUI window

SUBCOMMANDS
  center [project=<file>]             interactive Command Center (no automatic Studio launch)
                                      optional root= name= audio= transcript= captions= audiomap=
                                      --print prints current state without entering terminal UI
  exec ui.feedback                    discover canonical Studio identity, readiness, unsaved and ack state
  exec ui.components region=#view-project limit=30
                                      bounded actual DOM geometry/style/state; no input values
  exec center.events after=0 limit=20  bounded activity journal; use next_cursor for later reads
  exec center.snapshot [project=<file>]
                                      one JSON object with stable keys: project, subject, clip,
                                      playhead_sec, selected_part, unsaved_edits, hyperframes, fps,
                                      last_error (null = not observed); the Command Center status line
  exec center.tail [last=20] [project=<file>]
                                      last N typed events: saved, generation_started/finished,
                                      keyframe_added/removed (Theatre), motion_path_drawn/undone,
                                      selection_changed, clip_switched, clip_sheet_*, error
                                      (activity history: last 512, UI clicks never evict them)
  login [provider]                    native OpenCode authentication in your terminal (no --bridge)
  info                                print engine info and available commands
  commands [filter]                   list commands (id, label, shortcut, params)
  doctor [--clean] [keep_port=N]      check lingering Craft processes, clean stale servers, & flag Ponytail code smells
  storyboard --doctor                read-only section-boundary task list for the selected Studio project
                                      exit 0: no supported issue; 1: findings; 2: insufficient evidence
  storyboard edit scene <N> <text> [field=action]  edit an existing native scene text field and save
  exec <id> [key=value ...]           run one command; prints its result as JSON
  <project> <subcommand | command id> [ARGS]
                                      run against one project: an .acproj, or a folder with exactly
                                      one (a song's hyperframes/ or remotion/ folder finds its song);
                                      same as project=<file> _expected_project=<file>, e.g.
                                      mc \"Bumble Bee.acproj\" center.snapshot
  clips [character=<id>] [project=<file>] [--convert]
                                      each clip's Theatre sheet, duration and conversion status
                                      (motion.clips; --convert runs motion.clips.convert first)
  vision [image_path | character]     direct pixel/alpha/drift/lift vision analysis of a PNG or a character's artwork
  run <script.jsonl | ->              run commands, one {\"id\",\"params\"} per line
  mcp                                 MCP server on stdio (headless engine, or the open window with --bridge);
                                      register it with your agent as: command `mc`, args [\"mcp\"]
  help                                this text
  --version                           print the version

  agent say \"<text>\" [wait=300] [project=<dir>]
                                      talk to the app-selected chat agent (fresh by default; prints its reply);
                                      also: agent read [last=10], agent session (not an MCP tool)

LIVE UI CONTROL COMMANDS (run with `--bridge auto exec <command>`, or the Studio's unix:<socket>)
  ui.state                            project, view, animation (subject, track, time, selected part)
  ui.elements [region=<id or CSS>]    visible clickable elements with ids, labels and boxes
  ui.click id=<element id>            click an element by its ui.elements id (or aria label)
  ui.set id=<element id> value=<v>    set an input or select (e.g. id=motion-track)
  ui.key key=<key> [id=<element id>]  press a key, optionally on an element
  ui.screenshot [output=...]          save a PNG of the window
  ui.look [region=<id prefix>] [click=<id>] [pad=0] [max_side=1000] [output=...]
                                      one-step look: click (optional), then a PNG of the window or region,
                                      the app state and the ids inside it (MCP: ui_look returns the image)
  ui.focus                            bring the Studio window to the front
  app.quit                            request guarded close; unsaved/unknown state refuses
  app_quit (MCP tool)                 same guarded close, current Studio auto-discovery

VALUES
  key=value parses value as JSON when it can (3.5, true, [1,2], {\"a\":1}), else as a string.

AGENT GUIDE
  ~/.agents/skills/animationcraft/SKILL.md  workflow, tools and gotchas (the `animationcraft` skill)

EXIT STATUS
  0 success · 1 a command failed · 2 usage error
";

/// How long to wait for one bridge reply. An explicit `ANIMATIONCRAFT_BRIDGE_TIMEOUT_MS` wins;
/// otherwise commands the engine marks `long_running` (exports, renders, agent runs) get an hour
/// and everything else 30 s, so a slow render is not reported as a dead bridge.
fn bridge_timeout(id: &str) -> std::time::Duration {
    if let Some(ms) = std::env::var("ANIMATIONCRAFT_BRIDGE_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        return std::time::Duration::from_millis(ms.max(50));
    }
    let long = animationcraft_engine::find_command(id).is_some_and(|c| c.long_running);
    std::time::Duration::from_secs(if long { 3600 } else { 30 })
}

fn bridge_call(addr: &str, id: &str, params: Value) -> Result<Value, String> {
    if addr=="auto"||addr.starts_with("unix:"){return local_ui::call(addr,id,params);}
    use std::io::{BufRead, BufReader};
    use std::net::{TcpStream, ToSocketAddrs};

    let timeout = bridge_timeout(id);
    let sock_addr = addr
        .to_socket_addrs()
        .map_err(|e| format!("bridge resolve {addr}: {e}"))?
        .next()
        .ok_or_else(|| format!("bridge resolve {addr}: no socket address found"))?;
    let mut stream = TcpStream::connect_timeout(&sock_addr, timeout)
        .map_err(|e| format!("bridge connect {addr}: {e}"))?;
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));

    let payload = json!({ "id": id, "params": params }).to_string();
    writeln!(stream, "{payload}").map_err(|e| format!("bridge write: {e}"))?;
    stream.flush().map_err(|e| format!("bridge flush: {e}"))?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let n = reader
        .read_line(&mut line)
        .map_err(|e| format!("bridge read ({id} on {addr}): {e}"))?;
    if n == 0 {
        return Err(format!(
            "bridge closed connection without replying ({id} on {addr})"
        ));
    }
    let resp: Value = serde_json::from_str(line.trim()).map_err(|e| format!("bridge json: {e}"))?;
    if resp.get("ok").and_then(Value::as_bool) == Some(true) {
        Ok(resp.get("result").cloned().unwrap_or(json!({})))
    } else {
        let err = resp
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown bridge error");
        Err(err.to_string())
    }
}

fn is_payload_error(v: &Value) -> bool {
    v.get("ok").and_then(Value::as_bool) == Some(false)
}

// The shared positional parser skips dash-prefixed tokens, including stdin's
// literal `-`. Recover that marker here without changing the parser's owner.
fn run_script_argument<'a>(positional: Option<&'a str>, raw_args: &[String]) -> Option<&'a str> {
    positional.or_else(|| raw_args.iter().any(|arg| arg == "-").then_some("-"))
}

/// Typed MCP input schemas for commands whose parameters are fixed; the rest keep a bare object
/// and publish their parameter shape in the tool description.
fn tool_input_schema(id:&str)->Value{
    let mut schema=command_input_schema(id);
    if schema["properties"].is_object(){
        schema["properties"]["_client"]=json!({"type":"string","description":"Optional supplied agent/client label; not authentication"});
        schema["properties"]["_request_id"]=json!({"type":"string","description":"Optional request/correlation ID for action receipts"});
    }
    schema
}

fn command_input_schema(id: &str) -> Value {
    let asset = json!({ "type": "string", "description": "Loaded asset id; defaults to the active asset" });
    let source = json!({ "type": "string", "description": "Source image path; defaults to the asset's image" });
    let out = json!({ "type": "string", "description": "Output PNG path" });
    let unit = json!({ "type": "number", "minimum": 0, "maximum": 1 });
    let point = json!({ "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 });
    match id {
        "project.root" => json!({"type":"object","properties":{"root":{"type":"string","description":"Optional absolute production root to persist; omit to inspect. Intake root= overrides never change it."}},"additionalProperties":false}),
        "agent.config" | "agent.session" => json!({"type":"object","properties":{},"additionalProperties":false}),
        "agent.models" => json!({"type":"object","properties":{"agent":{"type":"string","enum":["opencode","codex","omp","claude"]}},"additionalProperties":false}),
        "agent.new_chat" => json!({"type":"object","properties":{"free_only":{"type":"boolean"},"image_task":{"type":"boolean","description":"Native restricted image-job tools; no chat self-invocation"}},"additionalProperties":false}),
        "agent.send" => json!({"type":"object","properties":{"message":{"type":"string"},"attachments":{"type":"array","items":{"type":"string"}},"new_chat":{"type":"boolean","default":true},"wait":{"type":"integer","minimum":0,"maximum":300},"free_only":{"type":"boolean"},"image_task":{"type":"boolean","description":"Native restricted image-job tools; no chat self-invocation"}},"required":["message"],"additionalProperties":false}),
        "workshop.build_storyboard" => json!({"type":"object","properties":{"template":{"type":"string","enum":["pdf","backlot"]},"source":{"type":"string","description":"Optional OpenMontage project folder containing scene plan, script and reference artwork"}},"additionalProperties":false}),
        "image.break_apart" => json!({
            "type": "object",
            "properties": {
                "asset": asset,
                "regions": {
                    "type": "object",
                    "minProperties": 1,
                    "description": "Part id -> polygon [[x, y], ...] (keeps what is inside), or {polygons: [polygon, ...], keep: \"inside\"|\"outside\"}",
                    "additionalProperties": {
                        "oneOf": [
                            { "type": "array", "items": point },
                            {
                                "type": "object",
                                "properties": {
                                    "polygons": { "type": "array", "items": { "type": "array", "items": point } },
                                    "keep": { "enum": ["inside", "outside"] }
                                }
                            }
                        ]
                    }
                },
                "out_dir": { "type": "string" }
            },
            "required": ["regions"]
        }),
        "image.puppet_warp" => json!({
            "type": "object",
            "properties": {
                "asset": asset,
                "source": source,
                "out": out,
                "pins": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "properties": { "src": point, "dst": point },
                        "required": ["src", "dst"]
                    }
                },
                "mode": { "enum": ["rigid", "normal", "distort"] },
                "density": { "enum": ["fewer", "normal", "more"] },
                "expansion": { "type": "number" }
            },
            "required": ["pins"]
        }),
        "image.color_to_alpha" => json!({
            "type": "object",
            "properties": {
                "asset": asset,
                "source": source,
                "out": out,
                "color": { "type": "string", "description": "Colour to remove, e.g. #FFFFFF" },
                "transparencyThreshold": unit,
                "opacityThreshold": unit
            }
        }),
        "image.content_aware_fill" => json!({
            "type": "object",
            "properties": {
                "asset": asset,
                "source": source,
                "out": out,
                "select": {
                    "type": "object",
                    "description": "PhotoCraft selection command run before the fill, e.g. select.rect",
                    "properties": { "id": { "type": "string" }, "params": { "type": "object" } },
                    "required": ["id"]
                },
                "contents": { "type": "string" },
                "color": { "type": "string" }
            },
            "required": ["select"]
        }),
        "validate.motion" => json!({
            "type": "object",
            "properties": {
                "video": { "type": "string", "description": "Rendered transparent clip (WebM)" },
                "frames": { "type": "integer", "minimum": 1, "description": "Expected frame count" },
                "fps": { "type": "number", "description": "Expected frame rate" },
                "loop": { "type": "boolean", "description": "Require the last frame to match the first" },
                "reference": { "type": "string", "description": "Image of the expected look, compared by colour and size" },
                "spec": { "type": "string", "description": "Layered motion spec to lint for aliasing" }
            },
            "required": ["video"]
        }),
        "export.layered_motion" => json!({
            "type": "object",
            "properties": {
                "spec": { "type": "string", "description": "Layered motion spec (JSON): layers, pivots and per-track motion" },
                "out_dir": { "type": "string", "description": "Output folder for the transparent WebM per track" },
                "tracks": { "description": "Track ids to build (array or comma-separated); default all" },
                "stills": { "type": "integer", "minimum": 0, "description": "Also render this many evenly spaced transparent stills per track" },
                "video": { "type": "boolean", "description": "false skips the WebM export (stills only)" },
                "gpu": { "type": "boolean", "description": "Render on the GPU (default true); false uses the CPU" },
                "crf": { "type": "integer", "minimum": 0, "maximum": 63, "description": "VP9 quality, lower is better (default 24)" },
                "resolution": { "enum": ["full", "half", "third", "quarter"], "description": "EffectCraft render resolution factor (default full)" },
                "quality": { "enum": ["best", "draft"], "description": "EffectCraft render quality (default best)" }
            },
            "required": ["spec"]
        }),
        _ => schema_from_description(id),
    }
}

/// Typed MCP schema from a command's `params` description: example values give the JSON type
/// (`false` -> boolean, `25` -> number), so MCP clients send the same types the engine checks.
fn schema_from_description(id: &str) -> Value {
    let described = animationcraft_engine::command_specs()
        .iter()
        .find(|s| s.id == id)
        .and_then(|s| serde_json::from_str::<Value>(s.params).ok());
    let mut properties = serde_json::Map::new();
    if let Some(Value::Object(fields)) = described {
        for (key, example) in fields {
            let prop = match example {
                Value::Bool(b) => json!({ "type": "boolean", "description": format!("default {b}") }),
                Value::Number(n) => json!({ "type": "number", "description": format!("default {n}") }),
                Value::String(hint) => json!({ "type": "string", "description": hint }),
                other => json!({ "description": other.to_string() }),
            };
            properties.insert(key, prop);
        }
    }
    json!({ "type": "object", "properties": properties, "additionalProperties": true })
}

/// Sent with `initialize` so a client knows where to start without reading the code.
const MCP_INSTRUCTIONS: &str = "Magic Studio turns still character artwork into animated characters and plans \
song videos (storyboard, transcript, video target). Every tool is an engine command (command_list shows ids and \
params; tool names are the dotted ids with underscores). Start with project_open or project_new, then \
workshop_status (song, transcript, storyboard, assets) or project_list_assets. Assets need a destination: \
project_add_asset {path, character: <name>} copies an outside image to assets/characters/<name>/parts/, \
{background: <name>} to assets/backgrounds/<name>/; a rig spec connects with kind: cutout_rig. Without --bridge \
these tools run a headless engine; to drive the user's open window start the server as \
`mc --bridge <port> mcp` (the app runs with `animationcraft-shell --control <port>`). Full guide: the \
animationcraft skill (~/.agents/skills/animationcraft/SKILL.md); CLI: `mc help`.";

fn run_mcp_stdio(bridge: Option<String>) {
    use std::io::BufRead;
    let stdin = std::io::stdin();
    let mut session = Session::user_session();
    let mut stop_after_reply = false;
    for line in stdin.lock().lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        // A message without an id is a notification (e.g. notifications/initialized): never answered.
        let Some(id) = req.get("id").cloned() else {
            continue;
        };
        let method = req.get("method").and_then(Value::as_str).unwrap_or("");
        let resp = match method {
            "initialize" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "mc",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "instructions": MCP_INSTRUCTIONS
                }
            }),
            "tools/list" => {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "tools": mcp_tools() }
                })
            }
            "tools/call" => {
                let params = req.get("params").cloned().unwrap_or(json!({}));
                let raw_name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let cmd_id = animationcraft_engine::command_specs()
                    .iter()
                    .find(|s| s.id == raw_name || s.id.replace('.', "_") == raw_name)
                    .map(|s| s.id)
                    .unwrap_or(raw_name.as_str());
                let mut args = params.get("arguments").cloned().unwrap_or(json!({}));args["_origin"]=json!("MCP");
                if let Some(v) = center_observation(&raw_name.replace('_', "."), &args, bridge.as_deref()) {
                    outln!("{}", json!({"jsonrpc": "2.0", "id": id, "result": {"content": [{"type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default()}]}}));
                    continue;
                }
                if raw_name == "ui_look" || raw_name == "ui.look" {
                    outln!("{}", json!({"jsonrpc": "2.0", "id": id, "result": ui_look(bridge.as_deref(), args)}));
                    continue;
                }
                let res = if raw_name == "app_quit" || raw_name == "app.quit" {
                    let out = call_app_quit(bridge.as_deref(), args);
                    if out.as_ref().is_ok_and(|v| !is_payload_error(v)) {
                        stop_after_reply = true;
                    }
                    out
                } else if cmd_id=="ui.feedback"||cmd_id=="ui.components" {bridge_call(bridge.as_deref().unwrap_or("auto"),cmd_id,args)
                } else if let Some(addr) = &bridge {
                    bridge_call(addr, cmd_id, args)
                } else {
                    dispatch(&mut session,cmd_id,args)
                };
                match res {
                    Ok(v) => {
                        let is_err = is_payload_error(&v);
                        let mut result_obj = json!({
                            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() }]
                        });
                        if is_err {
                            result_obj["isError"] = json!(true);
                        }
                        json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": result_obj
                        })
                    }
                    Err(e) => json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32000, "message": e }
                    }),
                }
            }
            "ping" => json!({"jsonrpc":"2.0","id":id,"result":{}}),
            "resources/list" => json!({"jsonrpc":"2.0","id":id,"result":{"resources":[]}}),
            "resources/templates/list" => json!({"jsonrpc":"2.0","id":id,"result":{"resourceTemplates":[]}}),
            "prompts/list" => json!({"jsonrpc":"2.0","id":id,"result":{"prompts":[]}}),
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code":-32601,"message":format!("Method not found: {method}")}
            }),
        };
        outln!("{}", resp);
        if stop_after_reply {
            break;
        }
    }
}

/// The MCP tool list: every engine command plus the live-window lifecycle tool.
fn mcp_tools() -> Vec<Value> {
    let mut tools: Vec<Value> = animationcraft_engine::command_specs()
        .iter()
        .map(|spec| {
            json!({
                "name": spec.id.replace('.', "_"),
                "description": format!("{} ({})", spec.label, spec.params),
                "inputSchema": tool_input_schema(spec.id)
            })
        })
        .collect();
    tools.push(json!({
        "name": "ui_look",
        "description": "See the live Magic Studio window in one call: optionally click an element first, \
            then get a picture of the window (or of the elements whose id starts with `region`, e.g. \
            `workshop.storyboard`, `stage`, `header`) plus the app state and the clickable ids inside it. \
            Needs `mcp --bridge <port>`.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "region": {"type": "string", "description": "element id prefix to crop to; empty = whole window"},
                "click": {"type": "string", "description": "element id to click before looking"},
                "max_side": {"type": "number", "description": "longest side in pixels (default 1000)"},
                "pad": {"type": "number", "description": "points of surroundings to include around the region"}
            },
            "additionalProperties": false
        }
    }));
    tools.push(json!({
        "name": "center_snapshot",
        "description": "Current Magic Studio state as one JSON object with stable keys: project, subject, clip, \
            playhead_sec, selected_part, unsaved_edits, hyperframes, fps, last_error (null = not observed). \
            Read-only; uses the open Studio for `project` (or the only one open).",
        "inputSchema": {
            "type": "object",
            "properties": {"project": {"type": "string", "description": "canonical .acproj path; default the only open Studio"}},
            "additionalProperties": false
        }
    }));
    tools.push(json!({
        "name": "center_tail",
        "description": "Last N typed Magic Studio events (saved, generation_started, generation_finished, \
            keyframe_added, keyframe_removed (Theatre keyframes), motion_path_drawn, motion_path_undone, selection_changed, clip_switched, \
            clip_sheet_started, clip_sheet_converted, clip_sheet_failed, error) from the activity history, oldest first. Read-only.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "last": {"type": "integer", "minimum": 1, "maximum": 192, "description": "default 20"},
                "project": {"type": "string", "description": "canonical .acproj path; default the only open Studio"}
            },
            "additionalProperties": false
        }
    }));
    tools.push(json!({
        "name": "app_quit",
        "description": "Quit the live Magic Studio window this server is bridged to. \
            Refuses unsaved, pending or unknown state; reports close requested, not proof of exit. \
            Uses current Studio auto-discovery or the explicit --bridge target; never launches an app.",
        "inputSchema": {
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }
    }));
    tools
}

/// `agent say|read|session`: talk to the project's in-app opencode agent from outside the app.
/// Deliberately not an engine command, so it is not an MCP tool the agent itself could call.
fn agent_command(verb: &str, text: Option<&str>, params: &Value, bridge: Option<String>) -> Result<(), String> {
    let command = match verb { "say" => "agent.send", "read" => "agent.log", "session" => "agent.session", _ => return Err("usage: agent say \"<text>\" [project=<existing project>] | agent read | agent session".into()) };
    let mut request = params.clone();
    if verb == "say" { request["message"] = json!(text.ok_or("agent say needs message text")?); }
    let result = if let Some(addr) = bridge {
        if params.get("project").is_some() { return Err("Use an explicitly pinned Studio bridge or a headless project path; no automatic project switch".into()); }
        bridge_call(&addr, command, request)?
    } else {
        let mut session = Session::user_session();
        let target = params["project"].as_str().map(std::path::PathBuf::from)
            .unwrap_or(std::env::current_dir().map_err(|e|e.to_string())?);
        let project = if target.extension().is_some_and(|e|e=="acproj") { target } else {
            let files: Vec<_> = std::fs::read_dir(&target).map_err(|_|"Project directory unavailable; pass project=<existing .acproj>")?
                .filter_map(Result::ok).map(|e|e.path()).filter(|p|p.extension().is_some_and(|e|e=="acproj")).collect();
            if files.len()!=1 { return Err("Pass project=<existing .acproj>; no unique project found and no bootstrap project was created".into()); }
            files[0].clone()
        };
        session.execute("project.attach", json!({"path":project})).map_err(|e|e.to_string())?;
        request["_origin"]=json!("CLI agent");
        dispatch(&mut session,command,request)?
    };
    if verb=="say" {
        if let Some(text)=result["text"].as_str(){outln!("{}",text);}else{outln!("{}",result);}
    }
    else if verb=="session" { outln!("{}",result["session"].as_str().ok_or("No recorded provider session; inspect agent.config and start a fresh chat")?); }
    else { outln!("{}",result); }
    if result["ok"]==false { return Err(format!("Agent needs attention in session {}; inspect agent read project=<same .acproj> before retrying",result["session"])); }
    Ok(())
}

/// `ui_look` as an MCP result: the picture as an image block, the state as text.
fn ui_look(bridge: Option<&str>, mut args: Value) -> Value {
    let error = |e: String| json!({"content": [{"type": "text", "text": e}], "isError": true});
    let Some(addr) = bridge else {
        return error("ui_look needs the native window: start `animationcraft-shell --control <port>` and run this MCP server with `--bridge <port>`".into());
    };
    let out = std::env::temp_dir().join(format!("animationcraft-look-{}.png", std::process::id()));
    args["output"] = json!(out);
    match bridge_call(addr, "ui.look", args).and_then(|v| std::fs::read(&out).map(|png| (v, png)).map_err(|e| e.to_string())) {
        Ok((state, png)) => json!({"content": [
            {"type": "image", "data": base64(&png), "mimeType": "image/png"},
            {"type": "text", "text": serde_json::to_string_pretty(&state).unwrap_or_default()}
        ]}),
        Err(e) => error(e),
    }
}

/// Standard base64 (RFC 4648, padded).
fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            s.push(if i <= chunk.len() { ABC[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    s
}

/// Quit the live window behind `bridge`, or explain why a headless MCP session cannot.
pub(crate) fn dispatch(session:&mut Session,id:&str,mut params:Value)->Result<Value,String>{
 if id=="image.generate" && params["project"].is_string(){
  let target=params["project"].as_str().ok_or("Missing image project")?;
  let identity=session.execute("project.identity",json!({"path":target})).map_err(|e|e.to_string())?;
  let project=identity["project"].as_str().ok_or("No canonical image project")?;
  if session.project_path.as_deref().is_some_and(|p|p!=std::path::Path::new(project)){return Err("Image target differs from this session's project; attach it explicitly in a separate session".into());}
  if session.project_path.is_none(){session.execute("project.attach",json!({"path":project})).map_err(|e|e.to_string())?;}
 }

 let requested=if id=="project.open" || (id=="project.intake" && params["path"].as_str().is_some_and(|p|std::path::Path::new(p).is_file())){Some(session.execute("project.identity",params.clone()).map_err(|e|e.to_string())?["project"].as_str().ok_or("No canonical project")?.to_string())}else if id=="project.intake"{None}else{session.project_path.as_ref().map(|p|p.to_string_lossy().into_owned())};
 if let Some(project)=requested{
  if let Some(live)=animationcraft_engine::studio_sessions::for_project(std::path::Path::new(&project))?{
   if let Some(pin)=params["_expected_instance"].as_str(){if Some(pin)!=live["instance"].as_str(){return Err("Pinned Studio instance changed; explicitly reload this project attachment before further commands".into());}}
   params["_expected_project"]=json!(project);params["_expected_instance"]=live["instance"].clone();
   if live["project_guard_version"]!=1 && animationcraft_engine::find_command(id).is_some_and(|c|!c.journal){let mut value=session.execute(id,params).map_err(|e|e.to_string())?;if value.is_object(){value["state_source"]=json!("disk snapshot; matched Studio requires current guarded-attachment build");}return Ok(value);}
   let endpoint=live["endpoint"].as_str().ok_or("Matched Studio has no endpoint")?;
   let value=local_ui::call(endpoint,id,params.clone())?;
   if id=="project.open"{session.execute("project.attach",json!({"path":project})).map_err(|e|e.to_string())?;}
   return Ok(value);
  }
 }
 if params["_expected_instance"].is_string(){return Err("Pinned Studio is no longer available for this project. Explicit reload is required; no fallback writer was used.".into());}
 session.execute(id,params).map_err(|e|e.to_string())
}

/// `center.snapshot` / `center.tail` are mc observations, not engine commands; None for any other id.
fn center_observation(id: &str, params: &Value, bridge: Option<&str>) -> Option<Value> {
    let project = params["project"].as_str();
    match id {
        "center.snapshot" => Some(snapshot::collect(project, bridge).0),
        "center.tail" => {
            let last = params["last"].as_u64().unwrap_or(20).clamp(1, 192) as usize;
            let (state, activity) = snapshot::collect(project, bridge);
            let events = snapshot::tail(&activity, state["project"].as_str());
            let events = &events[events.len().saturating_sub(last)..];
            Some(json!({"schema": snapshot::TAIL_SCHEMA, "project": state["project"], "events": events,
                "next_cursor": activity["next_cursor"], "error": activity["error"]}))
        }
        _ => None,
    }
}

fn call_app_quit(bridge: Option<&str>, mut args: Value) -> Result<Value, String> {
    args["_origin"]=json!("MCP");
    bridge_call(bridge.unwrap_or("auto"), "app.quit", args)
}

/// A short spelling of the existing storyboard.edit operation; dash-only text is literal.
fn storyboard_edit_params(raw:&[String])->Result<Value,String>{
 let mut args=Vec::new();let mut i=0;
 while i<raw.len(){if raw[i]=="--bridge"{i+=2;continue;}args.push(raw[i].as_str());i+=1;}
 if args.len()<5 || args[..3]!=["storyboard","edit","scene"] {return Err("storyboard edit scene <N> <text> [field=action]".into());}
 let scene=args[3].parse::<u64>().ok().filter(|n|*n>0).ok_or("scene must be a positive number")?;
 let field=match args.get(5){None=>"action",Some(value)=>value.strip_prefix("field=").filter(|s|!s.is_empty()).ok_or("expected field=<native field from storyboard.read>")?};
 if args.len()>6{return Err("unexpected arguments after field".into());}
 Ok(json!({"scene":scene,"field":field,"value":args[4],"_origin":"CLI"}))
}

const SUBCOMMANDS: &[&str] = &["help", "-h", "version", "login", "center", "info", "commands", "vision", "storyboard", "exec", "run", "mcp", "agent", "doctor", "clips"];

/// The command line with a leading project path resolved (see `project_argument`), read once.
fn cli_args() -> &'static Vec<String> {
    static ARGS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    ARGS.get_or_init(|| project_argument(std::env::args().collect()).unwrap_or_else(|e| usage(e)))
}

/// `mc <project> <subcommand | command id> ...`: a leading project path becomes
/// `project=<file> _expected_project=<file>` for this call, and a bare command id after it
/// runs as `exec <id>`. Anything else is returned unchanged.
fn project_argument(mut raw: Vec<String>) -> Result<Vec<String>, String> {
    let mut i = 1;
    while i < raw.len() && raw[i].starts_with('-') && !SUBCOMMANDS.contains(&raw[i].as_str()) {
        i += if raw[i] == "--bridge" { 2 } else { 1 };
    }
    let Some(token) = raw.get(i).cloned() else { return Ok(raw) };
    if SUBCOMMANDS.contains(&token.as_str()) || !(token.contains('/') || token.ends_with(".acproj")) {
        return Ok(raw);
    }
    let project = resolve_project(std::path::Path::new(&token))?;
    raw.remove(i);
    if raw.get(i).is_some_and(|next| !SUBCOMMANDS.contains(&next.as_str()) && next.contains('.')) {
        raw.insert(i, "exec".into());
    }
    raw.push(format!("project={}", project.display()));
    raw.push(format!("_expected_project={}", project.display()));
    Ok(raw)
}

/// An .acproj file, or the one .acproj in a folder. A song project's video-target folder
/// (hyperframes/, remotion/, filmcraft/, or one level inside it) resolves to that song project.
fn resolve_project(path: &std::path::Path) -> Result<std::path::PathBuf, String> {
    if path.is_file() {
        if path.extension().is_some_and(|e| e == "acproj") {
            return std::fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()));
        }
        return Err(format!("{} is not an .acproj project file", path.display()));
    }
    if !path.is_dir() {
        return Err(format!("{}: no such project file or folder", path.display()));
    }
    for dir in path.ancestors().take(3) {
        // Above the given folder, only a song project's video-target folder leads up to it.
        let via_target = path.strip_prefix(dir).ok().and_then(|rest| rest.components().next())
            .and_then(|c| c.as_os_str().to_str()).is_some_and(|c| animationcraft_engine::workshop::VIDEO_TARGETS.contains(&c));
        if dir != path && !via_target {
            continue;
        }
        let found: Vec<_> = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?
            .filter_map(Result::ok).map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "acproj")).collect();
        match found.as_slice() {
            [] => continue,
            [one] => return std::fs::canonicalize(one).map_err(|e| e.to_string()),
            many => return Err(format!("{} has {} .acproj files; pass the one you mean", dir.display(), many.len())),
        }
    }
    Err(format!("no .acproj in {} (or above it, from a hyperframes/remotion/filmcraft folder); pass the project file", path.display()))
}

fn main() {
    let a = args::Args::parse(cli_args().iter().cloned().skip(1));
    if a.help {
        outln!("{HELP}");
        return;
    }
    if a.version {
        outln!("mc {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let Some(cmd) = a.pos(0) else {
        usage("missing subcommand")
    };
    match cmd {
        "help" | "-h" => outln!("{HELP}"),
        "version" => outln!("mc {}", env!("CARGO_PKG_VERSION")),
        "login" => {
            use std::io::IsTerminal;
            if a.bridge_addr().is_some() || a.pos(2).is_some() {
                usage("login [provider] runs locally without --bridge");
            }
            if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal()
                || !std::io::stderr().is_terminal()
            {
                fail("Login requires your interactive terminal");
            }
            let program = animationcraft_engine::agent::opencode_program().unwrap_or_else(|e| fail(e));
            diag(format_args!("Using OpenCode: {}", program.display()));
            let mut command = std::process::Command::new(program);
            command.args(["auth", "login"]);
            if let Some(provider) = a.pos(1) { command.arg(provider); }
            let status = command.status().unwrap_or_else(|_| fail("Cannot start installed OpenCode"));
            std::process::exit(status.code().unwrap_or(1));
        }
        "center" => {
            if a.bridge_addr().is_some(){usage("Command Center currently uses an explicit headless project; --bridge is reserved for existing live command access");}
            if let Err(e)=center::start(a.params_from(1).unwrap_or_else(|e|usage(e)),cli_args().iter().cloned().any(|a|a=="--print")){fail(e);}
        }
        "info" => {
            let specs = animationcraft_engine::command_specs();
            outln!("Magic Studio engine v{}", env!("CARGO_PKG_VERSION"));
            outln!("Registered commands: {}", specs.len());
            for spec in specs {
                outln!("  {} — {}", spec.id, spec.label);
            }
        }
        "commands" => {
            let f = a.pos(1).map(str::to_ascii_lowercase).unwrap_or_default();
            let specs = animationcraft_engine::command_specs();
            let hit = |spec: &&animationcraft_engine::CommandSpec| {
                let id = spec.id.to_ascii_lowercase();
                let label = spec.label.to_ascii_lowercase();
                f.is_empty() || id.contains(&f) || label.contains(&f)
            };
            for spec in specs.iter().filter(hit) {
                outln!(
                    "{:<36} {:<24} {:<14} {}",
                    spec.id,
                    spec.label,
                    spec.shortcut.unwrap_or("-"),
                    spec.params
                );
            }
        }
        "vision" => {
            let target = a.pos(1);
            let res = match (a.bridge_addr(), target) {
                // Character metrics on a whole-window screenshot are meaningless; ask for a picture instead.
                (Some(_), None) => usage("vision needs an image or character; for a picture of the window use `exec ui.look` or `exec ui.screenshot`"),
                (bridge_opt, Some(t)) => {
                    let params = if std::path::Path::new(t).exists() {
                        json!({ "image": t })
                    } else {
                        json!({ "character": t })
                    };
                    if let Some(addr) = bridge_opt {
                        bridge_call(&addr, "vision.analyze", params)
                    } else {
                        let mut s = Session::user_session();
                        s.execute("vision.analyze", params).map_err(|e| e.to_string())
                    }
                }
                (None, None) => {
                    let mut s = Session::user_session();
                    s.execute("vision.analyze", json!({ "character": "hopper" }))
                        .map_err(|e| e.to_string())
                }
            };
            match res {
                Ok(v) => {
                    if is_payload_error(&v) {
                        fail(format!("vision: command returned ok:false: {v}"));
                    }
                    outln!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
                }
                Err(e) => fail(format!("vision: {e}")),
            }
        }
        "storyboard" => {
            if cli_args().iter().cloned().any(|v|v=="--doctor"){
                if a.pos(1).is_some_and(|v|!v.contains('=')){usage("storyboard --doctor (select the project with --bridge <port>)");}
                let result=bridge_call(a.bridge_addr().as_deref().unwrap_or("auto"),"storyboard.doctor",a.params_from(1).map(|mut p|{p["_origin"]=json!("CLI");p}).unwrap_or_else(|e|usage(e))).unwrap_or_else(|e|fail(e));
                outln!("{}",serde_json::to_string_pretty(&result).unwrap_or_default());
                std::process::exit(if result["healthy"]==true{0}else if result["healthy"]==false{1}else{2});
            }
            let params=storyboard_edit_params(&cli_args().iter().cloned().skip(1).collect::<Vec<_>>()).unwrap_or_else(|e|usage(e));
            let result=bridge_call(a.bridge_addr().as_deref().unwrap_or("auto"),"storyboard.edit",params).unwrap_or_else(|e|fail(e));
            if is_payload_error(&result){fail(format!("storyboard.edit: {result}"));}
            outln!("{}",serde_json::to_string_pretty(&result).unwrap_or_default());
        }
        "clips" => {
            let mut params = a.params_from(1).unwrap_or_else(|e| usage(e));
            params["_origin"]=json!("CLI");
            let bridge = a.bridge_addr();
            let mut s = Session::user_session();
            // Headless reads need the project attached; dispatch then follows a Studio that has it open.
            if bridge.is_none() {
                if let Some(project) = params["project"].as_str() {
                    s.execute("project.attach", json!({"path": project})).unwrap_or_else(|e| fail(format!("clips: {e}")));
                }
            }
            let mut run = |id: &str| match &bridge { Some(addr) => bridge_call(addr, id, params.clone()), None => dispatch(&mut s, id, params.clone()) };
            if cli_args().iter().cloned().any(|v| v == "--convert") {
                run("motion.clips.convert").unwrap_or_else(|e| fail(format!("motion.clips.convert: {e}")));
            }
            let v = run("motion.clips").unwrap_or_else(|e| fail(format!("motion.clips: {e}")));
            outln!("{:<30} {:<36} {:>8}  {:<10}  PARTS", "CLIP", "SHEET", "SECONDS", "STATUS");
            for c in v["clips"].as_array().into_iter().flatten() {
                let seconds = c["duration"].as_f64().map(|d| format!("{d:.2}")).unwrap_or_else(|| "-".into());
                let parts = c["keyframed_parts"].as_array().map(|a| {
                    let names: Vec<&str> = a.iter().filter_map(Value::as_str).collect();
                    if names.is_empty() { "-".to_string() } else { format!("{} ({})", names.len(), names.join(", ")) }
                }).unwrap_or_else(|| "-".into());
                outln!("{:<30} {:<36} {:>8}  {:<10}  {}", c["clip"].as_str().unwrap_or("?"), c["sheet"].as_str().unwrap_or("?"), seconds, c["status"].as_str().unwrap_or("?"), parts);
            }
            if let Some(legacy) = v["legacy_sheet"].as_str() {
                outln!("legacy per-character sheet kept: {legacy}");
            }
        }
        "exec" => {
            let id = a
                .pos(1)
                .unwrap_or_else(|| usage("exec <id> [key=value ...]"));
            let mut params = a.params_from(2).unwrap_or_else(|e| usage(e));
            params["_origin"]=json!("CLI");
            // Opening names its target with path=; pinning the current project would refuse the switch.
            if matches!(id, "project.open" | "project.identity") && params["path"].is_null() && params["project"].is_string() {
                params["path"] = params["project"].clone();
                params.as_object_mut().map(|p| { p.remove("project"); p.remove("_expected_project") });
            }
            if let Some(v) = center_observation(id, &params, a.bridge_addr().as_deref()) {
                outln!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
                return;
            }
            let res = if id=="ui.feedback"||id=="ui.components"||id=="app.quit" {bridge_call(a.bridge_addr().as_deref().unwrap_or("auto"),id,params)
            }else if let Some(addr) = a.bridge_addr() {
                bridge_call(&addr, id, params)
            } else {
                let mut s = Session::user_session();
                // A named project is attached first, so dispatch reaches the Studio that has it open.
                if let Some(project) = params["project"].as_str() {
                    s.execute("project.attach", json!({"path": project})).unwrap_or_else(|e| fail(format!("{id}: {e}")));
                }
                dispatch(&mut s,id,params)
            };
            match res {
                Ok(v) => {
                    if is_payload_error(&v) {
                        fail(format!("{id}: command returned ok:false: {v}"));
                    }
                    outln!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
                }
                Err(e) => fail(format!("{id}: {e}")),
            }
        }
        "run" => {
            let raw_args: Vec<String> = cli_args().iter().cloned().skip(1).collect();
            let script = run_script_argument(a.pos(1), &raw_args)
                .unwrap_or_else(|| usage("run <script.jsonl | ->"));
            let text = if script == "-" {
                std::io::read_to_string(std::io::stdin()).unwrap_or_else(|e| fail(e))
            } else {
                std::fs::read_to_string(script).unwrap_or_else(|e| fail(format!("{script}: {e}")))
            };
            let bridge = a.bridge_addr();
            let mut s = Session::user_session();
            let mut failed = 0;
            for (n, line) in text
                .lines()
                .enumerate()
                .filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
            {
                let v: Value = serde_json::from_str(line)
                    .unwrap_or_else(|e| usage(format!("line {}: {e}", n + 1)));
                let id = v["id"]
                    .as_str()
                    .unwrap_or_else(|| usage(format!("line {}: missing \"id\"", n + 1)));
                let mut params = v.get("params").cloned().unwrap_or(json!({}));
                params["_origin"]=json!("CLI JSONL");
                let res = if id=="ui.feedback"||id=="ui.components"||id=="app.quit" {bridge_call(bridge.as_deref().unwrap_or("auto"),id,params)
                }else if let Some(addr) = &bridge {
                    bridge_call(addr, id, params)
                } else {
                    dispatch(&mut s,id,params)
                };
                match res {
                    Ok(r) => {
                        let cmd_ok = !is_payload_error(&r);
                        if !cmd_ok {
                            outln!(
                                "{}",
                                json!({"line": n + 1, "id": id, "ok": false, "result": r})
                            );
                            failed += 1;
                            if !a.flag_keep_going {
                                fail(format!("{id}: command returned ok:false: {r}"));
                            }
                            continue;
                        }
                        outln!(
                            "{}",
                            json!({"line": n + 1, "id": id, "ok": true, "result": r})
                        );
                    }
                    Err(e) => {
                        outln!(
                            "{}",
                            json!({"line": n + 1, "id": id, "ok": false, "error": e})
                        );
                        failed += 1;
                        if !a.flag_keep_going {
                            std::process::exit(1);
                        }
                    }
                }
            }
            if failed > 0 {
                std::process::exit(1);
            }
        }
        "mcp" => {
            run_mcp_stdio(a.bridge_addr());
        }
        "agent" => {
            let params = a.params_from(3).unwrap_or_else(|e| usage(e));
            if let Err(e) = agent_command(a.pos(1).unwrap_or(""), a.pos(2), &params, a.bridge_addr()) {
                fail(e);
            }
        }
        "doctor" => {
            let mut params = a.params_from(1).unwrap_or_else(|e| usage(e));
            if a.flag_clean {
                if let Some(obj) = params.as_object_mut() {
                    obj.insert("clean".to_string(), Value::Bool(true));
                }
            }
            let mut s = Session::user_session();
            match s.execute("app.doctor", params) {
                Ok(v) => {
                    outln!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
                }
                Err(e) => fail(format!("doctor: {e}")),
            }
        }
        other => usage(format!("unknown subcommand `{other}`")),
    }
    if STDOUT_ERROR.load(Ordering::Relaxed) {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::args::parse_value;
    use super::*;

    #[test]
    fn parse_value_handles_types() {
        assert_eq!(parse_value("42"), Value::Number(42.into()));
        assert_eq!(
            parse_value("3.5"),
            Value::Number(serde_json::Number::from_f64(3.5).unwrap())
        );
        assert_eq!(parse_value("true"), Value::Bool(true));
        assert_eq!(parse_value("hello"), Value::String("hello".to_string()));
        assert_eq!(
            parse_value("{\"key\":\"val\"}"),
            serde_json::json!({"key": "val"})
        );
    }

    #[test]
    fn mcp_tools_lists_the_lifecycle_tool_once_with_no_arguments() {
        let tools = mcp_tools();
        let quit: Vec<&Value> = tools
            .iter()
            .filter(|t| t.get("name").and_then(Value::as_str) == Some("app_quit"))
            .collect();
        assert_eq!(quit.len(), 1, "app_quit must appear exactly once");
        assert_eq!(quit[0]["inputSchema"]["additionalProperties"], json!(false));
        assert!(tools.len() >= animationcraft_engine::command_specs().len());
    }

    #[test]
    fn base64_matches_rfc_4648() {
        for (raw, enc) in [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foobar", "Zm9vYmFy")] {
            assert_eq!(base64(raw.as_bytes()), enc);
        }
    }

    #[test]
    fn run_accepts_stdin_marker_after_options() {
        let raw = vec!["--keep-going".into(), "run".into(), "-".into()];
        assert_eq!(run_script_argument(None, &raw), Some("-"));
        assert_eq!(
            run_script_argument(Some("script.jsonl"), &[]),
            Some("script.jsonl")
        );
        assert_eq!(run_script_argument(None, &[]), None);
    }

    #[test]
    fn engine_has_registered_commands() {
        let specs = animationcraft_engine::command_specs();
        assert!(!specs.is_empty());
    }

    #[test]
    fn bridge_call_reports_eof_and_read_timeout_honestly() {
        use std::net::TcpListener;

        // 1. Server accepts and immediately drops connection -> EOF error
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            if let Ok((stream, _)) = listener.accept() {
                drop(stream);
            }
        });
        let err = bridge_call(&addr, "ui.state", json!({})).unwrap_err();
        assert!(
            err.contains("without replying") || err.contains("bridge"),
            "unexpected error: {err}"
        );

        // 2. Payload ok:false detection
        assert!(is_payload_error(&json!({ "ok": false, "error": "failed" })));
        assert!(!is_payload_error(&json!({ "ok": true })));
        assert!(!is_payload_error(&json!({ "viewport_tab": "2d" })));
    }
}

#[cfg(test)]
mod project_argument_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("mc-project-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
    fn args(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }

    #[test]
    fn file_folder_and_video_target_folder_resolve_to_the_one_project() {
        let dir = temp("one");
        let file = dir.join("Song.acproj");
        std::fs::write(&file, "{}").unwrap();
        std::fs::create_dir_all(dir.join("hyperframes/src")).unwrap();
        let canonical = std::fs::canonicalize(&file).unwrap();
        for path in [file.clone(), dir.clone(), dir.join("hyperframes"), dir.join("hyperframes/src")] {
            assert_eq!(resolve_project(&path).unwrap(), canonical, "{}", path.display());
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn folders_with_no_or_several_projects_and_other_files_are_refused() {
        let empty = temp("none");
        assert!(resolve_project(&empty).unwrap_err().contains("no .acproj"));
        let many = temp("many");
        std::fs::write(many.join("A.acproj"), "{}").unwrap();
        std::fs::write(many.join("B.acproj"), "{}").unwrap();
        assert!(resolve_project(&many).unwrap_err().contains("has 2 .acproj files"));
        std::fs::write(empty.join("notes.txt"), "").unwrap();
        assert!(resolve_project(&empty.join("notes.txt")).unwrap_err().contains("not an .acproj"));
        assert!(resolve_project(&empty.join("missing.acproj")).unwrap_err().contains("no such project"));
        std::fs::remove_dir_all(&empty).unwrap();
        std::fs::remove_dir_all(&many).unwrap();
    }

    #[test]
    fn a_leading_project_becomes_pinned_params_and_a_bare_id_runs_as_exec() {
        let dir = temp("argv");
        let file = dir.join("Song.acproj");
        std::fs::write(&file, "{}").unwrap();
        let p = std::fs::canonicalize(&file).unwrap().display().to_string();
        let f = file.display().to_string();
        let pinned = [format!("project={p}"), format!("_expected_project={p}")];
        let got = project_argument(args(&["mc", "--bridge", "auto", &f, "center.tail", "last=5"])).unwrap();
        assert_eq!(got[..6], args(&["mc", "--bridge", "auto", "exec", "center.tail", "last=5"])[..]);
        assert_eq!(got[6..], pinned[..]);
        let got = project_argument(args(&["mc", &f, "clips", "character=bee"])).unwrap();
        assert_eq!(got[1..3], args(&["clips", "character=bee"])[..], "subcommands stay as they are");
        assert_eq!(got[3..], pinned[..]);
        let plain = args(&["mc", "exec", "center.snapshot", &format!("project={f}")]);
        assert_eq!(project_argument(plain.clone()).unwrap(), plain, "key=value forms are unchanged");
        assert!(project_argument(args(&["mc", "/no/such/place", "center.snapshot"])).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_studio_showing_another_project_refuses_the_command() {
        let dir = temp("wrong");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let (mine, other) = (dir.join("Mine.acproj"), dir.join("Other.acproj"));
        std::fs::write(&mine, "{}").unwrap();
        std::fs::write(&other, "{}").unwrap();
        let socket = dir.join("studio.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).unwrap();
        let shown = std::fs::canonicalize(&other).unwrap();
        let served = std::thread::spawn(move || {
            use std::io::{BufRead, Write};
            let mut ids = Vec::new();
            for stream in listener.incoming().take(1) {
                let mut stream = stream.unwrap();
                let mut line = String::new();
                std::io::BufReader::new(&stream).read_line(&mut line).unwrap();
                ids.push(serde_json::from_str::<Value>(&line).unwrap()["id"].as_str().unwrap().to_owned());
                writeln!(stream, "{}", json!({"ok": true, "result": {"active_project": shown, "project_guard_version": 1}})).unwrap();
            }
            ids
        });
        let err = local_ui::call(&format!("unix:{}", socket.display()), "workshop.status", json!({"_expected_project": mine})).unwrap_err();
        assert!(err.contains("changed project"), "{err}");
        assert_eq!(served.join().unwrap(), ["ui.feedback"], "only the identity check reached the Studio; the command never ran");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

#[cfg(test)]
mod storyboard_text_tests {
 use super::*;
 #[test]
 fn shortcut_keeps_dash_text_and_uses_explicit_field(){
  let raw=["--bridge","19897","storyboard","edit","scene","14","----","field=drawing"].map(str::to_owned);
  let value=storyboard_edit_params(&raw).unwrap();
  assert_eq!(value["scene"],14);assert_eq!(value["value"],"----");assert_eq!(value["field"],"drawing");
 }
 #[test]
 fn shortcut_defaults_to_named_action_field_and_rejects_ambiguous_extra_args(){
  let raw=["storyboard","edit","scene","4","Saved action"].map(str::to_owned);
  assert_eq!(storyboard_edit_params(&raw).unwrap()["field"],"action");
  let bad=["storyboard","edit","scene","4","Saved action","other"].map(str::to_owned);
  assert!(storyboard_edit_params(&bad).is_err());
 }
}
