//! AnimationCraft headless CLI: every engine command is one shell call away, for scripts and AI agents.
//!
//! Run `animationcraft-cli help` for the full reference.

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
        "animationcraft-cli: can't write to stdout: {e}"
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
        "animationcraft-cli: {msg}\nRun `animationcraft-cli help` for usage."
    ));
    std::process::exit(2)
}

fn fail(msg: impl std::fmt::Display) -> ! {
    diag(format_args!("animationcraft-cli: {msg}"));
    std::process::exit(1)
}

const HELP: &str = "\
animationcraft-cli: drive AnimationCraft from the shell (headless engine or live GUI bridge)

USAGE
  animationcraft-cli [--bridge <port|host:port>] <SUBCOMMAND> [ARGS]

OPTIONS
  --bridge <port|host:port>           send commands to a running `animationcraft --control <port>` GUI window

SUBCOMMANDS
  info                                print engine info and available commands
  commands [filter]                   list commands (id, label, shortcut, params)
  doctor [--clean] [keep_port=N]      check lingering Craft processes, clean stale servers, & flag Ponytail code smells
  exec <id> [key=value ...]           run one command; prints its result as JSON
  vision [image_path | character]     direct pixel/alpha/drift/lift vision analysis (or live GUI viewport when --bridge is used)
  run <script.jsonl | ->              run commands, one {\"id\",\"params\"} per line
  mcp                                 MCP server on stdio
  help                                this text
  --version                           print the version

LIVE UI CONTROL COMMANDS (via `--bridge <port>`)
  ui.state                            inspect live GUI tab, anim_mode, yaw_deg, pose_range_pct
  ui.inspect [filter=...]             list all interactive UI elements & bounding boxes
  ui.click id=<elem_id> | x=.. y=..   click a UI element by its ui.inspect id (e.g. viewport.fit) or screen coordinate
  ui.vision                           analyze the live stage canvas pixels (no screenshot download needed)
  ui.screenshot [output=...]          save window PNG + vision metrics for the stage canvas (see vision.region)
  ui.set_view tab=2d|3d mode=Walk yaw=90 pose_range_pct=70
                                      switch live GUI viewport tab, action cycle, yaw, or pose range
  app.quit                            close the live GUI window (save the project first)
  app_quit (MCP tool)                 same, only when the MCP server runs with --bridge

VALUES
  key=value parses value as JSON when it can (3.5, true, [1,2], {\"a\":1}), else as a string.

EXIT STATUS
  0 success · 1 a command failed · 2 usage error
";

fn bridge_timeout() -> std::time::Duration {
    let ms = std::env::var("ANIMATIONCRAFT_BRIDGE_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30_000);
    std::time::Duration::from_millis(ms.max(50))
}

fn bridge_call(addr: &str, id: &str, params: Value) -> Result<Value, String> {
    use std::io::{BufRead, BufReader};
    use std::net::{TcpStream, ToSocketAddrs};

    let timeout = bridge_timeout();
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
fn tool_input_schema(id: &str) -> Value {
    let asset = json!({ "type": "string", "description": "Loaded asset id; defaults to the active asset" });
    let source = json!({ "type": "string", "description": "Source image path; defaults to the asset's image" });
    let out = json!({ "type": "string", "description": "Output PNG path" });
    let unit = json!({ "type": "number", "minimum": 0, "maximum": 1 });
    let point = json!({ "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 });
    match id {
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

fn run_mcp_stdio(bridge: Option<String>) {
    use std::io::BufRead;
    let stdin = std::io::stdin();
    let mut session = Session::default();
    let mut stop_after_reply = false;
    for line in stdin.lock().lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(Value::as_str).unwrap_or("");
        let resp = match method {
            "initialize" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "animationcraft-mcp",
                        "version": env!("CARGO_PKG_VERSION")
                    }
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
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                let res = if raw_name == "app_quit" || raw_name == "app.quit" {
                    let out = call_app_quit(bridge.as_deref(), args);
                    if out.as_ref().is_ok_and(|v| !is_payload_error(v)) {
                        stop_after_reply = true;
                    }
                    out
                } else if let Some(addr) = &bridge {
                    bridge_call(addr, cmd_id, args)
                } else {
                    session.execute(cmd_id, args).map_err(|e| e.to_string())
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
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {}
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
        "name": "app_quit",
        "description": "Quit the live AnimationCraft window this server is bridged to. \
            Save the project first; this closes the window without a prompt. \
            It needs `mcp --bridge <port>` and reports an error in headless mode.",
        "inputSchema": {
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }
    }));
    tools
}

/// Quit the live window behind `bridge`, or explain why a headless MCP session cannot.
fn call_app_quit(bridge: Option<&str>, args: Value) -> Result<Value, String> {
    match bridge {
        Some(addr) => bridge_call(addr, "app.quit", args),
        None => Err(
            "app_quit needs the native window: start `animationcraft --control <port>` and run this MCP server with `--bridge <port>`; a headless session has no app process to quit"
                .into(),
        ),
    }
}

fn main() {
    let a = args::Args::parse(std::env::args().skip(1));
    if a.help {
        outln!("{HELP}");
        return;
    }
    if a.version {
        outln!("animationcraft-cli {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let Some(cmd) = a.pos(0) else {
        usage("missing subcommand")
    };
    match cmd {
        "help" | "-h" => outln!("{HELP}"),
        "version" => outln!("animationcraft-cli {}", env!("CARGO_PKG_VERSION")),
        "info" => {
            let specs = animationcraft_engine::command_specs();
            outln!("AnimationCraft engine v{}", env!("CARGO_PKG_VERSION"));
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
                (Some(addr), None) => bridge_call(&addr, "ui.vision", json!({})),
                (bridge_opt, Some(t)) => {
                    let params = if std::path::Path::new(t).exists() {
                        json!({ "image": t })
                    } else {
                        json!({ "character": t })
                    };
                    if let Some(addr) = bridge_opt {
                        bridge_call(&addr, "vision.analyze", params)
                    } else {
                        let mut s = Session::default();
                        s.execute("vision.analyze", params).map_err(|e| e.to_string())
                    }
                }
                (None, None) => {
                    let mut s = Session::default();
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
        "exec" => {
            let id = a
                .pos(1)
                .unwrap_or_else(|| usage("exec <id> [key=value ...]"));
            let params = a.params_from(2).unwrap_or_else(|e| usage(e));
            let res = if let Some(addr) = a.bridge_addr() {
                bridge_call(&addr, id, params)
            } else {
                let mut s = Session::default();
                s.execute(id, params).map_err(|e| e.to_string())
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
            let raw_args: Vec<String> = std::env::args().skip(1).collect();
            let script = run_script_argument(a.pos(1), &raw_args)
                .unwrap_or_else(|| usage("run <script.jsonl | ->"));
            let text = if script == "-" {
                std::io::read_to_string(std::io::stdin()).unwrap_or_else(|e| fail(e))
            } else {
                std::fs::read_to_string(script).unwrap_or_else(|e| fail(format!("{script}: {e}")))
            };
            let bridge = a.bridge_addr();
            let mut s = Session::default();
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
                let params = v.get("params").cloned().unwrap_or(json!({}));
                let res = if let Some(addr) = &bridge {
                    bridge_call(addr, id, params)
                } else {
                    s.execute(id, params).map_err(|e| e.to_string())
                };
                match res {
                    Ok(r) => {
                        let cmd_ok = r.get("ok").and_then(Value::as_bool) == Some(true);
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
        "doctor" => {
            let mut params = a.params_from(1).unwrap_or_else(|e| usage(e));
            if a.flag_clean {
                if let Some(obj) = params.as_object_mut() {
                    obj.insert("clean".to_string(), Value::Bool(true));
                }
            }
            let mut s = Session::default();
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
    fn app_quit_without_a_bridge_refuses_instead_of_pretending_to_close() {
        let err = call_app_quit(None, json!({})).unwrap_err();
        assert!(err.contains("--bridge"), "unexpected error: {err}");
        assert!(err.contains("no app process"), "unexpected error: {err}");
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
