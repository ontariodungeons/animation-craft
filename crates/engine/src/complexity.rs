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
/// configurable in code-limits.json at the repository root, which is also what
/// the commit hook enforces.
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
        "apps/mc/src".to_string(),
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
    if !scan {
        return json!({ "available": false, "reason": "Scan disabled" });
    }
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
