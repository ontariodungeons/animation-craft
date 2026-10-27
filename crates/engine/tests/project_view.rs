#[test]
fn project_view_command_and_selection_regressions() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for file in ["app.js", "project.js", "song.js", "storyboard.js", "feedback.js", "menu.js"] {
        let output = std::process::Command::new("node").arg("--check")
            .arg(root.join("apps/magicstudio-shell/ui").join(file)).output().unwrap();
        assert!(output.status.success(), "{file}: {}", String::from_utf8_lossy(&output.stderr));
    }
    let output = std::process::Command::new("node")
        .arg("--test").arg(root.join("apps/magicstudio-shell/tests/project-view.cjs"))
        .output().expect("Node is required for the embedded Project view checks");
    assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
}
