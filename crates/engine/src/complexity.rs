//! Code complexity analytics for the app's Analytics view.
//!
//! Two costs are measured side by side: cyclomatic complexity (what a change
//! costs) and physical file length (what reading a change costs). Long files
//! are what slow agents down, so they are reported with the same weight as
//! complex functions instead of being left to guesswork.

use std::path::Path;

use serde_json::{Value, json};

use crate::workspace::resolve_craft_cli;

/// A file at or above this many lines is reported as a long file. The limit is
/// configurable in code-limits.json at the repository root.
const COMPLEXITY_LONG_FILE_LINES: u64 = 2000;

fn long_file_limit(root: &Path) -> u64 {
    let Ok(text) = std::fs::read_to_string(root.join("code-limits.json")) else {
        return COMPLEXITY_LONG_FILE_LINES;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return COMPLEXITY_LONG_FILE_LINES;
    };
    value["max_file_lines"].as_u64().unwrap_or(COMPLEXITY_LONG_FILE_LINES)
}

/// The sources worth measuring: the Rust engine and CLIs, the Python pipeline and
/// scripts, and the shell UI's own JavaScript modules (vendor bundles are excluded).
fn scan_targets(root: &Path) -> Vec<String> {
    let mut targets: Vec<String> = vec![
        "crates/engine/src".to_string(),
        "agent-harness/cli_anything/magic".to_string(),
        "apps/magicstudio-shell/src".to_string(),
        "pipeline".to_string(),
        "scripts".to_string(),
    ];
    let ui_dir = root.join("apps/magicstudio-shell/ui");
    let Ok(entries) = std::fs::read_dir(ui_dir) else {
        return targets;
    };
    let mut ui_files: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("js") {
                return None;
            }
            path.file_name().map(|name| {
                format!(
                    "apps/magicstudio-shell/ui/{}",
                    name.to_string_lossy()
                )
            })
        })
        .collect();
    ui_files.sort();
    targets.extend(ui_files);
    targets
}

/// Run lizard over the given targets. A single scan keeps the common case fast; when
/// one target makes lizard fail to parse, the scan is retried per target so a single
/// unreadable file never hides every measurement. Failed targets are reported.
fn lizard_csv(lizard_bin: &str, root: &Path, targets: &[String]) -> (String, Vec<String>) {
    let run = |list: &[String]| {
        let mut command = std::process::Command::new(lizard_bin);
        command
            .current_dir(root)
            .args(["--csv", "-l", "rust", "-l", "python", "-l", "javascript"]);
        for target in list {
            command.arg(target);
        }
        command.output()
    };
    let Ok(out) = run(targets) else {
        return (String::new(), targets.to_vec());
    };
    if out.status.success() {
        return (String::from_utf8_lossy(&out.stdout).to_string(), Vec::new());
    }
    let mut stdout = String::new();
    let mut failures = Vec::new();
    for target in targets {
        match run(std::slice::from_ref(target)) {
            Ok(single) if single.status.success() => {
                stdout.push_str(&String::from_utf8_lossy(&single.stdout));
            }
            _ => failures.push(target.clone()),
        }
    }
    (stdout, failures)
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

/// Measure the repository's complexity and reading cost.
pub fn read(root: &Path, scan: bool) -> Value {
    let cache = root.join("target/analytics/lizard.json");
    let revision = source_revision(root);
    let dirty = source_dirty(root);
    let previous = std::fs::read(&cache).ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .filter(|v| v["available"] == true && v["source_root"] == root.to_string_lossy().as_ref());
    if !scan {
        return previous.map(|value| cached_result(value, &revision, dirty)).unwrap_or_else(||
            json!({"available":false,"tool":"lizard","state":"not_measured","reason":"No saved scan. Refresh scan to measure code.","source_root":root,"current_revision":revision}));
    }
    let mut result = measure(root);
    result["tool"] = json!("lizard");
    let executable = resolve_craft_cli("lizard");
    let executable_path = if Path::new(&executable).is_file() {Some(std::path::PathBuf::from(&executable))} else {
        std::env::var_os("PATH").and_then(|paths|std::env::split_paths(&paths).map(|path|path.join(&executable)).find(|path|path.is_file()))
    };
    result["tool_path"] = json!(executable_path);
    result["tool_version"] = json!(std::process::Command::new(&executable).arg("--version").output().ok().filter(|o|o.status.success()).map(|o|String::from_utf8_lossy(&o.stdout).trim().to_string()));
    result["targets"] = json!(scan_targets(root));
    result["source_root"] = json!(root);
    result["source_revision"] = json!(revision);
    result["source_dirty"] = json!(dirty);
    result["measured_at_unix_secs"] = json!(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok().map(|v|v.as_secs()));
    result["cached"] = json!(false);
    result["state"] = json!(if result["available"] == true {"measured"} else {"error"});
    if result["available"] != true {
        let error = "Lizard returned no measurements; check tool availability and scan_failures.";
        if let Some(value) = previous {
            let mut saved = cached_result(value, &revision, dirty);
            saved["last_error"] = json!(error);
            saved["scan_failures"] = result["scan_failures"].clone();
            return saved;
        }
        result["reason"] = json!(error);
        return result;
    }
    let save = || -> std::io::Result<()> {
        std::fs::create_dir_all(cache.parent().expect("cache has parent"))?;
        // A unique temporary file prevents concurrent scans from truncating the saved result.
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|v|v.as_nanos()).unwrap_or(0);
        let temporary = cache.with_extension(format!("{}-{stamp}.tmp",std::process::id()));
        let bytes = serde_json::to_vec(&result).map_err(std::io::Error::other)?;
        std::fs::write(&temporary, bytes)?;
        if let Err(error) = std::fs::rename(&temporary, &cache) {let _=std::fs::remove_file(temporary);return Err(error);}
        Ok(())
    };
    if let Err(error) = save() {result["cache_error"] = json!(error.to_string());}
    result
}

fn source_revision(root: &Path) -> Option<String> {
    std::process::Command::new("git").args(["rev-parse","HEAD"]).current_dir(root).output().ok()
        .filter(|o|o.status.success()).map(|o|String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn source_dirty(root: &Path) -> Option<bool> {
    std::process::Command::new("git").args(["status","--porcelain","--untracked-files=normal","--",
        "crates/engine/src","agent-harness/cli_anything/magic","apps/magicstudio-shell/src","apps/magicstudio-shell/ui","pipeline","scripts","code-limits.json"])
        .current_dir(root).output().ok().filter(|o|o.status.success()).map(|o|!o.stdout.is_empty())
}

fn cached_result(mut value: Value, revision: &Option<String>, dirty: Option<bool>) -> Value {
    value["cached"] = json!(true);
    value["current_revision"] = json!(revision);
    value["state"] = json!(if revision.is_none() || dirty.is_none() || dirty == Some(true) || value["source_dirty"] != false {
        "worktree_unverified"
    } else if value["source_revision"] != json!(revision) {"stale"} else {"saved"});
    value
}

fn measure(root: &Path) -> Value {
    let lizard_bin = resolve_craft_cli("lizard");
    let long_file_limit = long_file_limit(root);
    let targets = scan_targets(root);
    let (stdout, scan_failures) = lizard_csv(&lizard_bin, root, &targets);
    let mut functions = Vec::new();
    let mut files = std::collections::HashSet::new();
    let mut per_file: std::collections::BTreeMap<String, (u64, usize, u64)> =
        std::collections::BTreeMap::new();
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
        let entry = per_file.entry(file.clone()).or_insert((0, 0, 0));
        entry.0 = entry.0.saturating_add(ccn);
        entry.1 += 1;
        entry.2 = entry.2.max(ccn);
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

    // Per-file cost: physical lines (what an agent has to read) next to cyclomatic
    // complexity (what the code costs to change).
    let mut file_rows: Vec<Value> = per_file
        .iter()
        .map(|(file, (ccn, functions, max_file_ccn))| {
            let lines = std::fs::read_to_string(root.join(file))
                .map(|text| text.lines().count() as u64)
                .unwrap_or(0);
            json!({
                "file": file,
                "lines": lines,
                "ccn": ccn,
                "functions": functions,
                "max_ccn": max_file_ccn,
                "long": lines >= long_file_limit,
            })
        })
        .collect();
    file_rows.sort_by_key(|row| {
        (
            std::cmp::Reverse(row["lines"].as_u64().unwrap_or(0)),
            std::cmp::Reverse(row["ccn"].as_u64().unwrap_or(0)),
        )
    });
    let long_files: Vec<Value> = file_rows
        .iter()
        .filter(|row| row["long"].as_bool().unwrap_or(false))
        .cloned()
        .collect();
    let long_file_count = long_files.len();
    let longest_files: Vec<Value> = long_files.iter().take(15).cloned().collect();
    let mut by_ccn: Vec<Value> = file_rows.clone();
    by_ccn.sort_by_key(|row| {
        (
            std::cmp::Reverse(row["ccn"].as_u64().unwrap_or(0)),
            std::cmp::Reverse(row["lines"].as_u64().unwrap_or(0)),
        )
    });
    let largest_ccn_files: Vec<Value> = by_ccn.iter().take(15).cloned().collect();
    let ui_files_analyzed = file_rows
        .iter()
        .filter(|row| row["file"].as_str().unwrap_or("").starts_with("apps/magicstudio-shell/ui/"))
        .count();
    let total_lines: u64 = file_rows
        .iter()
        .map(|row| row["lines"].as_u64().unwrap_or(0))
        .sum();

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
        "long_file_threshold_lines": long_file_limit,
        "long_files": long_file_count,
        "longest_files": longest_files,
        "largest_ccn_files": largest_ccn_files,
        "ui_files_analyzed": ui_files_analyzed,
        "total_source_lines": total_lines,
        "scan_failures": scan_failures,
    })
}
