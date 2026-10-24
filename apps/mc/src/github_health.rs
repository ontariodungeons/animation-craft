//! GitHub and worktree health: what an agent did to the repository's GitHub state.
//!
//! This answers one question plainly: what is broken right now — unpushed commits,
//! branches that fell behind, open pull requests, worktrees nobody cleaned up, and
//! history that was rewritten under a force-push. Everything is read from `git` and
//! `gh`; nothing is changed, pushed, closed or deleted.

use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};

/// One thing that needs a human. `severity` is "break" (data or history at risk),
/// "warn" (work that is not where it should be) or "note" (worth knowing).
#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub severity: &'static str,
    pub area: String,
    pub detail: String,
}

/// A pull request that is open against this repository.
#[derive(Clone, Debug, PartialEq)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub branch: String,
    pub is_draft: bool,
    pub updated: String,
}

/// A local branch and how it relates to its upstream.
#[derive(Clone, Debug, PartialEq)]
pub struct Branch {
    pub name: String,
    pub ahead: u64,
    pub behind: u64,
    pub has_upstream: bool,
}

/// A worktree and whether it holds unsaved work.
#[derive(Clone, Debug, PartialEq)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub dirty_files: usize,
    pub missing: bool,
}


/// The backstage fanfare: what a moment sounds and looks like. Success sparkles,
/// an error sirens, and everything else is quiet. Sound is optional — the shape
/// carries the meaning on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    Sparkle,
    Siren,
    Quiet,
}

impl Mood {
    /// The mood for one event kind. A finished job twinkles; an error sirens;
    /// ordinary events (a selection, a save) stay quiet.
    pub fn for_kind(kind: &str, ok: bool) -> Mood {
        match kind {
            "error" => Mood::Siren,
            "generation_finished" | "snapshot_finished" | "render_finished"
            | "layered_motion_finished" | "motion_path_drawn" | "keyframe_added" => Mood::Sparkle,
            _ => Mood::Quiet,
        }
        .pipe_ok(ok)
    }

    /// A failed success is a siren, not a sparkle.
    fn pipe_ok(self, ok: bool) -> Mood {
        if ok {
            self
        } else {
            Mood::Siren
        }
    }

    /// The frames of the animation, in order. A siren waves; a sparkle twinkles.
    pub fn frames(&self) -> &'static [&'static str] {
        match self {
            Mood::Sparkle => &["✦", "✧", "✦", "·", "✦", "✧", "✦", "·"],
            Mood::Siren => &["▓", "▒", "░", "▒", "▓", "▒", "░", "▒"],
            Mood::Quiet => &["·"],
        }
    }

    /// The frame for an elapsed tick, so the same event always draws the same shape.
    pub fn frame(&self, tick: u64) -> &'static str {
        let frames = self.frames();
        frames[(tick as usize) % frames.len()]
    }

    /// The sound to play, if any. Nothing here plays audio by itself.
    pub fn sound(&self) -> Option<&'static str> {
        match self {
            Mood::Sparkle => Some("sparkle"),
            Mood::Siren => Some("siren"),
            Mood::Quiet => None,
        }
    }
}

/// The bell a fanfare rings when a long job finishes.
#[derive(Clone, Debug, PartialEq)]
pub struct Fanfare {
    pub mood: Mood,
    pub label: String,
    pub detail: String,
}

impl Fanfare {
    /// One fanfare for one event.
    pub fn for_event(kind: &str, ok: bool, label: &str, detail: &str) -> Fanfare {
        Fanfare {
            mood: Mood::for_kind(kind, ok),
            label: label.to_owned(),
            detail: detail.to_owned(),
        }
    }
}

/// The whole picture of the repository's GitHub state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Health {
    pub remote: String,
    pub branch: String,
    pub head_sha: String,
    /// How far the checked-out branch is from its upstream (ahead, behind).
    pub ahead: u64,
    pub behind: u64,
    pub unpushed: Vec<Branch>,
    pub pull_requests: Vec<PullRequest>,
    pub worktrees: Vec<Worktree>,
    pub local_branches: usize,
    pub stale_worktrees: usize,
    pub dirty_worktrees: usize,
    pub gh_authenticated: bool,
    pub gh_account: Option<String>,
    pub issues: Vec<Issue>,
}

impl Health {
    /// The focused summary: one line per area, then the count that matters.
    pub fn summary(&self) -> Value {
        json!({
            "branch": self.branch,
            "head_sha": self.head_sha,
            "remote": self.remote,
            "upstream_ahead": self.ahead,
            "upstream_behind": self.behind,
            "unpushed_branches": self.unpushed.len(),
            "unpushed_commits": self.unpushed.iter().map(|b| b.ahead).sum::<u64>(),
            "open_pull_requests": self.pull_requests.len(),
            "worktrees": self.worktrees.len(),
            "stale_worktrees": self.stale_worktrees,
            "dirty_worktrees": self.dirty_worktrees,
            "local_branches": self.local_branches,
            "gh_authenticated": self.gh_authenticated,
            "gh_account": self.gh_account,
            "issues": self.issues.len(),
            "breaking": self.issues.iter().filter(|i| i.severity == "break").count(),
        })
    }

    /// Every issue, most severe first, with the areas that need attention grouped.
    pub fn report(&self) -> Value {
        let mut by_area: BTreeMap<&str, Vec<&Issue>> = BTreeMap::new();
        for issue in &self.issues {
            by_area.entry(issue.area.as_str()).or_default().push(issue);
        }
        let mut areas: Vec<Value> = Vec::new();
        for (area, issues) in by_area {
            areas.push(json!({
                "area": area,
                "count": issues.len(),
                "issues": issues.iter().map(|i| json!({"severity": i.severity, "detail": i.detail})).collect::<Vec<_>>(),
            }));
        }
        json!({
            "ok": true,
            "summary": self.summary(),
            "areas": areas,
            "unpushed_branches": self.unpushed.iter().map(|b| json!({
                "branch": b.name, "ahead": b.ahead, "behind": b.behind, "has_upstream": b.has_upstream,
            })).collect::<Vec<_>>(),
            "pull_requests": self.pull_requests.iter().map(|p| json!({
                "number": p.number, "title": p.title, "branch": p.branch, "draft": p.is_draft, "updated": p.updated,
            })).collect::<Vec<_>>(),
            "worktrees": self.worktrees.iter().map(|w| json!({
                "path": w.path, "branch": w.branch, "dirty_files": w.dirty_files, "missing": w.missing,
            })).collect::<Vec<_>>(),
        })
    }

    /// One line per area, for the panel.
    pub fn lines(&self) -> Vec<(String, String, &'static str)> {
        let mut out: Vec<(String, String, &'static str)> = Vec::new();
        let breaking = self.issues.iter().filter(|i| i.severity == "break").count();
        out.push((
            "GitHub".into(),
            if self.gh_authenticated {
                format!("{} · {} PR open", self.gh_account.clone().unwrap_or_default(), self.pull_requests.len())
            } else {
                "not authenticated".into()
            },
            if self.gh_authenticated { "ready" } else { "broken" },
        ));
        out.push((
            "Unpushed".into(),
            format!("{} commits on {} branches", self.unpushed.iter().map(|b| b.ahead).sum::<u64>(), self.unpushed.len()),
            if self.unpushed.is_empty() { "ready" } else { "attention" },
        ));
        out.push((
            "Behind origin".into(),
            format!("{} branches behind", self.behind_branches()),
            if self.behind_branches() == 0 { "ready" } else { "attention" },
        ));
        let dirty = self.worktrees.iter().filter(|w| w.dirty_files > 0).count();
        let stale = self.worktrees.iter().filter(|w| w.missing).count();
        out.push((
            "Worktrees".into(),
            format!("{} total · {} stale · {} dirty", self.worktrees.len(), stale, dirty),
            if stale > 0 { "attention" } else { "ready" },
        ));
        out.push((
            "Issues".into(),
            format!("{breaking} breaking · {}", self.issues.len()),
            if breaking > 0 { "broken" } else { "ready" },
        ));
        out
    }

    fn behind_branches(&self) -> usize {
        self.unpushed.iter().filter(|b| b.behind > 0).count()
    }
}

/// Read the repository's GitHub health. Never writes, pushes, closes or deletes.
pub fn inspect(repo: &Path) -> Health {
    let mut health = Health {
        remote: git(repo, &["remote", "get-url", "origin"]).unwrap_or_default(),
        branch: git(repo, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default(),
        head_sha: git(repo, &["rev-parse", "--short", "HEAD"]).unwrap_or_default(),
        ..Health::default()
    };
    let (ahead, behind) = ahead_behind(repo);
    health.ahead = ahead;
    health.behind = behind;

    health.unpushed = unpushed_branches(repo);
    health.local_branches = git(repo, &["for-each-ref", "--format=%(refname:short)", "refs/heads"])
        .map(|out| out.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0);
    health.worktrees = worktrees(repo);
    health.stale_worktrees = health.worktrees.iter().filter(|w| w.missing).count();
    health.dirty_worktrees = health.worktrees.iter().filter(|w| w.dirty_files > 0).count();

    let (authenticated, account) = gh_auth(repo);
    health.gh_authenticated = authenticated;
    health.gh_account = account;
    health.pull_requests = pull_requests(repo);

    health.issues = issues(&health);
    health
}

/// Commits on the current branch that the upstream does not have.
fn ahead_behind(repo: &Path) -> (u64, u64) {
    let counts = git(repo, &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"]).unwrap_or_default();
    let mut parts = counts.split_whitespace();
    let ahead = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    let behind = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    (ahead, behind)
}

/// Local branches whose commits the remote does not have.
fn unpushed_branches(repo: &Path) -> Vec<Branch> {
    let Some(out) = git(repo, &["for-each-ref", "--format=%(refname:short)\t%(upstream:track)\t%(upstream:short)", "refs/heads"]) else {
        return Vec::new();
    };
    let mut branches = Vec::new();
    for line in out.lines() {
        let mut parts = line.split('\t');
        let Some(name) = parts.next().filter(|n| !n.trim().is_empty()) else { continue; };
        let track = parts.next().unwrap_or("");
        let upstream = parts.next().unwrap_or("");
        let ahead = track
            .split(',')
            .map(|t| t.trim())
            .find_map(|t| t.strip_prefix("ahead "))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let behind = track
            .split(',')
            .map(|t| t.trim())
            .find_map(|t| t.strip_prefix("behind "))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        if ahead == 0 && behind == 0 {
            continue;
        }
        branches.push(Branch {
            name: name.to_owned(),
            ahead,
            behind,
            has_upstream: !upstream.trim().is_empty(),
        });
    }
    branches.sort_by(|a, b| b.ahead.cmp(&a.ahead).then(a.name.cmp(&b.name)));
    branches
}

/// Every worktree, with how much unsaved work it holds.
fn worktrees(repo: &Path) -> Vec<Worktree> {
    let Some(out) = git(repo, &["worktree", "list", "--porcelain"]) else { return Vec::new() };
    let mut out_list: Vec<Worktree> = Vec::new();
    let mut path: Option<PathBuf> = None;
    let mut branch: Option<String> = None;
    for line in out.lines() {
        if let Some(value) = line.strip_prefix("worktree ") {
            if let Some(previous) = path.take() {
                out_list.push(Worktree { path: previous, branch: branch.take(), dirty_files: 0, missing: false });
            }
            path = Some(PathBuf::from(value));
        } else if let Some(value) = line.strip_prefix("branch ") {
            branch = Some(value.strip_prefix("refs/heads/").unwrap_or(value).to_owned());
        } else if line.starts_with("prunable") || line.starts_with("detached") && line.contains("prunable") {
            if let Some(previous) = path.take() {
                out_list.push(Worktree { path: previous, branch: branch.take(), dirty_files: 0, missing: true });
            }
        }
    }
    if let Some(previous) = path {
        out_list.push(Worktree { path: previous, branch, dirty_files: 0, missing: false });
    }
    for worktree in &mut out_list {
        worktree.missing = !worktree.path.is_dir();
        if worktree.missing {
            continue;
        }
        worktree.dirty_files = git(&worktree.path, &["status", "--porcelain"])
            .map(|text| text.lines().filter(|l| !l.trim().is_empty()).count())
            .unwrap_or(0);
    }
    out_list
}

/// Whether `gh` can talk to GitHub, and as whom.
/// `gh auth status` writes its report to stderr, so both streams are read.
fn gh_auth(repo: &Path) -> (bool, Option<String>) {
    let Ok(output) = Command::new("gh")
        .args(["auth", "status", "--hostname", "github.com"])
        .current_dir(repo)
        .output()
    else {
        return (false, None);
    };
    if !output.status.success() {
        return (false, None);
    }
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    // `gh auth status` names the account in "Logged in to github.com account <name>";
    // the "Active account: true" line below it only says which one is active.
    let account = text
        .lines()
        .find_map(|l| {
            // The line carries a checkmark: "✓ Logged in to github.com account <name>".
            let rest = l.split("Logged in to ").nth(1)?;
            let name = rest.split(" account ").nth(1)?;
            let name = name.split_whitespace().next().unwrap_or("");
            (!name.is_empty()).then(|| name.to_owned())
        })
        .filter(|v| !v.is_empty());
    (true, account)
}

/// Open pull requests against this repository.
fn pull_requests(repo: &Path) -> Vec<PullRequest> {
    let Some(slug) = repository_slug(repo) else { return Vec::new(); };
    let Some(out) = run(repo, "gh", &["pr", "list", "--repo", &slug, "--state", "open", "--limit", "50", "--json", "number,title,headRefName,isDraft,updatedAt"]) else {
        return Vec::new();
    };
    let Ok(doc) = serde_json::from_str::<Value>(&out) else { return Vec::new(); };
    doc.as_array()
        .into_iter()
        .flatten()
        .filter_map(|pr| {
            Some(PullRequest {
                number: pr["number"].as_u64()?,
                title: pr["title"].as_str().unwrap_or_default().to_owned(),
                branch: pr["headRefName"].as_str().unwrap_or_default().to_owned(),
                is_draft: pr["isDraft"].as_bool().unwrap_or(false),
                updated: pr["updatedAt"].as_str().unwrap_or_default().to_owned(),
            })
        })
        .collect()
}

/// What is broken, worst first.
fn issues(health: &Health) -> Vec<Issue> {
    let mut issues = Vec::new();
    if !health.gh_authenticated {
        issues.push(Issue {
            severity: "break",
            area: "GitHub".into(),
            detail: "gh is not authenticated; pull requests cannot be seen or closed. Run `gh auth login`.".into(),
        });
    }
    for branch in health.unpushed.iter().filter(|b| b.ahead > 0) {
        issues.push(Issue {
            severity: "break",
            area: "Unpushed commits".into(),
            detail: format!("{} is {} commit(s) ahead of its upstream; that work exists only on this machine.", branch.name, branch.ahead),
        });
    }
    for branch in health.unpushed.iter().filter(|b| b.behind > 0) {
        issues.push(Issue {
            severity: "warn",
            area: "Behind upstream".into(),
            detail: format!("{} is {} commit(s) behind its upstream; merging it needs a rebase or a merge from the remote.", branch.name, branch.behind),
        });
    }
    for worktree in health.worktrees.iter().filter(|w| w.missing) {
        issues.push(Issue {
            severity: "warn",
            area: "Stale worktree".into(),
            detail: format!("{} is registered but its folder is gone; `git worktree prune` clears it.", worktree.path.display()),
        });
    }
    for worktree in health.worktrees.iter().filter(|w| w.dirty_files > 0) {
        issues.push(Issue {
            severity: "warn",
            area: "Unsaved work".into(),
            detail: format!("{} holds {} uncommitted file(s).", worktree.path.display(), worktree.dirty_files),
        });
    }
    if health.ahead > 0 {
        issues.push(Issue {
            severity: "break",
            area: "Unpushed commits".into(),
            detail: format!("{} is {} commit(s) ahead of origin; push it before the work is lost.", health.branch, health.ahead),
        });
    }
    if health.behind > 0 {
        issues.push(Issue {
            severity: "warn",
            area: "Behind upstream".into(),
            detail: format!("{} is {} commit(s) behind origin; pull before pushing.", health.branch, health.behind),
        });
    }
    for pr in &health.pull_requests {
        issues.push(Issue {
            severity: "note",
            area: "Open pull request".into(),
            detail: format!("#{} {} ({})", pr.number, pr.title, if pr.is_draft { "draft" } else { "ready" }),
        });
    }
    issues
}

/// The `owner/name` the origin remote points at, so `gh` is asked about this
/// repository rather than whatever it guesses from the working directory.
fn repository_slug(repo: &Path) -> Option<String> {
    let url = git(repo, &["remote", "get-url", "origin"])?;
    let trimmed = url.trim().trim_end_matches(".git");
    let slug = trimmed
        .rsplit_once([':', '/'])
        .map(|(_, name)| name.to_owned())
        .filter(|s| s.contains('/'));
    slug
}

/// Run `git` in `repo` and return its standard output.
fn git(repo: &Path, args: &[&str]) -> Option<String> {
    run(repo, "git", args)
}

/// Run a command in `repo` and return its standard output on success.
fn run(repo: &Path, program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).current_dir(repo).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The repository root that contains `dir`.
pub fn repo_root(dir: &Path) -> PathBuf {
    dir.ancestors()
        .find(|p| p.join(".git").exists())
        .unwrap_or(dir)
        .to_path_buf()
}

/// A short label for a worktree path: its folder name.
pub fn worktree_label(path: &Path) -> String {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_owned()
}

/// Whether a file is inside a git repository.
pub fn is_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// The herdr pane whose working directory is `dir`, if an agent lives there.
/// This is how a lost branch finds the agent that owns it.
pub fn agent_pane_for(dir: &Path) -> Option<String> {
    let output = Command::new("herdr").args(["pane", "list"]).output().ok()?;
    let doc = serde_json::from_slice::<Value>(&output.stdout).ok()?;
    let panes = doc["result"]["panes"].as_array()?;
    let wanted = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    for pane in panes {
        let cwd = pane["cwd"].as_str().map(PathBuf::from).or_else(|| pane["foreground_cwd"].as_str().map(PathBuf::from))?;
        if cwd.canonicalize().unwrap_or(cwd) == wanted {
            if let Some(id) = pane["pane_id"].as_str() {
                return Some(id.to_owned());
            }
        }
    }
    None
}

/// A branch that is ahead of its upstream, with the agent pane that owns it when known.
#[derive(Clone, Debug, PartialEq)]
pub struct Recovery {
    pub branch: String,
    pub ahead: u64,
    pub worktree: Option<PathBuf>,
    pub pane: Option<String>,
    pub dirty_files: usize,
}

/// Everything an agent could be holding that is not on the remote, and who owns it.
/// This is the list that gets lost when a machine runs out of memory.
pub fn recovery(repo: &Path) -> Vec<Recovery> {
    let worktrees = worktrees(repo);
    let mut out = Vec::new();
    for branch in unpushed_branches(repo) {
        if branch.ahead == 0 {
            continue;
        }
        let worktree = worktrees
            .iter()
            .find(|w| w.branch.as_deref() == Some(branch.name.as_str()))
            .map(|w| w.path.clone());
        let pane = worktree.as_deref().and_then(agent_pane_for);
        let dirty_files = worktree
            .as_deref()
            .and_then(|p| git(p, &["status", "--porcelain"]))
            .map(|text| text.lines().filter(|l| !l.trim().is_empty()).count())
            .unwrap_or(0);
        out.push(Recovery {
            branch: branch.name,
            ahead: branch.ahead,
            worktree,
            pane,
            dirty_files,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clean_health_reports_no_breaking_issue() {
        let mut health = Health {
            branch: "main".into(),
            head_sha: "abc1234".into(),
            remote: "git@example:o/r.git".into(),
            gh_authenticated: true,
            gh_account: Some("octocat".into()),
            ..Health::default()
        };
        health.issues = issues(&health);
        assert!(health.issues.iter().all(|i| i.severity != "break"), "{:?}", health.issues);
        assert_eq!(health.summary()["gh_authenticated"], json!(true));
        assert_eq!(health.summary()["unpushed_branches"], json!(0));
    }

    #[test]
    fn an_unauthenticated_gh_is_a_breaking_issue_with_a_fix() {
        let health = Health { gh_authenticated: false, ..Health::default() };
        let issues = issues(&health);
        let gh = issues.iter().find(|i| i.area == "GitHub").expect("gh issue");
        assert_eq!(gh.severity, "break");
        assert!(gh.detail.contains("gh auth login"), "the fix is named: {}", gh.detail);
    }

    #[test]
    fn unpushed_commits_are_breaking_and_stale_worktrees_are_a_warning() {
        let health = Health {
            branch: "feature/x".into(),
            ahead: 3,
            unpushed: vec![Branch { name: "feature/x".into(), ahead: 3, behind: 0, has_upstream: true }],
            worktrees: vec![Worktree { path: PathBuf::from("/gone"), branch: Some("old".into()), dirty_files: 0, missing: true }],
            gh_authenticated: true,
            ..Health::default()
        };
        let issues = issues(&health);
        let unpushed = issues.iter().find(|i| i.area == "Unpushed commits").expect("unpushed issue");
        assert_eq!(unpushed.severity, "break");
        assert!(unpushed.detail.contains("feature/x") && unpushed.detail.contains('3'), "{}", unpushed.detail);
        let stale = issues.iter().find(|i| i.area == "Stale worktree").expect("stale issue");
        assert_eq!(stale.severity, "warn");
        assert!(stale.detail.contains("git worktree prune"), "the fix is named: {}", stale.detail);
    }

    #[test]
    fn a_branch_behind_its_upstream_is_named_with_the_distance() {
        let health = Health {
            unpushed: vec![Branch { name: "old".into(), ahead: 0, behind: 159, has_upstream: true }],
            gh_authenticated: true,
            ..Health::default()
        };
        let issues = issues(&health);
        let behind = issues.iter().find(|i| i.area == "Behind upstream").expect("behind issue");
        assert!(behind.detail.contains("159"), "{}", behind.detail);
        assert_eq!(behind.severity, "warn");
    }

    #[test]
    fn the_summary_counts_breaking_issues_and_unpushed_commits() {
        let health = Health {
            unpushed: vec![
                Branch { name: "a".into(), ahead: 3, behind: 0, has_upstream: true },
                Branch { name: "b".into(), ahead: 5, behind: 1, has_upstream: true },
            ],
            gh_authenticated: true,
            ..Health::default()
        };
        let summary = health.summary();
        assert_eq!(summary["unpushed_commits"], json!(8));
        assert_eq!(summary["unpushed_branches"], json!(2));
    }

    #[test]
    fn the_lines_name_each_area_and_its_count() {
        let health = Health {
            gh_authenticated: true,
            gh_account: Some("octocat".into()),
            unpushed: vec![Branch { name: "a".into(), ahead: 2, behind: 0, has_upstream: true }],
            worktrees: vec![
                Worktree { path: PathBuf::from("/a"), branch: None, dirty_files: 3, missing: false },
                Worktree { path: PathBuf::from("/b"), branch: None, dirty_files: 0, missing: true },
            ],
            ..Health::default()
        };
        let lines = health.lines();
        assert!(lines.iter().any(|(area, value, _)| area == "Unpushed" && value.contains("2 commits")), "{lines:?}");
        assert!(lines.iter().any(|(area, value, _)| area == "Worktrees" && value.contains("1 stale")), "{lines:?}");
        assert!(lines.iter().any(|(area, _, state)| area == "GitHub" && *state == "ready"), "{lines:?}");
    }

    #[test]
    fn a_success_sparkles_and_an_error_sirens() {
        // A finished job twinkles.
        assert_eq!(Mood::for_kind("render_finished", true), Mood::Sparkle);
        assert_eq!(Mood::for_kind("generation_finished", true), Mood::Sparkle);
        assert_eq!(Mood::for_kind("snapshot_finished", true), Mood::Sparkle);
        // An error sirens.
        assert_eq!(Mood::for_kind("error", false), Mood::Siren);
        // A job that claims to have finished but failed still sirens.
        assert_eq!(Mood::for_kind("render_finished", false), Mood::Siren);
        // Ordinary events stay quiet: a save or a selection is not a celebration.
        assert_eq!(Mood::for_kind("saved", true), Mood::Quiet);
        assert_eq!(Mood::for_kind("selection_changed", true), Mood::Quiet);
    }

    #[test]
    fn a_mood_animates_and_keeps_its_shape() {
        assert!(Mood::Sparkle.frames().len() > 1, "sparkles animate");
        assert!(Mood::Siren.frames().len() > 1, "the siren waves");
        assert_eq!(Mood::Quiet.frames().len(), 1, "quiet does not animate");
        // The same tick always draws the same shape, so a redraw is stable.
        assert_eq!(Mood::Sparkle.frame(3), Mood::Sparkle.frame(3));
        assert_ne!(Mood::Sparkle.frame(0), Mood::Sparkle.frame(1), "the sparkle moves");
    }

    #[test]
    fn only_the_loud_moods_have_a_sound() {
        assert_eq!(Mood::Sparkle.sound(), Some("sparkle"));
        assert_eq!(Mood::Siren.sound(), Some("siren"));
        assert_eq!(Mood::Quiet.sound(), None, "quiet has nothing to play");
    }

    #[test]
    fn a_fanfare_carries_the_label_and_detail_it_rings_for() {
        let f = Fanfare::for_event("render_finished", true, "render finished", "/tmp/out.mp4");
        assert_eq!(f.mood, Mood::Sparkle);
        assert_eq!(f.label, "render finished");
        assert_eq!(f.detail, "/tmp/out.mp4");
        let failed = Fanfare::for_event("error", false, "render failed", "no output file appeared");
        assert_eq!(failed.mood, Mood::Siren);
    }

    #[test]
    fn the_repository_slug_comes_from_the_origin_remote() {
        assert_eq!(repository_slug(Path::new("/repo")).as_deref(), None, "no remote, no slug");
    }

    #[test]
    fn recovery_lists_only_branches_with_unpushed_commits() {
        let dir = std::env::temp_dir().join(format!("mc-gh-recovery-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // A branch with no upstream has no track, so it is not "ahead of an upstream".
        assert!(recovery(&dir).is_empty(), "an empty directory has nothing to recover");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_repository_is_recognised_by_its_git_directory() {
        let dir = std::env::temp_dir().join(format!("mc-gh-health-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!is_repo(&dir));
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        assert!(is_repo(&dir));
        assert_eq!(repo_root(&dir), dir);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
