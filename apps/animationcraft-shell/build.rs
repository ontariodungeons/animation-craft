fn main() {
    let revision=std::process::Command::new("git").args(["rev-parse","--short","HEAD"]).output().ok().and_then(|o|String::from_utf8(o.stdout).ok()).unwrap_or_else(||"unknown".into());
    let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d|d.as_secs()).unwrap_or(0);
    println!("cargo:rustc-env=ANIMATIONCRAFT_BUILD_ID={}-{}",revision.trim(),now);
    println!("cargo:rerun-if-changed=ui");
    tauri_build::build();
}
