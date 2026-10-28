//! Ask the configured system agent for a character's asset sheets.
//!
//! The format comes from `.agents/skills/asset-sheets/SKILL.md`, so every agent draws the same
//! turnaround, skeleton-and-limbs and pose sheets. The agent is the one the app is configured
//! with (the Omarchy system agent by default), run headlessly in the project folder.
//!
//! Two things this command has to get right, because the agent cannot guess them:
//!
//! * **The drawing tool.** The image tool in the agent's environment returns its own canvas,
//!   not the size it is asked for, so a sheet asked for as "2048x2048" comes back at another
//!   size. The agent has to draw the artwork and report the size; this command normalises it
//!   to the contract's canvas with `image.image_size`.
//! * **The session.** `magic-cli exec` runs one command in a throwaway session, so the
//!   project's own commands have to run through one `magic-cli run` script, not one
//!   `exec` call per step.
//!
//! After the agent runs, every sheet is checked against the contract, and the command either
//! reports sheets that pass or says exactly what is wrong with each one that does not.

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::workspace::workspace_root;
use crate::{EngineError, Result, Session};

const KINDS: [&str; 3] = ["turnaround", "skeleton", "poses"];
/// The canvas every asset sheet must have (see `.agents/skills/asset-sheets/SKILL.md`).
const SHEET_CANVAS: (u32, u32) = (2048, 2048);
/// How long the agent may take before it is stopped and the sheets are checked anyway.
const DEFAULT_TIMEOUT_S: u64 = 1800;
/// Pixels within this distance of pure white still count as the flat `#ffffff` background.
const WHITE_TOLERANCE: u8 = 12;

fn which(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).any(|dir| dir.join(name).is_file()))
        .unwrap_or(false)
}

/// The agents this command can drive non-interactively, and the ones that are installed.
fn available_agents() -> Vec<&'static str> {
    ["codex", "claude", "omp"]
        .into_iter()
        .filter(|name| which(name))
        .collect()
}

/// The agent the app is configured with (`~/.config/magicstudio/agent.json`), else the
/// Omarchy system default, else codex.
fn configured_agent() -> String {
    let settings = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
        .map(|base| base.join("magicstudio").join("agent.json"));
    if let Some(path) = settings
        && let Ok(text) = std::fs::read_to_string(path)
        && let Ok(value) = serde_json::from_str::<Value>(&text)
        && let Some(agent) = value["agent"].as_str().filter(|a| !a.trim().is_empty())
    {
        return agent.to_string();
    }
    crate::agent::default_agent().unwrap_or_else(|| "codex".to_string())
}

/// The last `chars` of a stream, with the tail marker that makes a truncation visible.
fn tail(text: &str, chars: usize) -> String {
    let trimmed = text.trim();
    let out: String = trimmed
        .chars()
        .rev()
        .take(chars)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if trimmed.chars().count() > chars {
        format!("…{out}")
    } else {
        out
    }
}

/// What one agent invocation produced.
pub(crate) struct AgentRun {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) code: Option<i32>,
    pub(crate) timed_out: bool,
    pub(crate) stopped_after_image: bool,
}

impl AgentRun {
    pub(crate) fn succeeded(&self) -> bool {
        !self.timed_out && self.code == Some(0)
    }

    /// The part of the agent's own report worth showing when it failed.
    pub(crate) fn failure_detail(&self) -> String {
        if !self.stderr.trim().is_empty() {
            tail(&self.stderr, 800)
        } else {
            tail(&self.stdout, 800)
        }
    }
}

/// Run the agent with a hard timeout, capturing both streams. A long run must not freeze the
/// app, so on timeout the agent is stopped and whatever it wrote is still checked.
pub(crate) fn run_agent(program: &str, args: &[String], cwd: &Path, timeout_s: u64) -> Result<AgentRun> {
    run_agent_inner(program,args,cwd,timeout_s,false)
}

/// Image bytes are already staged when the native success receipt arrives. Stop this one-shot
/// agent before it can retry generation while trying to reconstruct/copy binary tool output.
pub(crate) fn run_image_agent(program: &str, args: &[String], cwd: &Path, timeout_s: u64) -> Result<AgentRun> {
    run_agent_inner(program,args,cwd,timeout_s,true)
}

fn run_agent_inner(program: &str, args: &[String], cwd: &Path, timeout_s: u64, stop_on_image:bool) -> Result<AgentRun> {
    let mut command = std::process::Command::new(program);
    // `codex exec` defaults to error-only stderr. The native image importer needs
    // its INFO tool receipt to correlate staged bytes; never rely on the parent's log level.
    if stop_on_image { command.env("RUST_LOG", "error,codex_otel=info"); }
    let mut child = command.current_dir(cwd)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| EngineError::ExternalTool(program.to_string(), e.to_string()))?;
    let (tx_out, rx_out) = std::sync::mpsc::channel::<Vec<u8>>();
    let (tx_err, rx_err) = std::sync::mpsc::channel::<Vec<u8>>();
    let (image_tx,image_rx)=std::sync::mpsc::channel::<()>();
    if let Some(mut pipe) = child.stdout.take() {
        let tx = tx_out.clone();
        std::thread::spawn(move || {
            use std::io::Read;
            let mut buf = Vec::new();
            let _ = pipe.read_to_end(&mut buf);
            let _ = tx.send(buf);
        });
    }
    if let Some(mut pipe) = child.stderr.take() {
        let tx = tx_err.clone();
        std::thread::spawn(move || {
            use std::io::{BufRead,BufReader};
            let mut reader=BufReader::new(&mut pipe);
            let mut buf=Vec::new();let mut line=Vec::new();let mut signalled=false;
            loop {
                line.clear();
                match reader.read_until(b'\n',&mut line){Ok(0)|Err(_)=>break,Ok(_)=>{}}
                buf.extend_from_slice(&line);
                if stop_on_image&&!signalled&&crate::image_generation::native_image_success(&String::from_utf8_lossy(&line)) {
                    signalled=true;let _=image_tx.send(());
                }
            }
            let _ = tx.send(buf);
        });
    }
    let deadline = Instant::now() + Duration::from_secs(timeout_s.max(1));
    let mut timed_out = false;
    let mut stopped_after_image=false;
    let code = loop {
        if stop_on_image&&image_rx.try_recv().is_ok(){
            stopped_after_image=true;let _=child.kill();
            break child.wait().ok().and_then(|status|status.code());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) => {}
            Err(_) => break None,
        }
        if Instant::now() >= deadline {
            timed_out = true;
            let _ = child.kill();
            break child.wait().ok().and_then(|status| status.code());
        }
        std::thread::sleep(Duration::from_millis(150));
    };
    let stdout = rx_out.recv_timeout(Duration::from_secs(15)).unwrap_or_default();
    let stderr = rx_err.recv_timeout(Duration::from_secs(15)).unwrap_or_default();
    Ok(AgentRun {
        stdout: String::from_utf8_lossy(&stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&stderr).trim().to_string(),
        code,
        timed_out,
        stopped_after_image,
    })
}

/// The file name the contract gives a sheet.
fn sheet_file(character: &str, kind: &str) -> String {
    format!("{character}-{kind}.png")
}

/// The file the contract delivers: the agent's drawing, normalised to the contract canvas.
fn normalised_file(character: &str, kind: &str) -> String {
    format!("{character}-{kind}-2048.png")
}

/// One sheet against the contract: the agent's drawing, the normalised sheet, and whether the
/// normalised sheet has the contract's canvas, a flat white background and no drawing on the
/// edge.
fn sheet_report(kind: &str, raw: &Path, normalised: &Path) -> Value {
    let mut reasons: Vec<String> = Vec::new();
    let raw_file = raw.to_string_lossy().to_string();
    let raw_dims = image::image_dimensions(raw).ok();
    if !raw.is_file() {
        reasons.push("the agent wrote no sheet at this path".to_string());
    } else if raw_dims.is_none() {
        reasons.push("the sheet the agent wrote is not a readable image".to_string());
    }
    // The sheet that counts is the normalised one when it exists; a drawing that already came
    // back at the contract size is the sheet itself.
    let raw_is_contract = raw_dims == Some(SHEET_CANVAS);
    let effective = if normalised.is_file() || !raw_is_contract {
        normalised
    } else {
        raw
    };
    let file = effective.to_string_lossy().to_string();
    if !effective.is_file() {
        reasons.push(format!(
            "no sheet at the contract size: {file} is missing — normalise the drawing with \
             `image.image_size source={raw_file} out={file} width={w} height={h}`",
            w = SHEET_CANVAS.0,
            h = SHEET_CANVAS.1
        ));
        return json!({
            "kind": kind,
            "raw": {"file": raw_file, "exists": raw.is_file(), "width": raw_dims.map(|d| d.0).unwrap_or(0), "height": raw_dims.map(|d| d.1).unwrap_or(0)},
            "file": file,
            "normalised": normalised.to_string_lossy(),
            "exists": false,
            "ok": false,
            "reasons": reasons,
        });
    }
    let dims = image::image_dimensions(effective).ok();
    match dims {
        None => reasons.push("the sheet is not a readable image".to_string()),
        Some((w, h)) if (w, h) != SHEET_CANVAS => reasons.push(format!(
            "{w}x{h} px, but the contract needs {}x{} px: normalise it with `image.image_size` \
             (the image tool returns its own size)",
            SHEET_CANVAS.0, SHEET_CANVAS.1
        )),
        Some(_) => {}
    }
    if let Some(img) = image::open(effective).ok().map(|im| im.to_rgba8()) {
        let (w, h) = (img.width(), img.height());
        let low = WHITE_TOLERANCE;
        let mut transparent = 0usize;
        for px in img.pixels() {
            if px.0[3] < 250 {
                transparent += 1;
            }
        }
        if transparent > 0 {
            reasons.push(format!(
                "{transparent} pixels are not opaque: the contract wants a flat #ffffff background"
            ));
        }
        let mut edge_not_white = 0usize;
        let mut edge = Vec::new();
        for x in 0..w {
            edge.push((x, 0));
            edge.push((x, 1.min(h.saturating_sub(1))));
            edge.push((x, h.saturating_sub(1)));
            edge.push((x, h.saturating_sub(2)));
        }
        for y in 0..h {
            edge.push((0, y));
            edge.push((1.min(w.saturating_sub(1)), y));
            edge.push((w.saturating_sub(1), y));
            edge.push((w.saturating_sub(2), y));
        }
        for (x, y) in edge {
            if x >= w || y >= h {
                continue;
            }
            let px = img.get_pixel(x, y);
            if px.0[0] < 255 - low || px.0[1] < 255 - low || px.0[2] < 255 - low {
                edge_not_white += 1;
            }
        }
        if edge_not_white > 0 {
            reasons.push(format!(
                "{edge_not_white} pixels on the canvas edge are not flat white: the background must \
                 be #ffffff and no drawing may touch the edge"
            ));
        }
    }
    let ok = reasons.is_empty();
    json!({
        "kind": kind,
        "raw": {
            "file": raw_file,
            "exists": raw.is_file(),
            "width": raw_dims.map(|d| d.0).unwrap_or(0),
            "height": raw_dims.map(|d| d.1).unwrap_or(0),
        },
        "file": file,
        "normalised": normalised.to_string_lossy(),
        "exists": true,
        "width": dims.map(|d| d.0).unwrap_or(0),
        "height": dims.map(|d| d.1).unwrap_or(0),
        "canvas_ok": dims.map(|d| d == SHEET_CANVAS).unwrap_or(false),
        "ok": ok,
        "reasons": reasons,
    })
}

/// The instruction block that tells the agent how to draw in this environment.
fn draw_instructions(sheet_dir: &Path, character: &str, kinds: &[String]) -> String {
    let files: Vec<String> = kinds
        .iter()
        .map(|kind| sheet_file(character, kind))
        .collect();
    format!(
        "## How to draw in this environment\n\n\
Draw with the **built-in image tool** (`image_gen`, through the `imagegen` skill). Two servers \
that look like they could draw cannot be used here:\n\n\
- `revid` (MCP): no API key is configured, so it cannot authenticate.\n\
- `blender` (MCP): no Blender instance is running, so it cannot render.\n\n\
**The image tool returns its own canvas, not the size you ask for.** Asking it for \
\"2048x2048\" gives a square sheet of another size (observed 1254x1254). So ask for the \
largest square it can produce (>= 1024), report the size it returned, and let the app \
normalise it to {w}x{h} with `image.image_size` — never crop or compose it by hand.\n\n\
One image call per sheet: pass the reference image as a reference (`referenced_image_paths`), \
and put the contract's cell order, cell sizes and background rules in the prompt.\n\n\
Check what you saved before you report the sheet as done:\n\n\
```bash\npython3 -c \"from PIL import Image; print(Image.open('<path>').size)\"\n```\n\n\
Print the size it returned next to the sheet. The image tool saves under \
`$CODEX_HOME/generated_images/...`; move or copy the file into the project. A sheet that only \
exists under `$CODEX_HOME` has not been delivered.\n\n\
Save exactly these files, each with its exact prompt beside it:\n\n\
```text\n{sheets}\n```\n\n\
Keep the exact prompt of every image call in `<sheet>.prompt.md` beside its sheet.\n",
        w = SHEET_CANVAS.0,
        h = SHEET_CANVAS.1,
        sheets = files
            .iter()
            .map(|f| format!("{}/{f}", sheet_dir.display()))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// The instruction block that runs the project's own commands after the sheets exist.
fn chain_instructions(project_path: &Path, sheet_dir: &Path, character: &str) -> String {
    let sheet = sheet_dir.join(sheet_file(character, "turnaround"));
    format!(
        "## Then prepare the layers with Magic Studio's CLI\n\n\
Select the intended project once; subsequent commands retain its binding. \
The following JSONL runs the steps in order and stops on error:\n\n\
```bash\n\
cat > /tmp/asset-sheets-chain.jsonl <<'JSON'\n\
{{\"id\":\"project.open\",\"params\":{{\"path\":\"{project}\"}}}}\n\
{{\"id\":\"project.list_assets\",\"params\":{{}}}}\n\
{{\"id\":\"image.image_size\",\"params\":{{\"source\":\"{sheet}\",\"out\":\"{normal}\",\"width\":2048,\"height\":2048}}}}\n\
{{\"id\":\"intake.validate\",\"params\":{{\"image\":\"{normal}\",\"slot\":\"turnaround_sheet\",\"attach_to_active\":true}}}}\n\
{{\"id\":\"assets.assess\",\"params\":{{\"path\":\"{normal}\"}}}}\n\
{{\"id\":\"asset.cut_sheet\",\"params\":{{\"asset\":\"{character}\"}}}}\n\
{{\"id\":\"asset.build_cutout_rig\",\"params\":{{}}}}\n\
JSON\n\
magic-cli run /tmp/asset-sheets-chain.jsonl\n\
```\n\n\
Read every line of the reply. `assets.assess` decides clean-or-recreate: if it reports \
`recreate`, record the finding with `assets.fixlist_add` instead of forcing the cut. Keep the project context explicit; \
use installed tools directly; do not change app ownership: use each installed app's documented CLI directly when needed.\n\n\
Two things this chain can hit, so you do not have to rediscover them:\n\n\
- `asset.build_cutout_rig` writes to the repo's own `exports/pose_and_parts/<character>` \
folder. On a read-only checkout that step fails with `OSError: Read-only file system` — report \
it and stop; the sheets themselves are fine.\n\
- The cut expects five turnaround views and the contract draws four. When it asks for a \
`layout`, pass one and say which view is missing.\n",
        project = project_path.display(),
        sheet = sheet.display(),
        normal = sheet_dir.join(format!("{}-turnaround-2048.png", character)).display(),
        character = character,
    )
}

/// The chain the caller can run itself, as `magic-cli run` lines.
fn chain_script(project_path: &Path, sheet_dir: &Path, character: &str) -> Vec<Value> {
    let sheet = sheet_dir.join(sheet_file(character, "turnaround"));
    let normal = sheet_dir.join(format!("{}-turnaround-2048.png", character));
    vec![
        json!({"id": "project.open", "params": {"path": project_path.to_string_lossy()}}),
        json!({"id": "project.list_assets", "params": {}}),
        json!({"id": "image.image_size", "params": {"source": sheet.to_string_lossy(), "out": normal.to_string_lossy(), "width": 2048, "height": 2048}}),
        json!({"id": "intake.validate", "params": {"image": normal.to_string_lossy(), "slot": "turnaround_sheet", "attach_to_active": true}}),
        json!({"id": "assets.assess", "params": {"path": normal.to_string_lossy()}}),
        json!({"id": "asset.cut_sheet", "params": {"asset": character}}),
        json!({"id": "asset.build_cutout_rig", "params": {}}),
    ]
}

/// `assets.agent_sheets`: brief the configured agent, run it in the project, and check the
/// sheets it wrote against the contract. `run=false` only writes the brief; `check=true`
/// checks the sheets already on disk without running the agent.
pub fn cmd_assets_agent_sheets(s: &mut Session, params: &Value) -> Result<Value> {
    // `magic-cli exec` runs one command in a throwaway session, so this command is
    // usually reached with no project open. Say exactly how to reach it with one.
    let dir = crate::workshop::project_dir(s).map_err(|_| {
        EngineError::InvalidParams(
            "no project is open, so there is nowhere to write the sheets. Run it in one session: \
             `magic-cli run` with a script of two lines — \
             {\"id\":\"project.open\",\"params\":{\"path\":\"<project>.magic\"}} then \
             {\"id\":\"assets.agent_sheets\",\"params\":{\"character\":\"<id>\"}} — or open the \
             project in the app first."
                .into(),
        )
    })?;
    let character = params
        .get("character")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| s.current_asset().map(|a| a.id.clone()))
        .or_else(|| s.project_assets.first().map(|a| a.id.clone()))
        .ok_or_else(|| EngineError::InvalidParams("open a project with an asset first".into()))?;
    let reference = s
        .project_assets
        .iter()
        .find(|a| a.id == character)
        .and_then(|a| a.path.clone())
        .or_else(|| s.assets.get(&character).map(|a| a.path.clone()))
        .ok_or_else(|| EngineError::InvalidParams(format!("no artwork path for `{character}`")))?;

    let kinds: Vec<String> = match params.get("kinds") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .filter(|k| KINDS.contains(k))
            .map(str::to_owned)
            .collect(),
        Some(Value::String(one)) if KINDS.contains(&one.as_str()) => vec![one.clone()],
        _ => KINDS.iter().map(|k| (*k).to_string()).collect(),
    };
    if kinds.is_empty() {
        return Err(EngineError::InvalidParams(format!("kinds must be drawn from {KINDS:?}")));
    }

    let spec_path = workspace_root(s).join(".agents/skills/asset-sheets/SKILL.md");
    let spec = std::fs::read_to_string(&spec_path).map_err(|e| {
        EngineError::InvalidParams(format!("asset-sheet contract missing at {}: {e}", spec_path.display()))
    })?;

    let sheet_dir = dir.join("assets/sheets");
    let project_path = s
        .project_path
        .clone()
        .unwrap_or_else(|| dir.join(format!("{character}.magic")));

    let mut brief = format!(
        "# Asset sheets for `{character}`\n\nReference artwork: `{}`\n\nDraw these sheets, exactly as this contract says:\n\n{}",
        reference.display(),
        spec
    );
    brief.push_str(&format!(
        "\n\n## This request\n\nCharacter: `{character}`\nSheets: {}\nSave them under `{}` with the names the contract gives, plus each `.prompt.md` beside its sheet. Draw only these sheets; do not restyle the character.\n",
        kinds.join(", "),
        sheet_dir.display()
    ));
    brief.push_str(&draw_instructions(&sheet_dir, &character, &kinds));
    brief.push_str(&chain_instructions(&project_path, &sheet_dir, &character));
    if let Some(extra) = params.get("note").and_then(Value::as_str).filter(|n| !n.trim().is_empty()) {
        brief.push_str(&format!("\n## Note from the user\n\n{extra}\n"));
    }

    let briefs = dir.join("briefs");
    std::fs::create_dir_all(&briefs)?;
    let brief_path = briefs.join("AGENT-SHEETS.md");
    std::fs::write(&brief_path, &brief)?;

    let agent = params
        .get("agent")
        .and_then(Value::as_str)
        .filter(|a| !a.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(configured_agent);
    let run = params.get("run").and_then(Value::as_bool).unwrap_or(true);
    let check = params.get("check").and_then(Value::as_bool).unwrap_or(false);
    let timeout_s = params
        .get("timeout_s")
        .and_then(Value::as_u64)
        .filter(|s| *s > 0)
        .unwrap_or(DEFAULT_TIMEOUT_S);

    if !run && !check {
        return Ok(json!({"ok": true, "brief": brief_path, "agent": agent, "ran": false, "sheets": []}));
    }

    let mut error: Option<String> = None;
    let mut agent_run: Option<AgentRun> = None;
    if run {
        if !which(&agent) {
            let available = available_agents();
            return Err(EngineError::InvalidParams(format!(
                "agent `{agent}` is not installed; installed agents: {available:?}. Pass agent=<name> to choose another, or run=false to only write the brief."
            )));
        }
        let prompt = format!("Follow this brief exactly and draw the sheets it asks for:\n\n{brief}");
        let args: Vec<String> = match agent.as_str() {
            "codex" => vec![
                "exec".into(),
                prompt,
                "--skip-git-repo-check".into(),
                // The agent must write the sheets, then the prepared layers, inside the project.
                "-s".into(),
                "workspace-write".into(),
                // Nothing can answer an approval prompt in a non-interactive run: say no up
                // front, so a blocked command fails fast instead of hanging the command.
                "-c".into(),
                "approval_policy=\"never\"".into(),
                "-C".into(),
                dir.to_string_lossy().into_owned(),
            ],
            "claude" => vec!["-p".into(), prompt, "--permission-mode".into(), "auto".into()],
            "omp" => vec!["-p".into(), prompt, "--cwd".into(), dir.to_string_lossy().into_owned()],
            other => {
                return Err(EngineError::InvalidParams(format!(
                    "the app can only run omp, claude or codex non-interactively (asked for `{other}`)"
                )));
            }
        };
        let run_result = run_agent(&agent, &args, &dir, timeout_s)?;
        if run_result.timed_out {
            error = Some(format!(
                "the agent `{agent}` did not finish within {timeout_s}s and was stopped; the sheets \
                 it wrote before that are checked below"
            ));
        } else if !run_result.succeeded() {
            error = Some(format!(
                "the agent `{agent}` exited with code {:?}: {}",
                run_result.code,
                run_result.failure_detail()
            ));
        }
        agent_run = Some(run_result);
    }

    // Normalise every sheet the agent drew to the contract's canvas, with the app's own
    // command — the image tool returns its own size and the contract forbids cropping by hand.
    let mut normalise_errors: Vec<String> = Vec::new();
    if run {
        for kind in &kinds {
            let raw = sheet_dir.join(sheet_file(&character, kind));
            let normalised = sheet_dir.join(normalised_file(&character, kind));
            if !raw.is_file() {
                continue;
            }
            if normalised.is_file() && image::image_dimensions(&raw).ok() == Some(SHEET_CANVAS) {
                // The drawing already came back at the contract size and the normalised copy
                // is there: leave both files alone.
                continue;
            }
            let params = json!({
                "source": raw.to_string_lossy(),
                "out": normalised.to_string_lossy(),
                "width": SHEET_CANVAS.0,
                "height": SHEET_CANVAS.1,
                "anchor": "center",
                "extensionColor": "#ffffff",
            });
            if let Err(e) = crate::image_cmds::cmd_image_image_size(s, &params) {
                normalise_errors.push(format!("{}: {e}", sheet_file(&character, kind)));
            }
        }
    }

    // Check every requested sheet against the contract, whatever the agent did.
    let mut sheets = Vec::new();
    let mut problems: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut wrong_size: Vec<String> = Vec::new();
    for kind in &kinds {
        let raw = sheet_dir.join(sheet_file(&character, kind));
        let normalised = sheet_dir.join(normalised_file(&character, kind));
        let report = sheet_report(kind, &raw, &normalised);
        if !report["ok"].as_bool().unwrap_or(false) {
            let reasons: Vec<String> = report["reasons"]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|r| r.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            problems.push(format!(
                "{}: {}",
                sheet_file(&character, kind),
                reasons.join("; ")
            ));
            if !raw.is_file() {
                missing.push(sheet_file(&character, kind));
            } else {
                wrong_size.push(sheet_file(&character, kind));
            }
        }
        sheets.push(report);
    }
    for detail in &normalise_errors {
        problems.push(format!("normalising the sheet failed: {detail}"));
    }
    let sheets_ok = problems.is_empty();

    let mut next: Vec<String> = Vec::new();
    if let Some(detail) = &error {
        next.push(detail.clone());
    }
    if !sheets_ok {
        next.push(format!("Fix the sheet(s) and re-run: {}", problems.join(" | ")));
        if !missing.is_empty() {
            next.push(format!(
                "The agent drew no sheet for {}. Re-run the command to draw them again, with a \
                 note that says what to correct.",
                missing.join(", ")
            ));
        }
        if !wrong_size.is_empty() {
            next.push(format!(
                "{} {} at the wrong size. Re-run the command — it normalises what the agent drew \
                 with `image.image_size` — or run that command by hand.",
                wrong_size.join(", "),
                if wrong_size.len() == 1 { "is" } else { "are" }
            ));
        }
    } else {
        next.push(
            "Write `chain` to a .jsonl file and run `magic-cli run <file>`: `exec` starts \
             a throwaway session, so the project's own commands need one session to share."
                .to_string(),
        );
        next.push(
            "assets.assess path=<sheet> decides clean-or-recreate; on `recreate` use \
             assets.fixlist_add instead of forcing the cut."
                .to_string(),
        );
    }

    Ok(json!({
        "ok": sheets_ok,
        "command": "assets.agent_sheets",
        "agent": agent,
        "character": character,
        "brief": brief_path,
        "sheets": sheets,
        "problems": problems,
        "chain": chain_script(&project_path, &sheet_dir, &character),
        "agent_error": error,
        "output_tail": agent_run.as_ref().map(|r| tail(&r.stdout, 600)).unwrap_or_default(),
        "agent_log_tail": agent_run.as_ref().map(|r| tail(&r.stderr, 1200)).unwrap_or_default(),
        "next": next,
    }))
}
