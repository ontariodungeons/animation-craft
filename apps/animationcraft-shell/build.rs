fn main() {
    let revision=std::process::Command::new("git").args(["rev-parse","--short","HEAD"]).output().ok().and_then(|o|String::from_utf8(o.stdout).ok()).unwrap_or_else(||"unknown".into());
    let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d|d.as_secs()).unwrap_or(0);
    println!("cargo:rustc-env=ANIMATIONCRAFT_BUILD_ID={}-{}",revision.trim(),now);
    // Recompute runtime identity after local commits, including named worktrees.
    let branch=std::process::Command::new("git").args(["symbolic-ref","-q","HEAD"]).output().ok().filter(|o|o.status.success()).and_then(|o|String::from_utf8(o.stdout).ok()).unwrap_or_default();
    for name in ["HEAD", "packed-refs", branch.trim()].into_iter().filter(|p|!p.is_empty()) {
        if let Ok(output)=std::process::Command::new("git").args(["rev-parse","--git-path",name]).output(){if output.status.success(){println!("cargo:rerun-if-changed={}",String::from_utf8_lossy(&output.stdout).trim());}}
    }
    println!("cargo:rerun-if-changed=ui");
    tauri_build::build();
}
