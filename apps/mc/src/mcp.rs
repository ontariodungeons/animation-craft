//! Optional stdio MCP adapter and Legacy UI OCR helpers, isolated from main CLI dispatcher.
use super::*;

/// Typed MCP input schemas for commands whose parameters are fixed; the rest keep a bare object
/// and publish their parameter shape in the tool description.
fn tool_input_schema(id: &str) -> Value {
    let mut schema = command_input_schema(id);
    if schema["properties"].is_object() {
        schema["properties"]["_client"] = json!({"type":"string","description":"Optional supplied agent/client label; not authentication"});
        schema["properties"]["_request_id"] = json!({"type":"string","description":"Optional request/correlation ID for action receipts"});
    }
    schema
}

fn command_input_schema(id: &str) -> Value {
    let asset =
        json!({ "type": "string", "description": "Loaded asset id; defaults to the active asset" });
    let source = json!({ "type": "string", "description": "Source image path; defaults to the asset's image" });
    let out = json!({ "type": "string", "description": "Output PNG path" });
    let unit = json!({ "type": "number", "minimum": 0, "maximum": 1 });
    let point =
        json!({ "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 });
    match id {
        "project.root" => {
            json!({"type":"object","properties":{"root":{"type":"string","description":"Optional absolute production root to persist; omit to inspect. Intake root= overrides never change it."}},"additionalProperties":false})
        }
        "agent.session" => json!({"type":"object","properties":{},"additionalProperties":false}),
        "agent.config" => json!({
            "type":"object",
            "properties":{
                "selection":{
                    "type":"object",
                    "description":"Optional per-session chat selection; credentials and shared defaults unchanged",
                    "properties":{
                        "agent":{"type":"string","enum":["opencode","codex","omp","claude","agy","hermes","grok"]},
                        "provider":{"type":"string"},
                        "model":{"type":"string"},
                        "thinking":{"type":"string"},
                        "extra_args":{"type":"array","items":{"type":"string"}}
                    },
                    "additionalProperties":false
                }
            },
            "additionalProperties":false
        }),
        "agent.models" => {
            json!({"type":"object","properties":{"agent":{"type":"string","enum":["opencode","codex","omp","claude","agy","hermes","grok"]}},"additionalProperties":false})
        }
        "agent.new_chat" => {
            json!({"type":"object","properties":{"free_only":{"type":"boolean"},"image_task":{"type":"boolean","description":"Native restricted image-job tools; no chat self-invocation"}},"additionalProperties":false})
        }
        "agent.send" => {
            json!({"type":"object","properties":{"message":{"type":"string"},"attachments":{"type":"array","items":{"type":"string"}},"new_chat":{"type":"boolean","default":true},"session":{"type":"string","description":"Optional expected existing session; requires new_chat=false"},"wait":{"type":"integer","minimum":0,"maximum":300},"free_only":{"type":"boolean"},"image_task":{"type":"boolean","description":"Native restricted image-job tools; no chat self-invocation"}},"required":["message"],"additionalProperties":false})
        }
        "storyboard.create" | "storyboard.read" | "storyboard.update" | "storyboard.delete" => {
            let mut schema = schema_from_description(id);
            schema["properties"]["scene"] =
                json!({"oneOf":[{"type":"integer","minimum":1},{"type":"string"}]});
            if id == "storyboard.read" || id == "storyboard.update" {
                schema["properties"]["page"] = json!({"type":"integer","minimum":1});
            }
            schema
        }
        "workshop.build_storyboard" => {
            let mut schema = schema_from_description(id);
            schema["properties"]["template"] = json!({"type":"string","enum":["backlot"]});
            schema
        }
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
                "out_dir": { "type": "string" },
                "rig_order": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional part ids back to front; also writes rig.json for project.add_asset kind=cutout_rig"
                }
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
                "tracks": {
                    "oneOf": [
                        { "type": "array", "items": { "type": "string" } },
                        { "type": "string" }
                    ],
                    "description": "Track ids to build (array or comma-separated); default all"
                },
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
    let described = magicstudio_engine::command_specs()
        .iter()
        .find(|s| s.id == id)
        .and_then(|s| serde_json::from_str::<Value>(s.params).ok());
    let mut properties = serde_json::Map::new();
    if let Some(Value::Object(fields)) = described {
        for (key, example) in fields {
            properties.insert(key, infer_schema(&example));
        }
    }
    json!({ "type": "object", "properties": properties, "additionalProperties": true })
}

/// Infer a JSON Schema fragment from the example value in a command's parameter doc.
/// Nested arrays/objects are recursive; `a|b|c` examples become string enums.
fn infer_schema(example: &Value) -> Value {
    match example {
        Value::Bool(b) => json!({ "type": "boolean", "description": format!("default {b}") }),
        Value::Number(n) => json!({ "type": "number", "description": format!("default {n}") }),
        Value::String(hint) => {
            let options: Vec<&str> = hint.split('|').map(str::trim).collect();
            let enum_like = options.len() > 1
                && options.iter().all(|option| {
                    !option.is_empty()
                        && option
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
                });
            if enum_like {
                json!({ "type": "string", "enum": options, "description": hint })
            } else {
                json!({ "type": "string", "description": hint })
            }
        }
        Value::Array(items) => {
            let items_schema = match items.first() {
                Some(first) if items.iter().all(|item| same_json_kind(item, first)) => {
                    infer_schema(first)
                }
                _ => json!({}),
            };
            json!({ "type": "array", "items": items_schema })
        }
        Value::Object(map) => {
            let mut properties = serde_json::Map::new();
            for (key, value) in map {
                properties.insert(key.clone(), infer_schema(value));
            }
            json!({ "type": "object", "properties": properties, "additionalProperties": true })
        }
        Value::Null => json!({}),
    }
}

fn same_json_kind(a: &Value, b: &Value) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

/// Sent with `initialize` so a client knows where to start without reading the code.
const MCP_INSTRUCTIONS: &str = "Magic Studio turns still character artwork into animated characters and plans \
song videos (storyboard, transcript, video target). Engine commands are tools (command_list shows ids and \
params; tool names are the dotted ids with underscores). Duplicate aliases are omitted from this list and marked \
alias_of by command_list. Start with project_open or project_new, then \
workshop_status (song, transcript, storyboard, assets) or project_list_assets. Assets need a destination: \
project_add_asset {path, character: <name>} copies an outside image to assets/characters/<name>/parts/, \
{background: <name>} to assets/backgrounds/<name>/; a rig spec connects with kind: cutout_rig. Without --bridge \
these tools run a headless engine; to drive the user's open window start the server as \
`mc --bridge auto mcp` after `mc app`, or use the `app_open` tool. Full guide: the \
Magic Studio skill (~/.agents/skills/magicstudio/SKILL.md); CLI: `mc help`.";

pub(super) fn run_mcp_stdio(mut bridge: Option<String>) {
    use std::io::BufRead;
    let stdin = std::io::stdin();
    let mut session = Session::user_session();
    let _presence = mcp_presence::announce(bridge.as_deref());
    let mut stop_after_reply = false;
    let mut binding: Option<Value> = None;
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
                    "instructions": format!("{}{}",if scope::current().ok().flatten().is_some(){"Project-assigned Booth. Only the granted project tools are available. Project identity is fixed; Workshop and global controls belong to Jesse and Gonzo."}else{MCP_INSTRUCTIONS},mcp_presence::orientation(bridge.as_deref()))
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
                let cmd_id = magicstudio_engine::command_specs()
                    .iter()
                    .find(|s| s.id == raw_name || s.id.replace('.', "_") == raw_name)
                    .map(|s| s.id)
                    .unwrap_or(raw_name.as_str());
                let mut args = params.get("arguments").cloned().unwrap_or(json!({}));
                args["_origin"] = json!("MCP");
                if raw_name != "app_open" && raw_name != "app.open" {
                    if binding.is_none() {
                        if let Some(addr)=bridge.as_deref() {
                            match bridge_call(addr,"ui.feedback",json!({})) {
                                Ok(live)=>{ if let Some(endpoint)=live["endpoint"].as_str(){bridge=Some(endpoint.to_owned());} binding=Some(live); },
                                Err(error)=>{outln!("{}",json!({"jsonrpc":"2.0","id":id,"error":{"code":-32000,"message":error}}));continue;}
                            }
                        }
                    }
                    if let Some(live)=&binding {
                        if live["instance"].is_string(){args["_expected_instance"]=live["instance"].clone();}
                        if live["active_project"].is_string(){args["_expected_project"]=live["active_project"].clone();}
                    }
                }

                if let Some(scope) = scope::current().unwrap_or_else(|e| fail(e)) {
                    match scope.params(cmd_id, args.clone()) {
                        Ok(bound) => args = bound,
                        Err(error) => {
                            outln!(
                                "{}",
                                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32000,"message":error}})
                            );
                            continue;
                        }
                    }
                }
                if let Some(v) =
                    center_observation(&raw_name.replace('_', "."), &args, bridge.as_deref())
                {
                    outln!(
                        "{}",
                        json!({"jsonrpc": "2.0", "id": id, "result": {"content": [{"type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default()}]}})
                    );
                    continue;
                }
                if raw_name == "ui_look" || raw_name == "ui.look" {
                    outln!(
                        "{}",
                        json!({"jsonrpc": "2.0", "id": id, "result": ui_look(bridge.as_deref(), args)})
                    );
                    continue;
                }
                let res = if raw_name == "app_open" || raw_name == "app.open" {
                    studios::open(args["project"].as_str()).map(|live| {
                        bridge = live["endpoint"].as_str().map(str::to_owned);
                        binding=Some(live.clone());
                        studio_receipt(&live)
                    })
                } else if raw_name == "app_quit" || raw_name == "app.quit" {
                    let out = call_app_quit(bridge.as_deref(), args);
                    if out.as_ref().is_ok_and(|v| !is_payload_error(v)) {
                        stop_after_reply = true;
                    }
                    out
                } else if matches!(
                    cmd_id,
                    "ui.feedback" | "ui.components" | "ui.controls" | "ui.attention"
                ) {
                    bridge_call(bridge.as_deref().unwrap_or("auto"), cmd_id, args)
                } else if let Some(addr) = &bridge {
                    bridge_call(addr, cmd_id, args)
                } else {
                    dispatch(&mut session, cmd_id, args)
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
            "resources/templates/list" => {
                json!({"jsonrpc":"2.0","id":id,"result":{"resourceTemplates":[]}})
            }
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
pub(super) fn mcp_tools() -> Vec<Value> {
    let mut tools: Vec<Value> = magicstudio_engine::command_specs()
        .iter()
        .map(|spec| {
            json!({
                "name": spec.id.replace('.', "_"),
                "description": format!("{} ({})", spec.label, spec.params),
                "inputSchema": tool_input_schema(spec.id)
            })
        })
        .collect();
    tools.push(json!({"name":"app_open","description":"Open a Magic Studio project in its own window, or a new blank window when project is omitted. Existing windows and unsaved documents are preserved. Returns the live instance and attaches this MCP session; no port configuration needed.","inputSchema":{"type":"object","properties":{"project":{"type":"string","description":"Optional existing .magic file"}},"additionalProperties":false}}));
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
            "properties": {"project": {"type": "string", "description": "canonical .magic path; default the only open Studio"}},
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
                "project": {"type": "string", "description": "canonical .magic path; default the only open Studio"}
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
    let scope = scope::current().ok().flatten();
    tools.retain(|tool| {
        let name = tool["name"].as_str().unwrap_or("");
        let spec = magicstudio_engine::command_specs()
            .iter()
            .find(|s| s.id.replace('.', "_") == name);
        let id = match name {
            "center_snapshot" => "center.snapshot",
            "center_tail" => "center.tail",
            _ => spec.map(|s| s.id).unwrap_or(""),
        };
        // A restricted Booth sees exactly what it was granted. Unrestricted callers see every
        // command: aliases stay listed so no id is ever hidden from an agent.
        scope.is_none_or(|scope| scope.permits(id))
    });
    tools
}

/// `ui_look` as an MCP result: the picture as an image block, the state as text.
fn ui_look(bridge: Option<&str>, mut args: Value) -> Value {
    let error = |e: String| json!({"content": [{"type": "text", "text": e}], "isError": true});
    let Some(addr) = bridge else {
        return error("ui_look needs the native window: start `magicstudio-shell --control <port>` and run this MCP server with `--bridge <port>`".into());
    };
    let out = std::env::temp_dir().join(format!("magicstudio-look-{}.png", std::process::id()));
    args["output"] = json!(out);
    match bridge_call(addr, "ui.look", args).and_then(|v| {
        std::fs::read(&out)
            .map(|png| (v, png))
            .map_err(|e| e.to_string())
    }) {
        Ok((state, png)) => json!({"content": [
            {"type": "image", "data": base64(&png), "mimeType": "image/png"},
            {"type": "text", "text": serde_json::to_string_pretty(&state).unwrap_or_default()}
        ]}),
        Err(e) => error(e),
    }
}

/// `ui.read`: what the live window shows, as text. The window draws itself to a PNG (`ui.look`); the local
/// vision model (Ollama, llava-llama3) reads it and answers `ask=`, and tesseract gives each line of text
/// with its box (window pixels), beside the window's own state, so an agent checks a view by reading it.
pub(super) fn ui_read(addr: &str, mut args: Value) -> Result<Value, String> {
    let out = args["output"]
        .as_str()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!("magicstudio-read-{}.png", std::process::id()))
        });
    args["output"] = json!(out);
    let question = args["ask"].as_str().unwrap_or("").to_owned();
    let state = bridge_call(addr, "ui.look", args)?;
    let ocr = std::process::Command::new("tesseract").arg(&out).arg("stdout").args(["--psm", "11", "tsv"]).output()
        .map_err(|e| format!("tesseract is needed to read the window ({e}); install it with omarchy pkg add tesseract"))?;
    if !ocr.status.success() {
        return Err(format!(
            "tesseract could not read {}: {}",
            out.display(),
            String::from_utf8_lossy(&ocr.stderr).trim()
        ));
    }
    let mut lines: Vec<(String, [i64; 4])> = Vec::new();
    let mut key = String::new();
    for row in String::from_utf8_lossy(&ocr.stdout).lines().skip(1) {
        let f: Vec<&str> = row.split('\t').collect();
        if f.len() < 12 || f[11].trim().is_empty() || f[10].parse::<f64>().unwrap_or(-1.0) < 30.0 {
            continue;
        }
        let n = |i: usize| f[i].parse::<i64>().unwrap_or(0);
        let (x, y, w, h) = (n(6), n(7), n(8), n(9));
        let this = format!("{}-{}-{}", f[2], f[3], f[4]);
        match lines.last_mut() {
            Some((text, b)) if this == key => {
                text.push(' ');
                text.push_str(f[11]);
                b[2] = b[2].max(x + w - b[0]);
                b[3] = b[3].max(y + h - b[1]);
            }
            _ => {
                lines.push((f[11].to_owned(), [x, y, w, h]));
                key = this;
            }
        }
    }
    // The local vision model reads the picture as a person would and answers `ask`, if given.
    let png = std::fs::read(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let ocr_text = lines
        .iter()
        .map(|(t, _)| t.as_str())
        .collect::<Vec<_>>()
        .join(" | ");
    let prompt = format!(
        "This is a screenshot of a desktop app. Text read from it by OCR: {ocr_text}. Using only what you can see and that text, say in two sentences what the screen is showing.{}",
        if question.is_empty() {
            String::new()
        } else {
            format!(" Then answer: {question}")
        }
    );
    let body = json!({"model": std::env::var("MAGICSTUDIO_OCR_VLM").unwrap_or_else(|_| "llava-llama3:latest".into()), "prompt": prompt, "images": [base64(&png)], "stream": false});
    let request =
        std::env::temp_dir().join(format!("magicstudio-read-{}.json", std::process::id()));
    let reading = std::fs::write(&request, body.to_string())
        .ok()
        .and_then(|_| {
            std::process::Command::new("curl")
                .args([
                    "-s",
                    "--max-time",
                    "180",
                    "-H",
                    "Content-Type: application/json",
                    "--data-binary",
                ])
                .arg(format!("@{}", request.display()))
                .arg(
                    std::env::var("OLLAMA_URL")
                        .unwrap_or_else(|_| "http://127.0.0.1:11434/api/generate".into()),
                )
                .output()
                .ok()
        })
        .and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok())
        .and_then(|v| v["response"].as_str().map(str::to_owned));
    let _ = std::fs::remove_file(&request);
    let mut summary = state.clone();
    if let Some(m) = summary.as_object_mut() {
        m.remove("elements");
    }
    Ok(
        json!({"ok": true, "trust": "state and text are facts; reading is the local vision model's description", "image": out, "state": summary, "reading": reading.unwrap_or_else(|| "local vision model unavailable (ollama llava-llama3); OCR text below".into()), "elements": state["elements"].as_array().map_or(0, Vec::len),
        "lines": lines.iter().map(|(t, b)| json!({"text": t, "box": b})).collect::<Vec<_>>(),
        "text": lines.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>().join("\n")}),
    )
}

/// Standard base64 (RFC 4648, padded).
pub(super) fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            s.push(if i <= chunk.len() {
                ABC[(n >> (18 - 6 * i) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    s
}
