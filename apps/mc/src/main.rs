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
mod dials;
mod find;
mod github_health;
mod local_ui;
mod logo;
mod mayhem;
mod mcp_presence;
mod mcp_watch;
mod panels;
mod scope;
mod settings;
mod snapshot;
mod studios;
mod telemetry;
mod theme;
mod touch;
mod tui;
mod worker;

use magicstudio_engine::Session;
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
    diag(format_args!("mc: can't write to stdout: {e}"));
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
    diag(format_args!("mc: {msg}\nRun `mc help` for usage."));
    std::process::exit(2)
}

fn fail(msg: impl std::fmt::Display) -> ! {
    diag(format_args!("mc: {msg}"));
    std::process::exit(1)
}

const HELP: &str = "\
mc: drive Magic Studio from the shell (headless engine, TUI, or live GUI app)

USAGE
  mc [project=<file>|<project.magic>]                      open Magic Studio with an automatic endpoint
  mc app [<project.magic>]                                 open a new window or focus that project
  mc backstage [project=<file>]                            launch the interactive Backstage TUI
  mc [--bridge <port|host:port>] <SUBCOMMAND> [ARGS]

OPTIONS
  --bridge <port|host:port|auto|unix:path>  send commands to a running window; `mc studios --json` lists them

QUICK START
  mc                                   launch Magic Studio desktop app
  mc backstage                         launch Backstage TUI
  mc tui instances                     find a running TUI
  mc tui look                          read its frame and state
  mc --bridge auto exec ui.state       read the live Studio state
  mc commands --group                  list commands by menu
  mc find <words>                      find commands, projects and controls

SUBCOMMANDS
  app, studio [project=<file>]         launch the Magic Studio desktop app (also default when running bare `mc`)
  panels [--json|--check]              shared UI configuration and live panel telemetry
  tui look [instance=<pid>]            actual live TUI frame, state and all controls as JSON
  tui read [instance=<pid>]            the actual rendered terminal text
  tui watch [notify=<agent>] [count=<n>]
                                      stream only the selected TUI's new typed events; notify
                                      prompts that agent once per distinct new failure, never
                                      retried, and nothing already on screen is replayed
  tui instances                       list controllable TUIs; multiple instances require selection
  tui controls                        discover named actions and equivalent keys
  tui view name=booth|workshop|mops|telemetry
  tui agent name=gonzo                 select an agent (pane=<id> disambiguates names)
  tui click id=<control-id>            click a visible button; look section=elements lists IDs
  tui event index=0                    inspect an event; sequence=<id> selects a stable receipt
  tui note text='details'              draft a note; note.send explicitly submits it
  tui key key=F4                      every keyboard control uses the same live handler
  tui reload                          reload the same foreground TUI after an update
  backstage [project=<file>]           interactive Backstage (no automatic Studio launch)
                                      F1 WORKSHOP · F2 BOOTH · F3 MOPS · D PULSE
                                      optional root= name= audio= transcript= captions= audiomap=
                                      --print prints current state without entering terminal UI
  studios [--json] [<port>|--unpin|--check]
                                      which Studio is running: port, project, view, readiness, and
                                      how to confirm it. <port> pins it for plain `mc exec` calls;
                                      --check exits 0 only when one is UI-ready; with none running,
                                      names both ways forward (start a window, or read the TUI)
  booth, center                      aliases of `backstage`
  exec ui.feedback                    discover canonical Studio identity, readiness, unsaved and ack state
  exec ui.components region=#view-project limit=30
                                      bounded actual DOM geometry/style/state; no input values
  exec backstage.events after=0 limit=20  bounded activity journal; use next_cursor for later reads
  exec backstage.snapshot [project=<file>]
                                      one JSON object with stable keys: project, subject, clip,
                                      playhead_sec, selected_part, unsaved_edits, hyperframes, fps,
                                      last_error (null = not observed); the Backstage status line
  exec backstage.tail [last=20] [project=<file>]
                                      last N typed events: saved, generation_started/finished,
                                      keyframe_added/removed (Theatre), motion_path_drawn/undone,
                                      selection_changed, clip_switched, clip_sheet_*, error
                                      (activity history: last 512, UI clicks never evict them)
  login [provider]                    native OpenCode authentication in your terminal (no --bridge)
  info                                print engine info and available commands
  commands [filter] [--group]         list commands (id, label, shortcut, params);
                                      --group by menu, read-only commands last
  find [words] [project=<file>] [--json]  ranked commands, project objects and visible controls; no words suggests likely reads
  doctor [--clean]      check external Craft CLI availability and .video-server.json (not verified; not auto-deleted)
  storyboard --doctor                read-only section-boundary task list for the selected Studio project
                                      exit 0: no supported issue; 1: findings; 2: insufficient evidence
  storyboard create|read|update|delete [key=value ...]  shared native storyboard CRUD
  storyboard edit scene <N> <text> [field=action]  edit an existing native scene text field and save
  exec <id> [key=value ...]           run one command; prints its result as JSON
  <project> image add <file> [name=...] [background=... | character=...]
                                      register an image; outside files are copied into project assets
  <project> <subcommand | command id> [ARGS]
                                      run against one project: an .magic, or a folder with exactly
                                      one (a song's hyperframes/ folder finds its song);
                                      same as project=<file> _expected_project=<file>, e.g.
                                      mc \"Bumble Bee.magic\" center.snapshot
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
  sprocket [--status|--log|--models|--use <provider/id>|<message...>]
                                      Sprocket session, transcript, model catalog/selection or a message

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
  ~/.agents/skills/magicstudio/SKILL.md  workflow, tools and gotchas (the `magicstudio` skill)

EXIT STATUS
  0 success · 1 a command failed · 2 usage error
";

/// How long to wait for one bridge reply. An explicit `MAGICSTUDIO_BRIDGE_TIMEOUT_MS` wins;
/// otherwise commands the engine marks `long_running` (exports, renders, agent runs) get an hour
/// and everything else 30 s, so a slow render is not reported as a dead bridge.
fn bridge_timeout(id: &str) -> std::time::Duration {
    if let Some(ms) = std::env::var("MAGICSTUDIO_BRIDGE_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        return std::time::Duration::from_millis(ms.max(50));
    }
    let long = magicstudio_engine::find_command(id).is_some_and(|c| c.long_running);
    std::time::Duration::from_secs(if long { 3600 } else { 30 })
}

/// Names the caller on a command: the herdr pane it runs in. Every UI action goes through the
/// CLI, so the journal can say which agent touched which element. Unverified, never overwritten.
fn with_client(mut params: Value) -> Value {
    if let (true, Ok(pane)) = (
        params.is_object() && params["_client"].is_null(),
        std::env::var("HERDR_PANE_ID"),
    ) {
        params["_client"] = json!(format!("herdr:{pane}"));
    }
    params
}

fn bridge_call(addr: &str, id: &str, params: Value) -> Result<Value, String> {
    let params = with_client(params);
    touch::record(id, &params);
    if let Some(scope) = scope::current()? {
        let params = scope.params(id, params)?;
        if id == "command.list" {
            return Ok(scope.commands(params["filter"].as_str().unwrap_or("")));
        }
        if addr != "auto" {
            let live = magicstudio_engine::studio_sessions::for_project(&scope.project)?
                .ok_or("Restricted: assigned project's Studio is unavailable")?;
            if live["endpoint"].as_str() != Some(addr) {
                return Err("Restricted: bridge does not belong to the assigned project".into());
            }
        }
        return bridge_call_unscoped(addr, id, params).map(|v| scope.redact(v));
    }
    bridge_call_unscoped(addr, id, params)
}
fn bridge_call_unscoped(addr: &str, id: &str, params: Value) -> Result<Value, String> {
    if addr == "auto" || addr.starts_with("unix:") {
        return local_ui::call(addr, id, params);
    }
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

mod mcp;
use mcp::*;

/// `agent say|read|session`: talk to the project's in-app opencode agent from outside the app.
/// Deliberately not an engine command, so it is not an MCP tool the agent itself could call.
fn agent_command(
    verb: &str,
    text: Option<&str>,
    params: &Value,
    bridge: Option<String>,
) -> Result<(), String> {
    let command = match verb {
        "say" => "agent.send",
        "read" => "agent.log",
        "session" => "agent.session",
        _ => return Err(
            "usage: agent say \"<text>\" [project=<existing project>] | agent read | agent session"
                .into(),
        ),
    };
    let mut request = params.clone();
    if verb == "say" {
        request["message"] = json!(text.ok_or("agent say needs message text")?);
    }
    let result = if let Some(addr) = bridge {
        if params.get("project").is_some() {
            return Err("Use an explicitly pinned Studio bridge or a headless project path; no automatic project switch".into());
        }
        bridge_call(&addr, command, request)?
    } else {
        let mut session = Session::user_session();
        let target = params["project"]
            .as_str()
            .map(std::path::PathBuf::from)
            .unwrap_or(std::env::current_dir().map_err(|e| e.to_string())?);
        let project = if target.extension().is_some_and(|e| e == "magic") {
            target
        } else {
            let files: Vec<_> = std::fs::read_dir(&target)
                .map_err(|_| "Project directory unavailable; pass project=<existing .magic>")?
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "magic"))
                .collect();
            if files.len() != 1 {
                return Err("Pass project=<existing .magic>; no unique project found and no bootstrap project was created".into());
            }
            files[0].clone()
        };
        session
            .execute("project.attach", json!({"path":project}))
            .map_err(|e| e.to_string())?;
        request["_origin"] = json!("CLI agent");
        dispatch(&mut session, command, request)?
    };
    if verb == "say" {
        if let Some(text) = result["text"].as_str() {
            outln!("{}", text);
        } else {
            outln!("{}", result);
        }
    } else if verb == "session" {
        outln!(
            "{}",
            result["session"].as_str().ok_or(
                "No recorded provider session; inspect agent.config and start a fresh chat"
            )?
        );
    } else {
        outln!("{}", result);
    }
    if result["ok"] == false {
        return Err(format!(
            "Agent needs attention in session {}; inspect agent read project=<same .magic> before retrying",
            result["session"]
        ));
    }
    Ok(())
}

/// Quit the live window behind `bridge`, or explain why a headless MCP session cannot.
pub(crate) fn dispatch(session: &mut Session, id: &str, params: Value) -> Result<Value, String> {
    let params = with_client(params);
    touch::record(id, &params);
    if let Some(scope) = scope::current()? {
        let params = scope.params(id, params)?;
        if id == "command.list" {
            return Ok(scope.commands(params["filter"].as_str().unwrap_or("")));
        }
        if session
            .project_path
            .as_ref()
            .is_some_and(|p| p != &scope.project)
        {
            return Err("Restricted: session belongs to another project".into());
        }
        if session.project_path.is_none() {
            session
                .execute("project.attach", json!({"path":scope.project}))
                .map_err(|e| e.to_string())?;
        }
        return dispatch_unscoped(session, id, params).map(|v| scope.redact(v));
    }
    dispatch_unscoped(session, id, params)
}
fn dispatch_unscoped(session: &mut Session, id: &str, mut params: Value) -> Result<Value, String> {
    if matches!(id, "project.open" | "project.identity" | "project.attach")
        && params["path"].is_null()
        && params["project"].is_string()
    {
        params["path"] = params["project"].clone();
    }
    if !matches!(
        id,
        "project.open" | "project.new" | "project.intake" | "project.attach" | "project.migrate" | "project.identity"
    ) && let Some(target) = params["project"].as_str()
    {
        let identity = session
            .execute("project.identity", json!({"path": target}))
            .map_err(|e| e.to_string())?;
        if let Some(project) = identity["project"].as_str()
            && session.project_path.as_deref() != Some(std::path::Path::new(project))
        {
            session
                .execute("project.attach", json!({"path": project}))
                .map_err(|e| e.to_string())?;
        }
    }
    session.execute(id, params).map_err(|e| e.to_string())
}

/// `backstage.snapshot` / `backstage.tail` are mc observations, not engine commands; None for any other id.
fn center_observation(id: &str, params: &Value, bridge: Option<&str>) -> Option<Value> {
    if !matches!(
        id,
        "center.snapshot"
            | "booth.snapshot"
            | "backstage.snapshot"
            | "center.tail"
            | "booth.tail"
            | "backstage.tail"
    ) {
        return None;
    }
    let scoped = match scope::current() {
        Ok(Some(scope)) => match scope.params(id, params.clone()) {
            Ok(params) => Some((scope, params)),
            Err(error) => return Some(json!({"ok":false,"error":error})),
        },
        Ok(None) => None,
        Err(error) => return Some(json!({"ok":false,"error":error})),
    };
    let params = scoped.as_ref().map(|(_, p)| p).unwrap_or(params);
    let project = params["project"].as_str();
    let result = match id {
        "center.snapshot" | "booth.snapshot" | "backstage.snapshot" => {
            Some(snapshot::collect(project, bridge).0)
        }
        "center.tail" | "booth.tail" | "backstage.tail" => {
            let last = params["last"].as_u64().unwrap_or(20).clamp(1, 192) as usize;
            let (state, activity) = snapshot::collect(project, bridge);
            let events = snapshot::tail(&activity, state["project"].as_str());
            let events = &events[events.len().saturating_sub(last)..];
            Some(
                json!({"schema": snapshot::TAIL_SCHEMA, "project": state["project"], "events": events,
                "next_cursor": activity["next_cursor"], "error": activity["error"]}),
            )
        }
        _ => None,
    };
    result.map(|value| {
        if let Some((scope, _)) = &scoped {
            scope.redact(value)
        } else {
            value
        }
    })
}

fn call_app_quit(bridge: Option<&str>, mut args: Value) -> Result<Value, String> {
    args["_origin"] = json!("MCP");
    bridge_call(bridge.unwrap_or("auto"), "app.quit", args)
}

const SUBCOMMANDS: &[&str] = &[
    "app",
    "studio",
    "help",
    "-h",
    "panels",
    "version",
    "login",
    "workshop",
    "tui",
    "backstage",
    "booth",
    "center",
    "sprocket",
    "info",
    "commands",
    "find",
    "vision",
    "storyboard",
    "exec",
    "run",
    "mcp",
    "agent",
    "doctor",
    "debug",
    "clips",
    "image",
];

fn studio_receipt(live: &Value) -> Value {
    json!({
        "ok": true,
        "active_project": live["active_project"],
        "endpoint": live["endpoint"],
        "instance": live["instance"],
        "identity": {
            "pid": live["identity"]["pid"],
            "control_port": live["identity"]["control_port"],
            "build": live["identity"]["build_id"],
        },
        "ui_ready": live["ui_ready"],
        "next": "mc --bridge auto exec ui.feedback (MCP: ui_feedback)",
    })
}

fn launch_studio_app(project: Option<&str>, port: Option<&str>) -> ! {
    if port.is_some() { fail("Opening a window allocates its endpoint automatically; use --bridge to address an existing window"); }
    match studios::open(project) {
        Ok(live) => {
            let receipt = studio_receipt(&live);
            outln!("{}", serde_json::to_string_pretty(&receipt).unwrap_or_default());
            std::process::exit(0);
        }
        Err(error) => fail(error),
    }
}

/// The command line with a leading project path resolved (see `project_argument`), read once.
fn cli_args() -> &'static Vec<String> {
    static ARGS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    ARGS.get_or_init(|| {
        project_argument(std::env::args().collect())
            .and_then(friendly_argument)
            .unwrap_or_else(|e| usage(e))
    })
}

/// `mc <project> <subcommand | command id> ...`: a leading project path becomes
/// `project=<file> _expected_project=<file>` for this call, and a bare command id after it
/// runs as `exec <id>`. Anything else is returned unchanged.
fn project_argument(mut raw: Vec<String>) -> Result<Vec<String>, String> {
    let mut i = 1;
    while i < raw.len() && raw[i].starts_with('-') && !SUBCOMMANDS.contains(&raw[i].as_str()) {
        i += if raw[i] == "--bridge" || raw[i] == "--control" {
            2
        } else {
            1
        };
    }
    let Some(token) = raw.get(i).cloned() else {
        return Ok(raw);
    };
    if SUBCOMMANDS.contains(&token.as_str()) || !(token.contains('/') || token.ends_with(".magic"))
    {
        return Ok(raw);
    }
    let project = resolve_project(std::path::Path::new(&token))?;
    raw.remove(i);
    if raw
        .get(i)
        .is_some_and(|next| !SUBCOMMANDS.contains(&next.as_str()) && next.contains('.'))
    {
        raw.insert(i, "exec".into());
    }
    raw.push(format!("project={}", project.display()));
    raw.push(format!("_expected_project={}", project.display()));
    Ok(raw)
}

/// Small spellings only; the shared engine owns every operation.
fn friendly_argument(mut raw: Vec<String>) -> Result<Vec<String>, String> {
    let mut i = 1;
    while i < raw.len() && raw[i].starts_with('-') && !SUBCOMMANDS.contains(&raw[i].as_str()) {
        i += if raw[i] == "--bridge" || raw[i] == "--control" {
            2
        } else {
            1
        };
    }
    if raw.get(i).map(String::as_str) == Some("storyboard") {
        if let Some(verb) = raw
            .get(i + 1)
            .filter(|v| ["create", "read", "update", "delete"].contains(&v.as_str()))
            .cloned()
        {
            raw.splice(i..i + 2, ["exec".into(), format!("storyboard.{verb}")]);
            return Ok(raw);
        }
        if raw.get(i + 1).map(String::as_str) == Some("edit") {
            if raw.get(i + 2).map(String::as_str) != Some("scene") || raw.get(i + 4).is_none() {
                return Err("storyboard edit scene <N> <text> [field=action]".into());
            }
            let scene = raw[i + 3]
                .parse::<u64>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or("scene must be a positive number")?;
            if raw[i + 5..].iter().any(|v| !v.contains('=')) {
                return Err("Options after the text use key=value".into());
            }
            let text = raw[i + 4].clone();
            let field = raw[i + 5..].iter().any(|v| v.starts_with("field="));
            raw.splice(
                i..i + 5,
                [
                    "exec".into(),
                    "storyboard.update".into(),
                    format!("scene={scene}"),
                    format!("value={text}"),
                ],
            );
            if !field {
                raw.push("field=action".into());
            }
            return Ok(raw);
        }
    }
    if raw.get(i).map(String::as_str) != Some("image") {
        return Ok(raw);
    }
    if raw.get(i + 1).map(String::as_str) != Some("add") || raw.get(i + 2).is_none() {
        return Err(
            "image add <file> [name=<label>] [background=<name> | character=<name>]".into(),
        );
    }
    if raw[i + 3..].iter().any(|v| !v.contains('=')) {
        return Err("image add takes one file; options use key=value".into());
    }
    let path = std::fs::canonicalize(&raw[i + 2]).map_err(|e| format!("{}: {e}", raw[i + 2]))?;
    if !path.is_file() {
        return Err("image add needs an image file".into());
    }
    if !path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        [
            "png", "jpg", "jpeg", "webp", "svg", "gif", "bmp", "tif", "tiff", "exr",
        ]
        .contains(&e.to_ascii_lowercase().as_str())
    }) {
        return Err("Unsupported image extension".into());
    }
    if raw[i + 3..].iter().any(|v| {
        v.starts_with("path=")
            || v.starts_with("kind=")
            || v.starts_with("preset=")
            || v.starts_with("save=")
    }) {
        return Err(
            "image add fixes path and kind=image; use project.add_asset for other asset types"
                .into(),
        );
    }
    let destination = raw[i + 3..].iter().any(|v| {
        ["background=", "character=", "image="]
            .iter()
            .any(|key| v.starts_with(key))
    });
    raw.splice(
        i..i + 3,
        [
            "exec".into(),
            "project.add_asset".into(),
            format!("path={}", path.display()),
            "kind=image".into(),
            "save=true".into(),
        ],
    );
    if !destination {
        let folder: String = path
            .file_stem()
            .and_then(|v| v.to_str())
            .unwrap_or("image")
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        raw.push(format!("image={folder}"));
    }
    Ok(raw)
}

/// An .magic file, or the one .magic in a folder. A song project's video-target folder
/// (hyperframes/, or one level inside it) resolves to that song project.
fn resolve_project(path: &std::path::Path) -> Result<std::path::PathBuf, String> {
    if path.is_file() {
        if path.extension().is_some_and(|e| e == "magic") {
            return std::fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()));
        }
        return Err(format!("{} is not an .magic project file", path.display()));
    }
    if !path.is_dir() {
        return Err(format!(
            "{}: no such project file or folder",
            path.display()
        ));
    }
    for dir in path.ancestors().take(3) {
        // Above the given folder, only a song project's video-target folder leads up to it.
        let via_target = path
            .strip_prefix(dir)
            .ok()
            .and_then(|rest| rest.components().next())
            .and_then(|c| c.as_os_str().to_str())
            .is_some_and(|c| magicstudio_engine::workshop::VIDEO_TARGETS.contains(&c));
        if dir != path && !via_target {
            continue;
        }
        let found: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "magic"))
            .collect();
        match found.as_slice() {
            [] => continue,
            [one] => return std::fs::canonicalize(one).map_err(|e| e.to_string()),
            many => {
                return Err(format!(
                    "{} has {} .magic files; pass the one you mean",
                    dir.display(),
                    many.len()
                ));
            }
        }
    }
    Err(format!(
        "no .magic in {} (or above it, from a hyperframes folder); pass the project file",
        path.display()
    ))
}

fn main() {
    let a = args::Args::parse(cli_args().iter().cloned().skip(1));
    let assignment = scope::current().unwrap_or_else(|e| fail(e));
    if assignment.is_some() && a.help {
        outln!(
            "Project Booth: mc booth | tui | commands | exec <granted command> | run | mcp. Views: BOOTH / MOPS / MAYHEM. Workshop and global controls belong to Jesse and Gonzo."
        );
        return;
    }
    if a.help {
        outln!("{HELP}");
        return;
    }
    if a.version {
        outln!("mc {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let Some(cmd) = a.pos(0).filter(|c| !c.contains('=')) else {
        if assignment.is_some() {
            usage("missing subcommand");
        }
        let params = a.params_from(0).unwrap_or_else(|e| usage(e));
        launch_studio_app(
            params["project"]
                .as_str()
                .or_else(|| params["path"].as_str()),
            a.control_port()
                .as_deref()
                .or_else(|| params["port"].as_str()),
        );
    };
    if let Some(scope) = assignment {
        if matches!(cmd, "help" | "-h" | "commands" | "info") {
            outln!("{}", scope.commands(a.pos(1).unwrap_or("")));
            return;
        }
        if !matches!(
            cmd,
            "version" | "tui" | "booth" | "backstage" | "center" | "exec" | "run" | "mcp"
        ) {
            fail(
                "Restricted: this shortcut is outside the assigned Booth; use commands for granted operations",
            );
        }
    }
    match cmd {
        "app" | "studio" => {
            let params = a.params_from(1).unwrap_or_else(|e| usage(e));
            launch_studio_app(
                a.pos(1)
                    .filter(|c| !c.contains('='))
                    .or_else(|| params["project"].as_str())
                    .or_else(|| params["path"].as_str()),
                a.control_port()
                    .as_deref()
                    .or_else(|| params["port"].as_str()),
            );
        }
        "help" | "-h" => outln!("{HELP}"),
        "tui" => {
            let action = a.pos(1).unwrap_or("look");
            let params = a.params_from(2).unwrap_or_else(|e| usage(e));
            // A watch streams new events as they are observed; every other action is one reply.
            if action == "watch" {
                match tui::watch(&params, &mut |value| outln!("{}", value)) {
                    Ok(summary) => outln!("{}", summary),
                    Err(error) => {
                        outln!("{}", json!({"ok":false,"error":error}));
                        std::process::exit(1);
                    }
                }
                return;
            }
            let value = tui::call(if action == "read" { "look" } else { action }, &params)
                .unwrap_or_else(|error| {
                    outln!("{}", json!({"ok":false,"error":error}));
                    std::process::exit(1)
                });
            if action == "read" || params["format"] == "text" {
                for line in value["screen"]["lines"].as_array().into_iter().flatten() {
                    outln!("{}", line.as_str().unwrap_or(""));
                }
                if let Some(error) = value["error"].as_str() {
                    outln!("ERROR: {error}");
                }
            } else {
                outln!(
                    "{}",
                    serde_json::to_string_pretty(&value).unwrap_or_default()
                );
            }
            if value["ok"] == false {
                std::process::exit(1);
            }
        }
        "version" => outln!("mc {}", env!("CARGO_PKG_VERSION")),
        "login" => {
            use std::io::IsTerminal;
            if a.bridge_addr().is_some() || a.pos(2).is_some() {
                usage("login [provider] runs locally without --bridge");
            }
            if !std::io::stdin().is_terminal()
                || !std::io::stdout().is_terminal()
                || !std::io::stderr().is_terminal()
            {
                fail("Login requires your interactive terminal");
            }
            let program = magicstudio_engine::agent::opencode_program().unwrap_or_else(|e| fail(e));
            diag(format_args!("Using OpenCode: {}", program.display()));
            let mut command = std::process::Command::new(program);
            command.args(["auth", "login"]);
            if let Some(provider) = a.pos(1) {
                command.arg(provider);
            }
            let status = command
                .status()
                .unwrap_or_else(|_| fail("Cannot start installed OpenCode"));
            std::process::exit(status.code().unwrap_or(1));
        }
        "sprocket" => {
            let raw = cli_args().iter().cloned().skip(1).collect::<Vec<_>>();
            let wants = |flag: &str| raw.iter().any(|v| v == flag);
            let message = raw
                .iter()
                .skip(1)
                .filter(|v| !v.starts_with("--") && !v.contains('='))
                .cloned()
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_string();
            let mut params = a.params_from(1).unwrap_or_else(|e| usage(e));
            params["_origin"] = json!("CLI");
            let read_only = wants("--status") || wants("--log");
            if wants("--models") {
                let result = if let Some(addr) = a.bridge_addr() {
                    bridge_call(&addr, "sprocket.models", params)
                } else {
                    let mut s = Session::user_session();
                    if let Some(project) = params["project"].as_str() {
                        s.execute("project.attach", json!({"path":project}))
                            .unwrap_or_else(|e| fail(format!("sprocket.models: {e}")));
                    }
                    dispatch(&mut s, "sprocket.models", params)
                };
                let result = result.unwrap_or_else(|e| fail(e));
                let list = result["models"].as_array().cloned().unwrap_or_default();
                for m in list.iter().take(40) {
                    let mark = if m["current"] == true {
                        "*"
                    } else if m["local"] == true {
                        "L"
                    } else if m["free"] == true {
                        "F"
                    } else {
                        " "
                    };
                    outln!(
                        "{mark} {}/{}  {}",
                        m["provider"].as_str().unwrap_or("?"),
                        m["model"].as_str().unwrap_or("?"),
                        m["name"].as_str().unwrap_or("")
                    );
                }
                if let Some(rec) = result["recommended"].as_object() {
                    outln!(
                        "recommended: {}/{}",
                        rec["provider"].as_str().unwrap_or("?"),
                        rec["model"].as_str().unwrap_or("?")
                    );
                }
                outln!(
                    "(L local · F free · * current — switch with: mc sprocket --use <provider/id>)"
                );
                std::process::exit(0);
            }
            if wants("--use") {
                let model = raw
                    .iter()
                    .skip(1)
                    .find(|v| !v.starts_with("--") && !v.contains('='))
                    .cloned()
                    .unwrap_or_default();
                if model.is_empty() {
                    usage("mc sprocket --use <provider/id>   e.g. ollama/llama3.2:latest");
                }
                params["model"] = json!(model);
                let result = if let Some(addr) = a.bridge_addr() {
                    bridge_call(&addr, "sprocket.use", params)
                } else {
                    let mut s = Session::user_session();
                    if let Some(project) = params["project"].as_str() {
                        s.execute("project.attach", json!({"path":project}))
                            .unwrap_or_else(|e| fail(format!("sprocket.use: {e}")));
                    }
                    dispatch(&mut s, "sprocket.use", params)
                };
                let result = result.unwrap_or_else(|e| fail(e));
                outln!(
                    "Sprocket now uses {}/{} ({}).",
                    result["selection"]["provider"].as_str().unwrap_or("?"),
                    result["selection"]["model"].as_str().unwrap_or("?"),
                    if result["local"] == true {
                        "local"
                    } else if result["free"] == true {
                        "free"
                    } else {
                        "paid"
                    }
                );
                std::process::exit(0);
            }
            if !read_only {
                if message.is_empty() {
                    usage(
                        "mc sprocket <message...>   talk to Sprocket, the app's own agent\n       mc sprocket --status    show the Sprocket session\n       mc sprocket --log       show the Sprocket transcript\n       mc sprocket --continue  keep the current Sprocket chat instead of starting a new one",
                    );
                }
                params["message"] = json!(message);
                if wants("--continue") {
                    params["new_chat"] = json!(false);
                }
            } else {
                let id = if wants("--log") {
                    "sprocket.log"
                } else {
                    "sprocket.status"
                };
                if wants("--log") {
                    params["refresh"] = json!(true);
                }
                let result = if let Some(addr) = a.bridge_addr() {
                    bridge_call(&addr, id, params)
                } else {
                    let mut s = Session::user_session();
                    if let Some(project) = params["project"].as_str() {
                        s.execute("project.attach", json!({"path":project}))
                            .unwrap_or_else(|e| fail(format!("{id}: {e}")));
                    }
                    dispatch(&mut s, id, params)
                };
                outln!(
                    "{}",
                    serde_json::to_string_pretty(&result.unwrap_or_else(|e| fail(e)))
                        .unwrap_or_default()
                );
                std::process::exit(0);
            }
            let result = if let Some(addr) = a.bridge_addr() {
                bridge_call(&addr, "sprocket.send", params)
            } else {
                let mut s = Session::user_session();
                if let Some(project) = params["project"].as_str() {
                    s.execute("project.attach", json!({"path":project}))
                        .unwrap_or_else(|e| fail(format!("sprocket.send: {e}")));
                }
                dispatch(&mut s, "sprocket.send", params)
            };
            let result = result.unwrap_or_else(|e| fail(e));
            if let Some(repaired) = result["sprocket"]["selection_repaired"].as_object() {
                outln!(
                    "Sprocket: selection moved from {}/{} to {}/{} (the old model is no longer enabled).",
                    repaired["from"]["provider"].as_str().unwrap_or("?"),
                    repaired["from"]["model"].as_str().unwrap_or("?"),
                    repaired["to"]["provider"].as_str().unwrap_or("?"),
                    repaired["to"]["model"].as_str().unwrap_or("?")
                );
            }
            let reply = result["text"]
                .as_str()
                .unwrap_or_default()
                .trim()
                .to_string();
            if reply.is_empty() {
                outln!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
                std::process::exit(0);
            }
            outln!("{reply}");
            if result["status"].as_str() == Some("running") {
                outln!("(still running — mc sprocket --log for the reply)");
            }
        }
        "workshop" | "backstage" | "booth" | "center" => {
            if a.bridge_addr().is_some() {
                usage(
                    "Backstage currently uses an explicit headless project; --bridge is reserved for existing live command access",
                );
            }
            if let Err(e) = center::start(
                a.params_from(1).unwrap_or_else(|e| usage(e)),
                cli_args().iter().cloned().any(|a| a == "--print"),
            ) {
                fail(e);
            }
        }
        "info" => {
            let specs = magicstudio_engine::command_specs();
            outln!("Magic Studio engine v{}", env!("CARGO_PKG_VERSION"));
            outln!("Registered commands: {}", specs.len());
            for spec in specs {
                outln!("  {} — {}", spec.id, spec.label);
            }
        }
        "panels" => {
            let raw = cli_args();
            std::process::exit(panels::run(
                a.bridge_addr().as_deref(),
                raw.iter().any(|v| v == "--json"),
                raw.iter().any(|v| v == "--check"),
            ));
        }
        "studios" | "windows" => {
            // skip the program and this subcommand, so the first free argument is the studio to pin
            let raw = cli_args().iter().cloned().skip(2).collect::<Vec<_>>();
            let target = raw
                .iter()
                .find(|v| !v.starts_with("--") && !v.contains('='))
                .map(String::as_str);
            let code = if raw.iter().any(|v| v == "--check") {
                studios::verify()
            } else {
                studios::run(
                    raw.iter().any(|v| v == "--json"),
                    target,
                    raw.iter().any(|v| v == "--unpin"),
                )
            };
            std::process::exit(code);
        }
        "commands" => {
            let f = a.pos(1).map(str::to_ascii_lowercase).unwrap_or_default();
            let grouped = cli_args().iter().any(|arg| arg == "--group");
            let specs = magicstudio_engine::command_specs();
            let hit = |spec: &&magicstudio_engine::CommandSpec| {
                let id = spec.id.to_ascii_lowercase();
                let label = spec.label.to_ascii_lowercase();
                f.is_empty() || id.contains(&f) || label.contains(&f)
            };
            let print = |spec: &magicstudio_engine::CommandSpec| {
                outln!(
                    "{:<36} {:<24} {:<14} {}",
                    spec.id,
                    spec.label,
                    spec.shortcut.unwrap_or("-"),
                    spec.params
                );
            };
            let selected: Vec<&magicstudio_engine::CommandSpec> =
                specs.iter().filter(hit).collect();
            if grouped {
                for (index, (name, list)) in grouped_commands(selected).into_iter().enumerate() {
                    if index > 0 {
                        outln!();
                    }
                    outln!("{} ({})", name, list.len());
                    for spec in list {
                        print(spec);
                    }
                }
            } else {
                for spec in selected {
                    print(spec);
                }
            }
        }
        "find" => {
            let query = (1..)
                .map_while(|index| a.pos(index))
                .filter(|part| !part.contains('='))
                .collect::<Vec<_>>()
                .join(" ");
            let params = a.params_from(1).unwrap_or_else(|error| usage(error));
            let report = find::report(
                &query,
                params["project"].as_str(),
                a.bridge_addr().as_deref(),
            );
            if cli_args().iter().any(|arg| arg == "--json") {
                outln!(
                    "{}",
                    serde_json::to_string_pretty(&report).unwrap_or_default()
                );
            } else {
                outln!("{}", find::display(&report));
            }
        }
        "vision" => {
            let target = a.pos(1);
            let res = match (a.bridge_addr(), target) {
                // Character metrics on a whole-window screenshot are meaningless; ask for a picture instead.
                (Some(_), None) => usage(
                    "vision needs an image or character; for a picture of the window use `exec ui.look` or `exec ui.screenshot`",
                ),
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
                        s.execute("vision.analyze", params)
                            .map_err(|e| e.to_string())
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
            if cli_args().iter().cloned().any(|v| v == "--doctor") {
                if a.pos(1).is_some_and(|v| !v.contains('=')) {
                    usage("storyboard --doctor (select the project with --bridge <port>)");
                }
                let result = bridge_call(
                    a.bridge_addr().as_deref().unwrap_or("auto"),
                    "storyboard.doctor",
                    a.params_from(1)
                        .map(|mut p| {
                            p["_origin"] = json!("CLI");
                            p
                        })
                        .unwrap_or_else(|e| usage(e)),
                )
                .unwrap_or_else(|e| fail(e));
                outln!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
                std::process::exit(if result["healthy"] == true {
                    0
                } else if result["healthy"] == false {
                    1
                } else {
                    2
                });
            }
            usage("storyboard create|read|update|delete [key=value ...], or storyboard --doctor");
        }
        "clips" => {
            let mut params = a.params_from(1).unwrap_or_else(|e| usage(e));
            params["_origin"] = json!("CLI");
            let bridge = a.bridge_addr();
            let mut s = Session::user_session();
            // Headless reads need the project attached; dispatch then follows a Studio that has it open.
            if bridge.is_none() {
                if let Some(project) = params["project"].as_str() {
                    s.execute("project.attach", json!({"path": project}))
                        .unwrap_or_else(|e| fail(format!("clips: {e}")));
                }
            }
            let mut run = |id: &str| match &bridge {
                Some(addr) => bridge_call(addr, id, params.clone()),
                None => dispatch(&mut s, id, params.clone()),
            };
            if cli_args().iter().cloned().any(|v| v == "--convert") {
                run("motion.clips.convert")
                    .unwrap_or_else(|e| fail(format!("motion.clips.convert: {e}")));
            }
            let v = run("motion.clips").unwrap_or_else(|e| fail(format!("motion.clips: {e}")));
            outln!(
                "{:<30} {:<36} {:>8}  {:<10}  PARTS",
                "CLIP",
                "SHEET",
                "SECONDS",
                "STATUS"
            );
            for c in v["clips"].as_array().into_iter().flatten() {
                let seconds = c["duration"]
                    .as_f64()
                    .map(|d| format!("{d:.2}"))
                    .unwrap_or_else(|| "-".into());
                let parts = c["keyframed_parts"]
                    .as_array()
                    .map(|a| {
                        let names: Vec<&str> = a.iter().filter_map(Value::as_str).collect();
                        if names.is_empty() {
                            "-".to_string()
                        } else {
                            format!("{} ({})", names.len(), names.join(", "))
                        }
                    })
                    .unwrap_or_else(|| "-".into());
                outln!(
                    "{:<30} {:<36} {:>8}  {:<10}  {}",
                    c["clip"].as_str().unwrap_or("?"),
                    c["sheet"].as_str().unwrap_or("?"),
                    seconds,
                    c["status"].as_str().unwrap_or("?"),
                    parts
                );
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
            params["_origin"] = json!("CLI");
            // Opening names its target with path=; pinning the current project would refuse the switch.
            if matches!(id, "project.open" | "project.identity")
                && params["path"].is_null()
                && params["project"].is_string()
            {
                params["path"] = params["project"].clone();
                params.as_object_mut().map(|p| {
                    p.remove("project");
                    p.remove("_expected_project")
                });
            }
            if let Some(v) = center_observation(id, &params, a.bridge_addr().as_deref()) {
                outln!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
                if v["ok"] == false {
                    std::process::exit(1);
                }
                return;
            }
            let res = if id == "ui.read" {
                ui_read(a.bridge_addr().as_deref().unwrap_or("auto"), params)
            } else if matches!(
                id,
                "ui.feedback" | "ui.components" | "ui.controls" | "ui.attention" | "app.quit"
            ) {
                bridge_call(a.bridge_addr().as_deref().unwrap_or("auto"), id, params)
            } else if let Some(addr) = a.bridge_addr() {
                bridge_call(&addr, id, params)
            } else {
                let mut s = Session::user_session();
                // A named project is attached first, so dispatch reaches the Studio that has it open.
                if let Some(project) = params["project"].as_str() {
                    s.execute("project.attach", json!({"path": project}))
                        .unwrap_or_else(|e| fail(format!("{id}: {e}")));
                }
                dispatch(&mut s, id, params)
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
                params["_origin"] = json!("CLI JSONL");
                let res = if matches!(
                    id,
                    "ui.feedback" | "ui.components" | "ui.controls" | "ui.attention" | "app.quit"
                ) {
                    bridge_call(bridge.as_deref().unwrap_or("auto"), id, params)
                } else if let Some(addr) = &bridge {
                    bridge_call(addr, id, params)
                } else {
                    dispatch(&mut s, id, params)
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
            if let Err(e) =
                agent_command(a.pos(1).unwrap_or(""), a.pos(2), &params, a.bridge_addr())
            {
                fail(e);
            }
        }
        "doctor" | "debug" => {
            let mut params = a.params_from(1).unwrap_or_else(|e| usage(e));
            if a.flag_clean {
                if let Some(obj) = params.as_object_mut() {
                    obj.insert("clean".to_string(), Value::Bool(true));
                }
            }
            if a.flag_health {
                // Comprehensive health view: all magicstudio apps, MCP connections, TUI instances
                let (rows, errors) = studios::rows();
                let tui_instances = tui::call("instances", &json!({})).ok();
                let mcp_procs: Vec<Value> = std::process::Command::new("pgrep")
                    .args(["-af", "mc.*mcp"])
                    .output()
                    .ok()
                    .and_then(|o| String::from_utf8(o.stdout).ok())
                    .map(|s| s.lines().map(|l| json!(l)).collect())
                    .unwrap_or_default();
                let health = json!({
                    "ok": true,
                    "studios": rows,
                    "studio_errors": errors,
                    "tui_instances": tui_instances,
                    "mcp_bridge_count": mcp_procs.len(),
                    "mcp_bridges": mcp_procs,
                    "guidance": "Use --stop to kill all MCP bridges. Use --clean to remove stale endpoints."
                });
                outln!(
                    "{}",
                    serde_json::to_string_pretty(&health).unwrap_or_default()
                );
                return;
            }
            if a.flag_github {
                // GITHUB HEALTH: what an agent did to the repository's GitHub state.
                let repo = github_health::repo_root(&std::env::current_dir().unwrap_or_default());
                let health = github_health::inspect(&repo);
                outln!(
                    "{}",
                    serde_json::to_string_pretty(&health).unwrap_or_default()
                );
                return;
            }
            if a.flag_report {
                // MAYHEM report: a focused summary plus the expanded lists behind it,
                // and the measured trend since the first recorded sample.
                let servers = mcp_watch::scan();
                let (by, findings) = mcp_watch::analyze(&servers);
                let (rows, errors) = studios::rows();
                let sample = mayhem::Sample::capture(&servers, rows.len(), errors.len());
                mayhem::record(&sample);
                let mut families: Vec<_> = by.iter().collect();
                families.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));
                let hosts: std::collections::BTreeSet<u32> =
                    servers.iter().map(|s| s.host_pid).collect();
                let report = json!({
                    "ok": true,
                    "summary": {
                        "mcp_servers": servers.len(),
                        "mcp_agents": hosts.len(),
                        "mcp_rss_kb": servers.iter().map(|s| s.rss_kb).sum::<u64>(),
                        "collisions": findings.len(),
                        "studios": rows.len(),
                        "stale_studios": errors.len(),
                        "host_mem_used_kb": sample.host_mem_used_kb,
                        "host_mem_total_kb": sample.host_mem_total_kb,
                    },
                    "trend": mayhem::trend().map(|t| json!({
                        "samples": t.samples,
                        "span_seconds": t.span_ms / 1000,
                        "mcp_servers": mayhem::delta_text(t.servers_delta, "servers"),
                        "mcp_agents": mayhem::delta_text(t.agents_delta, "agents"),
                        "mcp_memory": mayhem::delta_text(t.rss_delta_kb, "MiB"),
                        "collisions": mayhem::delta_text(t.collisions_delta, "collisions"),
                        "stale_studios": mayhem::delta_text(t.stale_delta, "stale"),
                        "host_memory": mayhem::delta_text(t.host_mem_delta_kb, "MiB"),
                        "improving": mayhem::improved(t.servers_delta) || mayhem::improved(t.rss_delta_kb),
                    })),
                    "findings": findings.iter().map(|f| json!({"family": f.family, "problem": f.text})).collect::<Vec<_>>(),
                    "families": families.iter().map(|(family, group)| {
                        let mut who: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::<&str, usize>::new();
                        for s in group.iter() { *who.entry(s.host.as_str()).or_default() += 1; }
                        json!({
                            "family": family,
                            "count": group.len(),
                            "agents": who,
                            "rss_kb": group.iter().map(|s| s.rss_kb).sum::<u64>(),
                            "servers": group.iter().map(|s| json!({"pid": s.pid, "host": s.host, "age_seconds": s.age_secs, "rss_kb": s.rss_kb, "write_root": s.write_root})).collect::<Vec<_>>(),
                        })
                    }).collect::<Vec<_>>(),
                    "studios": rows,
                    "stale_studios": errors,
                    "next_step": if findings.is_empty() {
                        "No collisions. Keep the sample cadence running; the trend shows whether the footprint shrinks."
                    } else {
                        "Close the colliding agents for the families listed, then run this report again: fewer servers and less memory is the goal."
                    },
                });
                outln!(
                    "{}",
                    serde_json::to_string_pretty(&report).unwrap_or_default()
                );
                return;
            }
            if a.flag_stop {
                // Stop all MCP bridge processes
                let output = std::process::Command::new("pkill")
                    .args(["-f", "mc.*mcp"])
                    .output();
                let killed = output.as_ref().map(|o| o.status.success()).unwrap_or(false);
                let remaining = std::process::Command::new("pgrep")
                    .args(["-c", "-f", "mc.*mcp"])
                    .output()
                    .ok()
                    .and_then(|o| String::from_utf8(o.stdout).ok())
                    .and_then(|s| s.trim().parse::<u64>().ok())
                    .unwrap_or(0);
                outln!("{}", serde_json::to_string_pretty(&json!({
                    "ok": true,
                    "stopped": killed,
                    "remaining_mcp_bridges": remaining,
                    "message": if killed { "MCP bridges stopped" } else { "No MCP bridges found or kill failed" }
                })).unwrap_or_default());
                return;
            }
            let mut s = Session::user_session();
            match s.execute("app.doctor", params) {
                Ok(mut v) => {
                    // Same question an agent asks its owner; answer it here too.
                    let (rows, errors) = studios::rows();
                    v["studios"] = serde_json::json!(rows);
                    v["studio_errors"] = serde_json::json!(errors);
                    v["studio_guidance"] = serde_json::json!(if rows.is_empty() {
                        "no Studio is running: start one, or read the project with mc backstage --print project=<file>"
                    } else {
                        "address one with --bridge <endpoint>; mc studios lists them and can pin one"
                    });
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

/// Group command specs by their first menu entry. Commands without a menu are
/// read-only queries and are kept last so the actionable groups lead.
fn grouped_commands<'a>(
    specs: Vec<&'a magicstudio_engine::CommandSpec>,
) -> Vec<(String, Vec<&'a magicstudio_engine::CommandSpec>)> {
    let mut groups: Vec<(String, Vec<&'a magicstudio_engine::CommandSpec>)> = Vec::new();
    for spec in specs {
        let name = spec
            .menu
            .first()
            .copied()
            .unwrap_or("Read-only")
            .to_string();
        match groups
            .iter_mut()
            .find(|(group, _)| group.as_str() == name.as_str())
        {
            Some((_, list)) => list.push(spec),
            None => groups.push((name, vec![spec])),
        }
    }
    groups.sort_by(|a, b| {
        let (a_read, b_read) = (a.0 == "Read-only", b.0 == "Read-only");
        a_read.cmp(&b_read).then_with(|| a.0.cmp(&b.0))
    });
    groups
}
