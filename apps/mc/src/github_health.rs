//! One explicit, read-only observation of Git and GitHub state for the current checkout.
use serde_json::{Value, json};
use std::{path::{Path, PathBuf}, process::Command, time::{SystemTime, UNIX_EPOCH}};

const PR_LIMIT: &str = "30";

pub fn empty() -> Value {
    json!({"schema":"mc.github-health/1","ok":null,"refreshing":false,"observed_at_ms":null,
        "repository":null,"branch":null,"local_sha":null,"main_sha":null,"remote_sha":null,
        "remote_sha_source":"refs/remotes/origin/main; last fetched tracking ref, no fetch performed",
        "upstream":null,"ahead":null,"behind":null,"dirty_files":null,"pull_requests":null,"errors":[]})
}

pub fn inspect(repo: &Path) -> Value {
    let mut errors = Vec::new();
    let remote_url = git(repo, &["remote", "get-url", "origin"], &mut errors);
    let repository = remote_url.as_deref().and_then(|url| repository_slug(url));
    if remote_url.is_some() && repository.is_none() {
        errors.push("origin is not a recognized owner/repo GitHub URL".into());
    }
    let branch = git(repo, &["rev-parse", "--abbrev-ref", "HEAD"], &mut errors);
    let local_sha = git(repo, &["rev-parse", "--verify", "HEAD"], &mut errors);
    let main_sha = reference(repo, "refs/heads/main", "local main", &mut errors);
    // This is the last fetched tracking ref; collection does not fetch or contact git remotes.
    let remote_sha = reference(repo, "refs/remotes/origin/main", "origin/main tracking ref", &mut errors);
    let upstream = match command(repo, "git", &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"]) {
        Ok(value) => Some(value.trim().to_owned()),
        Err(error) if error.contains("no upstream configured") => None,
        Err(error) => { errors.push(error); None }
    };
    let (ahead, behind) = if upstream.is_some() {
        match git(repo, &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"], &mut errors) {
            Some(value) => parse_ahead_behind(&value).map(|(ahead, behind)| (Some(ahead), Some(behind)))
                .unwrap_or_else(|| { errors.push("git returned an invalid ahead/behind count".into()); (None, None) }),
            None => (None, None),
        }
    } else { (None, None) };
    let dirty_files = git(repo, &["status", "--porcelain=v1", "-uall"], &mut errors)
        .map(|value| value.lines().filter(|line| !line.trim().is_empty()).count());
    let pull_requests = repository.as_deref().and_then(|slug| {
        match command(repo, "gh", &["pr", "list", "--repo", slug, "--state", "open", "--limit", PR_LIMIT,
            "--json", "number,title,headRefName,isDraft,statusCheckRollup,url"]) {
            Ok(value) => match serde_json::from_str::<Value>(&value) {
                Ok(Value::Array(rows)) => Some(rows.into_iter().map(summarize_pr).collect::<Vec<_>>()),
                Ok(_) => { errors.push("gh pr list returned a non-array result".into()); None }
                Err(error) => { errors.push(format!("gh pr list returned invalid JSON: {error}")); None }
            },
            Err(error) => { errors.push(error); None }
        }
    });
    let observed_at_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    json!({
        "schema": "mc.github-health/1",
        "ok": errors.is_empty(),
        "observed_at_ms": observed_at_ms,
        "repository": repository,
        "branch": branch,
        "local_sha": local_sha,
        "main_sha": main_sha,
        "remote_sha": remote_sha,
        "remote_sha_source": "refs/remotes/origin/main; last fetched tracking ref, no fetch performed",
        "upstream": upstream,
        "ahead": ahead,
        "behind": behind,
        "dirty_files": dirty_files,
        "pull_requests": pull_requests,
        "errors": errors,
    })
}

fn reference(repo: &Path, name: &str, label: &str, errors: &mut Vec<String>) -> Option<String> {
    match command(repo, "git", &["rev-parse", "--verify", name]) {
        Ok(value) => Some(value.trim().to_owned()),
        Err(error) => { errors.push(format!("{label}: {error}")); None }
    }
}

fn git(repo: &Path, args: &[&str], errors: &mut Vec<String>) -> Option<String> {
    match command(repo, "git", args) {
        Ok(value) => Some(value),
        Err(error) => { errors.push(error); None }
    }
}

fn command(repo: &Path, program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program).args(args).current_dir(repo).output()
        .map_err(|error| format!("{program} {} could not run: {error}", args.join(" ")))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(format!("{program} {} exited {:?}: {}", args.join(" "), output.status.code(),
            if detail.is_empty() { "no stderr" } else { &detail }));
    }
    String::from_utf8(output.stdout).map_err(|error| format!("{program} output was not UTF-8: {error}"))
}

pub fn repository_slug(remote: &str) -> Option<String> {
    let remote = remote.trim().strip_suffix(".git").unwrap_or(remote.trim());
    let path = if let Some((_, rest)) = remote.split_once("://") {
        rest.split_once('/')?.1
    } else if let Some((_, rest)) = remote.split_once(':') {
        rest
    } else {
        remote
    };
    let mut parts = path.trim_matches('/').split('/');
    let owner = parts.next()?.trim();
    let repo = parts.next()?.trim();
    if owner.is_empty() || repo.is_empty() || parts.next().is_some() { return None; }
    Some(format!("{owner}/{repo}"))
}

pub fn parse_ahead_behind(value: &str) -> Option<(u64, u64)> {
    let mut fields = value.split_whitespace();
    Some((fields.next()?.parse().ok()?, fields.next()?.parse().ok()?))
}

fn summarize_pr(pr: Value) -> Value {
    let checks = pr["statusCheckRollup"].as_array();
    let mut passed = 0;
    let mut failed = 0;
    let mut pending = 0;
    let mut unknown = 0;
    for check in checks.into_iter().flatten() {
        let state = check["conclusion"].as_str().or_else(|| check["state"].as_str()).unwrap_or("").to_ascii_uppercase();
        match state.as_str() {
            "SUCCESS" | "SUCCEEDED" => passed += 1,
            "FAILURE" | "FAILED" | "ERROR" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" => failed += 1,
            "PENDING" | "IN_PROGRESS" | "QUEUED" | "EXPECTED" | "REQUESTED" => pending += 1,
            _ => unknown += 1,
        }
    }
    let check_state = if checks.is_none() { "unavailable" }
        else if failed > 0 { "failed" }
        else if pending > 0 { "pending" }
        else if unknown > 0 { "unknown" }
        else if passed > 0 { "passed" }
        else { "no checks" };
    json!({
        "number": pr["number"], "title": pr["title"], "head_branch": pr["headRefName"],
        "draft": pr["isDraft"], "url": pr["url"],
        "checks": {"state": check_state, "passed": passed, "failed": failed, "pending": pending, "unknown": unknown,
            "total": checks.map_or(Value::Null, |items| json!(items.len()))},
    })
}

pub fn repo_root(dir: &Path) -> PathBuf {
    dir.ancestors().find(|path| path.join(".git").exists()).unwrap_or(dir).to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_ssh_https_and_ssh_url_remotes() {
        for remote in ["git@github.com:owner/repo.git", "https://github.com/owner/repo.git", "ssh://git@github.com/owner/repo.git"] {
            assert_eq!(repository_slug(remote).as_deref(), Some("owner/repo"));
        }
        assert_eq!(repository_slug("https://github.com/owner/repo/extra"), None);
    }

    #[test]
    fn parses_ahead_then_behind_and_rejects_missing_counts() {
        assert_eq!(parse_ahead_behind("3\t7\n"), Some((3, 7)));
        assert_eq!(parse_ahead_behind("0"), None);
        assert_eq!(parse_ahead_behind("unknown 2"), None);
    }

    #[test]
    fn reports_check_failures_and_unknowns_without_claiming_success() {
        let pr = summarize_pr(json!({"number":4,"statusCheckRollup":[{"state":"SUCCESS"},{"conclusion":"FAILURE"},{"state":"QUEUED"}]}));
        assert_eq!(pr["checks"]["state"], "failed");
        assert_eq!(pr["checks"]["passed"], 1);
        assert_eq!(pr["checks"]["failed"], 1);
        assert_eq!(pr["checks"]["pending"], 1);
    }

    #[test]
    fn missing_check_rollup_is_unavailable_not_empty_success() {
        let pr = summarize_pr(json!({"number":4}));
        assert_eq!(pr["checks"]["state"], "unavailable");
        assert!(pr["checks"]["total"].is_null());
    }

    #[test]
    fn a_git_observation_failure_is_not_reported_as_clean() {
        let path = std::env::temp_dir().join(format!("mc-github-health-{}-{}", std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()));
        std::fs::create_dir_all(&path).unwrap();
        let report = inspect(&path);
        assert_eq!(report["ok"], false);
        assert!(!report["errors"].as_array().unwrap().is_empty());
        assert!(report["ahead"].is_null());
        assert!(report["behind"].is_null());
        std::fs::remove_dir_all(path).unwrap();
    }
}
