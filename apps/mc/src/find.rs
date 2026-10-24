//! Bounded discovery of Magic Studio commands and project objects.

use serde_json::{Value, json};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Hit {
    pub kind: &'static str,
    pub id: String,
    pub label: String,
    pub command: String,
    pub location: Option<String>,
    pub score: i32,
}

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn score(query: &str, id: &str, label: &str, detail: &str) -> i32 {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return 0;
    }
    let id = id.to_ascii_lowercase();
    let label = label.to_ascii_lowercase();
    let detail = detail.to_ascii_lowercase();
    let all = format!("{id} {label} {detail}");
    if !query.split_whitespace().all(|word| all.contains(word)) {
        return 0;
    }
    if id == query {
        120
    } else if label == query {
        110
    } else if id.starts_with(&query) {
        100
    } else if label.starts_with(&query) {
        90
    } else if id.contains(&query) {
        80
    } else if label.contains(&query) {
        70
    } else {
        40
    }
}

pub(crate) fn command_hits(query: &str) -> Vec<Hit> {
    let mut hits = magicstudio_engine::command_specs()
        .iter()
        .filter_map(|spec| {
            let score = score(query, spec.id, spec.label, spec.params);
            (score > 0).then(|| Hit {
                kind: "command",
                id: spec.id.into(),
                label: spec.label.into(),
                command: format!("mc exec {}", spec.id),
                location: None,
                score,
            })
        })
        .collect::<Vec<_>>();
    for (id, label) in [
        ("studios", "Find running Studios"),
        ("backstage", "Open the project command center"),
        ("debug", "Inspect runtime and Studios"),
        ("commands", "List engine commands"),
        ("find", "Find commands and project objects"),
    ] {
        let score = score(query, id, label, "");
        if score > 0 {
            hits.push(Hit {
                kind: "cli",
                id: id.into(),
                label: label.into(),
                command: format!("mc {id}"),
                location: None,
                score,
            });
        }
    }
    hits
}

pub(crate) fn project_hits(project: &str, doc: &Value, query: &str) -> Vec<Hit> {
    let mut hits = Vec::new();
    for asset in doc["assets"].as_array().into_iter().flatten().take(2000) {
        let Some(id) = asset["id"].as_str() else {
            continue;
        };
        let label = asset["name"].as_str().unwrap_or(id);
        let kind = asset["kind"].as_str().unwrap_or("asset");
        let location = asset["path"].as_str().map(str::to_owned);
        let score = score(query, id, label, location.as_deref().unwrap_or(""));
        if score > 0 {
            hits.push(Hit {
                kind: "asset",
                id: id.into(),
                label: format!("{label} · {kind}"),
                command: format!("mc {} project.list_assets", quote(project)),
                location,
                score,
            });
        }
    }
    if let Some(sources) = doc["animation_sources"].as_object() {
        for (id, source) in sources.iter().take(256) {
            let backend = source["backend"].as_str().unwrap_or("native");
            let label = format!("{id} · {backend} source");
            let location = source["source"].as_str().map(str::to_owned);
            let detail = format!(
                "{} {} {}",
                source["character"].as_str().unwrap_or(""),
                source["track"].as_str().unwrap_or(""),
                location.as_deref().unwrap_or("")
            );
            let score = score(query, id, &label, &detail);
            if score > 0 {
                hits.push(Hit {
                    kind: "source",
                    id: id.clone(),
                    label,
                    command: format!("mc {} animation.sources", quote(project)),
                    location,
                    score,
                });
            }
        }
    }
    hits
}

fn suggestion(id: &str, project: &str, suffix: &str, score: i32) -> Option<Hit> {
    let spec = magicstudio_engine::find_command(id)?;
    Some(Hit {
        kind: "command",
        id: id.into(),
        label: spec.label.into(),
        command: format!("mc {} {}{}", quote(project), id, suffix),
        location: None,
        score,
    })
}

pub(crate) fn suggestions(
    project: Option<&str>,
    view: Option<&str>,
    subject: Option<&str>,
) -> Vec<Hit> {
    let Some(project) = project else {
        return vec![
            Hit {
                kind: "cli",
                id: "studios".into(),
                label: "Find a running Studio".into(),
                command: "mc studios".into(),
                location: None,
                score: 100,
            },
            Hit {
                kind: "cli",
                id: "commands".into(),
                label: "List engine commands".into(),
                command: "mc commands".into(),
                location: None,
                score: 90,
            },
        ];
    };
    let mut hits = vec![Hit {
        kind: "cli",
        id: "backstage.snapshot".into(),
        label: "Read project and Studio state".into(),
        command: format!("mc {} backstage.snapshot", quote(project)),
        location: None,
        score: 100,
    }];
    let choices: &[&str] = match view {
        Some("animation") => &["animation.sources", "animation.list", "motion.tracks"],
        Some("project") => &["project.list_assets", "storyboard.toc"],
        _ => &["project.list_assets", "animation.sources"],
    };
    for (index, id) in choices.iter().enumerate() {
        let suffix = if *id == "motion.tracks" {
            subject
                .map(|id| format!(" character={}", quote(id)))
                .unwrap_or_default()
        } else {
            String::new()
        };
        if *id == "motion.tracks" && subject.is_none() {
            continue;
        }
        if let Some(hit) = suggestion(id, project, &suffix, 90 - index as i32 * 10) {
            hits.push(hit);
        }
    }
    hits
}

pub(crate) fn report(query: &str, project_arg: Option<&str>, bridge: Option<&str>) -> Value {
    let mut unavailable = Vec::new();
    let endpoint = bridge
        .map(str::to_owned)
        .or_else(crate::studios::pinned_endpoint)
        .or_else(|| {
            let (rows, _) = crate::studios::rows();
            (rows.len() == 1 && rows[0]["ready"] == true)
                .then(|| rows[0]["endpoint"].as_str().unwrap_or_default().to_string())
        });
    let feedback = endpoint
        .as_deref()
        .and_then(|addr| crate::bridge_call(addr, "ui.feedback", json!({})).ok());
    if let Some(addr) = endpoint.as_deref().filter(|_| feedback.is_none()) {
        unavailable.push(format!("Studio {addr} did not answer; showing offline matches"));
    }
    let live_project = feedback.as_ref().and_then(|v| v["active_project"].as_str());
    let project = project_arg.or(live_project);
    let live_matches = project.is_some() && project == live_project;
    let page = if live_matches {
        endpoint
            .as_deref()
            .and_then(|addr| crate::bridge_call(addr, "ui.state", json!({})).ok())
    } else {
        None
    };
    let view = page.as_ref().and_then(|v| v["view"].as_str());
    let subject = page
        .as_ref()
        .and_then(|v| v["animation"]["character"].as_str());
    let mut hits = if query.trim().is_empty() {
        suggestions(project, view, subject)
    } else {
        command_hits(query)
    };
    if !query.trim().is_empty() {
        if let Some(project) = project {
            let path = Path::new(project);
            if path.extension().is_none_or(|ext| ext != "magic") {
                unavailable.push(format!("Project search requires a .magic file: {project}"));
            } else {
                match std::fs::metadata(path) {
                    Ok(meta) if meta.len() <= 16 * 1024 * 1024 => match std::fs::read(path)
                        .ok()
                        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                    {
                        Some(doc) => hits.extend(project_hits(project, &doc, query)),
                        None => {
                            unavailable.push(format!("Cannot read project data from {project}"))
                        }
                    },
                    Ok(_) => unavailable
                        .push(format!("Project exceeds the 16 MiB find limit: {project}")),
                    Err(error) => {
                        unavailable.push(format!("Cannot inspect project {project}: {error}"))
                    }
                }
            }
        }
        if live_matches && let Some(addr) = endpoint.as_deref() {
            if let Ok(Value::Array(elements)) = crate::bridge_call(addr, "ui.elements", json!({})) {
                for element in elements.into_iter().take(500) {
                    let Some(id) = element["id"].as_str() else {
                        continue;
                    };
                    let label = element["label"].as_str().unwrap_or(id);
                    let score = score(query, id, label, element["kind"].as_str().unwrap_or(""));
                    if score == 0 {
                        continue;
                    }
                    let action = if matches!(
                        element["kind"].as_str(),
                        Some("select" | "input" | "textarea")
                    ) {
                        format!("ui.set id={} value={}", quote(id), quote("VALUE"))
                    } else {
                        format!("ui.click id={}", quote(id))
                    };
                    hits.push(Hit {
                        kind: "control",
                        id: id.into(),
                        label: label.into(),
                        command: format!("mc --bridge {} exec {action}", quote(addr)),
                        location: None,
                        score,
                    });
                }
            }
        }
    }
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.kind.cmp(b.kind))
            .then_with(|| a.id.cmp(&b.id))
    });
    hits.dedup_by(|a, b| a.kind == b.kind && a.id == b.id);
    hits.truncate(12);
    let results = hits.iter().map(|hit| json!({"kind":hit.kind,"id":hit.id,"label":hit.label,"command":hit.command,"location":hit.location})).collect::<Vec<_>>();
    json!({"query":query,"project":project,"studio":endpoint,"view":view,"results":results,"unavailable":unavailable})
}

pub(crate) fn display(report: &Value) -> String {
    let mut lines = vec![if report["query"].as_str().unwrap_or_default().is_empty() {
        "Likely next commands:".to_string()
    } else {
        format!(
            "Matches for {:?}:",
            report["query"].as_str().unwrap_or_default()
        )
    }];
    for hit in report["results"].as_array().into_iter().flatten() {
        lines.push(format!(
            "  {} · {}",
            hit["kind"].as_str().unwrap_or("?"),
            hit["label"].as_str().unwrap_or("?")
        ));
        lines.push(format!("    {}", hit["command"].as_str().unwrap_or("")));
        if let Some(path) = hit["location"].as_str() {
            lines.push(format!("    {path}"));
        }
    }
    if report["results"].as_array().is_none_or(Vec::is_empty) {
        lines.push(
            "  No matches. Try a command name, object ID, source ID, or visible control label."
                .into(),
        );
    }
    for note in report["unavailable"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        lines.push(format!("  Unavailable: {note}"));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn exact_command_id_ranks_first() {
        let hits = command_hits("blender.actions");
        assert_eq!(
            hits.first().map(|hit| hit.id.as_str()),
            Some("blender.actions")
        );
        assert_eq!(
            hits.first().map(|hit| hit.command.as_str()),
            Some("mc exec blender.actions")
        );
        assert!(
            command_hits("studios")
                .iter()
                .any(|hit| hit.id == "studios")
        );
    }

    #[test]
    fn project_search_finds_registered_source_and_asset_without_loading_media() {
        let doc = json!({"assets":[{"id":"little_fish_layered","name":"Little Fish","kind":"cutout_rig","path":"/art/rig.json"}],"animation_sources":{"first-fish":{"id":"first-fish","backend":"blender","source":"/art/fish.blend","character":"little_fish_layered","track":"swim-cruise"}}});
        let sources = project_hits("/song/Hey Ho.magic", &doc, "first-fish");
        assert_eq!(
            sources.first().map(|hit| hit.id.as_str()),
            Some("first-fish")
        );
        assert_eq!(
            sources.first().and_then(|hit| hit.location.as_deref()),
            Some("/art/fish.blend")
        );
        let assets = project_hits("/song/Hey Ho.magic", &doc, "little fish");
        assert_eq!(
            assets.first().map(|hit| hit.id.as_str()),
            Some("little_fish_layered")
        );
    }

    #[test]
    fn empty_query_suggests_read_only_animation_inspection() {
        let hits = suggestions(
            Some("/song/Hey Ho.magic"),
            Some("animation"),
            Some("little_fish_layered"),
        );
        assert!(hits.iter().any(|hit| hit.id == "animation.sources"));
        assert!(hits.iter().any(|hit| hit.id == "motion.tracks"));
        assert!(!hits.iter().any(|hit| hit.id == "project.save"));
    }
}
