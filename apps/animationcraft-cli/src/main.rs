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
  exec <id> [key=value ...]           run one command; prints its result as JSON
  vision [image_path | character]     direct pixel/alpha/drift/lift vision analysis (or live GUI viewport when --bridge is used)
  run <script.jsonl | ->              run commands, one {\"id\",\"params\"} per line
  mcp                                 MCP server on stdio
  help                                this text
  --version                           print the version

LIVE UI CONTROL COMMANDS (via `--bridge <port>`)
  ui.state                            inspect live GUI tab, anim_mode, yaw_deg, pose_range_pct
  ui.inspect [filter=...]             list all interactive UI elements & bounding boxes
  ui.click id=<elem_id> | x=.. y=..   click any registered UI button/tab or screen coordinate
  ui.vision                           analyze live viewport pixel buffer directly (no screenshot download needed)
  ui.screenshot [output=...]          save window PNG + return inline vision metrics
  ui.set_view tab=2d|3d mode=Walk yaw=90 pose_range_pct=70
                                      switch live GUI viewport tab, action cycle, yaw, or pose range

VALUES
  key=value parses value as JSON when it can (3.5, true, [1,2], {\"a\":1}), else as a string.

EXIT STATUS
  0 success · 1 a command failed · 2 usage error
";

fn bridge_call(addr: &str, id: &str, params: Value) -> Result<Value, String> {
    use std::io::{BufRead, BufReader};
    use std::net::TcpStream;

    let mut stream = TcpStream::connect(addr).map_err(|e| format!("bridge connect {addr}: {e}"))?;
    let payload = json!({ "id": id, "params": params }).to_string();
    writeln!(stream, "{payload}").map_err(|e| format!("bridge write: {e}"))?;
    stream.flush().map_err(|e| format!("bridge flush: {e}"))?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| format!("bridge read: {e}"))?;
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

// The shared positional parser skips dash-prefixed tokens, including stdin's
// literal `-`. Recover that marker here without changing the parser's owner.
fn run_script_argument<'a>(positional: Option<&'a str>, raw_args: &[String]) -> Option<&'a str> {
    positional.or_else(|| raw_args.iter().any(|arg| arg == "-").then_some("-"))
}

fn run_mcp_stdio(bridge: Option<String>) {
    use std::io::BufRead;
    let stdin = std::io::stdin();
    let mut session = Session::default();
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
                let tools: Vec<Value> = animationcraft_engine::command_specs()
                    .iter()
                    .map(|spec| {
                        json!({
                            "name": spec.id.replace('.', "_"),
                            "description": format!("{} ({})", spec.label, spec.params),
                            "inputSchema": { "type": "object" }
                        })
                    })
                    .collect();
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "tools": tools }
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
                let res = if let Some(addr) = &bridge {
                    bridge_call(addr, cmd_id, args)
                } else {
                    session.execute(cmd_id, args).map_err(|e| e.to_string())
                };
                match res {
                    Ok(v) => json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() }]
                        }
                    }),
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
                Ok(v) => outln!("{}", serde_json::to_string_pretty(&v).unwrap_or_default()),
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
                Ok(v) => outln!("{}", serde_json::to_string_pretty(&v).unwrap_or_default()),
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
                    Ok(r) => outln!(
                        "{}",
                        json!({"line": n + 1, "id": id, "ok": true, "result": r})
                    ),
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
    fn engine_has_expected_commands() {
        let specs = animationcraft_engine::command_specs();
        let ids: Vec<&str> = specs.iter().map(|s| s.id).collect();
        assert!(ids.contains(&"intake.validate"));
        assert!(ids.contains(&"intake.three_question"));
        assert!(ids.contains(&"asset.cut_sheet"));
        assert!(ids.contains(&"asset.load_manifest"));
        assert!(ids.contains(&"part.set_state"));
        assert!(ids.contains(&"part.show_all"));
        assert!(ids.contains(&"export.agent_prompt"));
        assert!(ids.contains(&"turnaround.optimize_accuracy"));
        assert!(ids.contains(&"anim.preview_2d"));
        assert!(ids.contains(&"mesh.build_4dir"));
        assert!(ids.contains(&"export.effectcraft"));
        assert!(ids.contains(&"export.filmcraft"));
        assert!(ids.contains(&"export.blender"));
        assert!(ids.contains(&"vision.analyze"));
    }
}
