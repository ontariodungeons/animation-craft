fn main() {
    let revision=std::process::Command::new("git").args(["rev-parse","--short","HEAD"]).output().ok().and_then(|o|String::from_utf8(o.stdout).ok()).unwrap_or_else(||"unknown".into());
    let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d|d.as_secs()).unwrap_or(0);
    println!("cargo:rustc-env=MAGICSTUDIO_BUILD_ID={}-{}",revision.trim(),now);
    // Recompute runtime identity after local commits, including named worktrees.
    let branch=std::process::Command::new("git").args(["symbolic-ref","-q","HEAD"]).output().ok().filter(|o|o.status.success()).and_then(|o|String::from_utf8(o.stdout).ok()).unwrap_or_default();
    println!("cargo:rustc-env=MAGICSTUDIO_BUILD_BRANCH={}",branch.trim().strip_prefix("refs/heads/").unwrap_or("detached"));
    for name in ["HEAD", "packed-refs", branch.trim()].into_iter().filter(|p|!p.is_empty()) {
        if let Ok(output)=std::process::Command::new("git").args(["rev-parse","--git-path",name]).output(){if output.status.success(){println!("cargo:rerun-if-changed={}",String::from_utf8_lossy(&output.stdout).trim());}}
    }
    println!("cargo:rerun-if-changed=ui");
    theatre_bundle();
    tauri_build::build();
}

/// ui/vendor/theatre.bundle.js is generated (gitignored) from frontend/, so a fresh checkout
/// builds it here instead of needing a manual `bun run build`. It is rebuilt only when missing
/// or older than its sources, which keeps the `ui` change watch above from looping.
fn theatre_bundle() {
    let sources = ["frontend/theatre-frame.js", "frontend/project-id.mjs", "frontend/package.json", "frontend/bun.lock", "frontend/learning-editor.js", "frontend/learning-markdown.mjs"];
    for source in sources { println!("cargo:rerun-if-changed={source}"); }
    let modified = |path: &str| std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let bundle = modified("ui/vendor/theatre.bundle.js");
    if bundle.is_some() && modified("ui/vendor/learning/learning-editor.js").is_some() && sources.iter().all(|s| modified(s) <= bundle && modified(s) <= modified("ui/vendor/learning/learning-editor.js")) { return; }
    let bun = |args: &[&str]| {
        let status = std::process::Command::new("bun").args(args).current_dir("frontend").status()
            .unwrap_or_else(|e| panic!("Building the Theatre panel needs Bun on PATH: {e}"));
        assert!(status.success(), "`bun {}` failed in apps/magicstudio-shell/frontend", args.join(" "));
    };
    if !std::path::Path::new("frontend/node_modules/@theatre/studio").exists() || !std::path::Path::new("frontend/node_modules/@milkdown/crepe").exists() { bun(&["install"]); }
    bun(&["run", "build"]);
}
