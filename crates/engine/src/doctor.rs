//! `doctor` subcommand (`mc doctor` / `app.doctor`):
//! lightweight local check of external CLI tool availability and stale `.video-server.json` state.
//! Never scans `/proc` to police or kill external processes.

use std::path::Path;

use serde_json::{Value, json};

use crate::workspace::resolve_craft_cli;
use crate::{Result, Session};

pub fn cmd_app_doctor(s: &mut Session, params: &Value) -> Result<Value> {
    let clean = params
        .get("clean")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let cleaned_files: Vec<String> = Vec::new();
    let project_dir = s
        .project_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| s.cwd.clone());
    let server_file = project_dir.join(".video-server.json");
    let mut video_server_state = json!("none");
    if server_file.is_file() {
        if clean {
            video_server_state = json!({"file": server_file, "clean_requested": true, "removed": false, "note": "ownership/staleness not verified; not deleted"});
        } else {
            video_server_state = json!({"file": server_file});
        }
    }

    let tools: Vec<Value> = [
        "hyperframes",
        "photocraft-cli",
        "effectcraft-cli",
        "filmcraft-cli",
        "pdfcraft-cli",
        "cli-anything-blender",
        "ffmpeg",
        "ffprobe",
    ]
    .iter()
    .map(|name| {
        let resolved = resolve_craft_cli(name);
        let found = Path::new(&resolved).is_file() || which_in_path(&resolved);
        json!({ "tool": name, "resolved": resolved, "available": found })
    })
    .collect();

    Ok(json!({
        "ok": true,
        "cleaned": {
            "stopped_pids": null,
            "removed_files": cleaned_files,
            "note": "processes are not scanned or killed; .video-server.json is reported but not verified or deleted"
        },
        "processes": null,
        "video_server": video_server_state,
        "tools": tools,
        "ponytail_candidates": null,
        "recommendations": [],
    }))
}

pub fn cmd_app_analytics(s: &mut Session, params: &Value) -> Result<Value> {
    let scan = params
        .get("scan")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let doctor = cmd_app_doctor(s, &json!({ "clean": false }))?;
    let host = read_host_telemetry();
    let usage = read_omarchy_usage();
    let project = read_project_analytics(s);
    let root = crate::workspace::workspace_root(s);
    let complexity = read_complexity(&root, scan);

    Ok(json!({
        "ok": true,
        "host": host,
        "tools": doctor["tools"],
        "video_server": doctor["video_server"],
        "usage": usage,
        "project": project,
        "complexity": complexity,
    }))
}

fn read_host_telemetry() -> Value {
    let pid = std::process::id();
    let self_rss_kb = std::fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse::<u64>().ok())
        })
        .unwrap_or(0);
    let load = std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| s.split_whitespace().take(3).collect::<Vec<_>>().join(" ").into())
        .unwrap_or_default();
    let mut mem_total_kb = 0u64;
    let mut mem_avail_kb = 0u64;
    if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
        for line in meminfo.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                mem_total_kb = rest.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
            } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
                mem_avail_kb = rest.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
            }
        }
    }
    json!({
        "pid": pid,
        "self_rss_kb": self_rss_kb,
        "load": load,
        "mem_total_kb": mem_total_kb,
        "mem_used_kb": mem_total_kb.saturating_sub(mem_avail_kb),
    })
}

fn read_omarchy_usage() -> Value {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/state")));
    let Some(dir) = base.map(|b| b.join("omarchy/agents/usage")) else {
        return json!({ "available": false, "agents": [], "totals": {} });
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return json!({ "available": false, "agents": [], "totals": {} });
    };
    let mut agents = Vec::new();
    let mut today_tokens_total = 0u64;
    let mut today_prompts_total = 0u64;
    let mut all_time_input = 0u64;
    let mut all_time_output = 0u64;
    let mut all_time_cache_read = 0u64;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let Ok(v) = serde_json::from_slice::<Value>(&bytes) else { continue };
        let today_tokens = v["todayTotalTokens"].as_u64().unwrap_or(0);
        let today_prompts = v["todayPrompts"].as_u64().unwrap_or(0);
        let total_prompts = v["totalPrompts"].as_u64().unwrap_or(0);
        let total_sessions = v["totalSessions"].as_u64().unwrap_or(0);
        let mut input_sum = 0u64;
        let mut output_sum = 0u64;
        let mut cache_sum = 0u64;
        let mut models = Vec::new();
        if let Some(map) = v["modelUsage"].as_object() {
            for (m_id, mv) in map {
                let inp = mv["inputTokens"].as_u64().unwrap_or(0);
                let out = mv["outputTokens"].as_u64().unwrap_or(0);
                let cr = mv["cacheReadInputTokens"].as_u64().unwrap_or(0);
                input_sum = input_sum.saturating_add(inp);
                output_sum = output_sum.saturating_add(out);
                cache_sum = cache_sum.saturating_add(cr);
                if inp + out + cr > 0 {
                    models.push(json!({
                        "model": m_id,
                        "input_tokens": inp,
                        "output_tokens": out,
                        "cache_read_tokens": cr,
                        "total_tokens": inp.saturating_add(out).saturating_add(cr),
                    }));
                }
            }
        }
        models.sort_by_key(|m| std::cmp::Reverse(m["total_tokens"].as_u64().unwrap_or(0)));
        let total_agent_tokens = input_sum.saturating_add(output_sum).saturating_add(cache_sum);
        if today_tokens == 0 && total_prompts == 0 && total_agent_tokens == 0 {
            continue;
        }
        today_tokens_total = today_tokens_total.saturating_add(today_tokens);
        today_prompts_total = today_prompts_total.saturating_add(today_prompts);
        all_time_input = all_time_input.saturating_add(input_sum);
        all_time_output = all_time_output.saturating_add(output_sum);
        all_time_cache_read = all_time_cache_read.saturating_add(cache_sum);
        agents.push(json!({
            "id": v["id"].as_str().unwrap_or_default(),
            "name": v["name"].as_str().unwrap_or_default(),
            "tier": v["tierLabel"].as_str().unwrap_or_default(),
            "today_tokens": today_tokens,
            "today_prompts": today_prompts,
            "total_prompts": total_prompts,
            "total_sessions": total_sessions,
            "input_tokens": input_sum,
            "output_tokens": output_sum,
            "cache_read_tokens": cache_sum,
            "total_tokens": total_agent_tokens,
            "limits": v["limits"],
            "models": models,
        }));
    }
    agents.sort_by_key(|a| std::cmp::Reverse(a["today_tokens"].as_u64().unwrap_or(0).max(a["total_tokens"].as_u64().unwrap_or(0) / 100)));
    json!({
        "available": true,
        "agents": agents,
        "totals": {
            "today_tokens": today_tokens_total,
            "today_prompts": today_prompts_total,
            "input_tokens": all_time_input,
            "output_tokens": all_time_output,
            "cache_read_tokens": all_time_cache_read,
            "total_tokens": all_time_input.saturating_add(all_time_output).saturating_add(all_time_cache_read),
        }
    })
}

fn read_project_analytics(s: &Session) -> Value {
    let turns = crate::agent::log_read(s);
    let list = turns.as_array().cloned().unwrap_or_default();
    let mut user_turns = 0usize;
    let mut assistant_turns = 0usize;
    let mut total_chars = 0usize;
    for item in &list {
        let role = item["role"].as_str().unwrap_or_default();
        if role == "user" {
            user_turns += 1;
        } else if role == "assistant" {
            assistant_turns += 1;
        }
        total_chars += item["text"].as_str().map(str::len).unwrap_or(0);
    }
    let estimated_tokens = (total_chars / 4) as u64;
    json!({
        "project": s.project_path,
        "assets_count": s.project_assets.len(),
        "user_turns": user_turns,
        "assistant_turns": assistant_turns,
        "total_turns": list.len(),
        "total_chars": total_chars,
        "estimated_tokens": estimated_tokens,
    })
}

fn read_complexity(root: &Path, scan: bool) -> Value {
    if !scan {
        return json!({ "available": false, "reason": "Scan disabled" });
    }
    let lizard_bin = resolve_craft_cli("lizard");
    let output = std::process::Command::new(&lizard_bin)
        .current_dir(root)
        .args([
            "--csv",
            "-l",
            "rust",
            "-l",
            "python",
            "crates/engine/src",
            "apps/mc/src",
            "apps/magicstudio-shell/src",
            "pipeline",
            "scripts",
        ])
        .output();
    let Ok(out) = output else {
        return json!({ "available": false, "reason": "lizard CLI not found on PATH" });
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut functions = Vec::new();
    let mut files = std::collections::HashSet::new();
    let mut total_nloc = 0u64;
    let mut total_tokens = 0u64;
    let mut over_15 = 0usize;
    let mut max_ccn = 0u64;

    for line in stdout.lines() {
        let fields = parse_csv_line(line);
        if fields.len() < 11 {
            continue;
        }
        let Ok(nloc) = fields[0].parse::<u64>() else { continue };
        let Ok(ccn) = fields[1].parse::<u64>() else { continue };
        let Ok(tokens) = fields[2].parse::<u64>() else { continue };
        let Ok(params) = fields[3].parse::<u64>() else { continue };
        let file = fields[6].clone();
        let name = fields[7].clone();
        let start_line = fields[9].parse::<u64>().unwrap_or(1);
        let end_line = fields[10].parse::<u64>().unwrap_or(start_line);

        files.insert(file.clone());
        total_nloc = total_nloc.saturating_add(nloc);
        total_tokens = total_tokens.saturating_add(tokens);
        if ccn > 15 {
            over_15 += 1;
        }
        if ccn > max_ccn {
            max_ccn = ccn;
        }
        functions.push(json!({
            "name": name,
            "file": file,
            "start_line": start_line,
            "end_line": end_line,
            "nloc": nloc,
            "ccn": ccn,
            "tokens": tokens,
            "params": params,
        }));
    }
    functions.sort_by_key(|f| (std::cmp::Reverse(f["ccn"].as_u64().unwrap_or(0)), std::cmp::Reverse(f["nloc"].as_u64().unwrap_or(0))));
    let hotspots: Vec<Value> = functions.iter().take(25).cloned().collect();
    json!({
        "available": !functions.is_empty(),
        "tool": "lizard",
        "files_analyzed": files.len(),
        "functions_analyzed": functions.len(),
        "total_nloc": total_nloc,
        "total_code_tokens": total_tokens,
        "functions_over_ccn_15": over_15,
        "max_ccn": max_ccn,
        "hotspots": hotspots,
    })
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut res = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for ch in line.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                res.push(std::mem::take(&mut cur));
            }
            _ => cur.push(ch),
        }
    }
    res.push(cur);
    res
}

fn which_in_path(bin: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| dir.join(bin).is_file())
    })
}
