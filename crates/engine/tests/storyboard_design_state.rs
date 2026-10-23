//! Synthetic metadata fixtures only; actual native save/inspection is verified through the CLIs.
use magicstudio_engine::Session;
use serde_json::json;
use std::{fs, path::PathBuf};

fn fixture(name: &str) -> (Session, PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("ac-design-state-{}-{name}", std::process::id()));
    fs::create_dir_all(dir.join("storyboard")).unwrap();
    let project = dir.join("Fixture.magic");
    let mut session = Session::default();
    session
        .execute("project.new", json!({"path":project,"overwrite":true}))
        .unwrap();
    let deck = dir.join("storyboard/saved/movie-board.deckcraft");
    let board = json!({"deck":deck,"pdf":dir.join("storyboard/saved/movie-board.pdf"),"first_scene_page":4,
        "design":{"dirty":true,"placements":[{"id":"saved-image","page":4,"asset":"fixture","rect":[10,20,30,40]}]},
        "validation":{"pages":[{"kind":"toc"},{"kind":"overview"},{"kind":"lyrics"},{"kind":"scene"},{"kind":"qc"}]}});
    fs::write(dir.join("storyboard/movie-board.json"), board.to_string()).unwrap();
    fs::write(
        dir.join("storyboard/design-draft.json"),
        json!({"dirty":false,"placements":[]}).to_string(),
    )
    .unwrap();
    (session, dir, project)
}

#[test]
fn saved_snapshot_dirty_flag_does_not_report_unsaved_work() {
    let (mut session, dir, project) = fixture("saved");
    let active = dir.join("storyboard/movie-board.json");
    let before = fs::read(&active).unwrap();
    let saved = session.execute("storyboard.design", json!({})).unwrap();
    assert_eq!(saved["dirty"], false);
    assert_eq!(saved["placements"][0]["id"], "saved-image");
    assert_eq!(
        saved["deck"],
        dir.join("storyboard/saved/movie-board.deckcraft")
            .to_string_lossy()
            .as_ref()
    );
    session.execute("project.save", json!({})).unwrap();
    session
        .execute("project.open", json!({"path":project}))
        .unwrap();
    assert_eq!(
        session.execute("storyboard.design", json!({})).unwrap()["dirty"],
        false
    );
    assert_eq!(
        fs::read(active).unwrap(),
        before,
        "inspection must not rewrite legacy saved metadata"
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn pending_removal_stays_dirty_across_project_save_and_reopen() {
    let (mut session, dir, project) = fixture("pending");
    let draft = dir.join("storyboard/design-draft.json");
    fs::write(&draft, json!({"dirty":true,"placements":[]}).to_string()).unwrap();
    let bytes = fs::read(&draft).unwrap();
    session.execute("project.save", json!({})).unwrap();
    session
        .execute("project.open", json!({"path":project}))
        .unwrap();
    let pending = session.execute("storyboard.design", json!({})).unwrap();
    assert_eq!(pending["dirty"], true);
    assert_eq!(pending["placements"], json!([]));
    assert_eq!(
        fs::read(draft).unwrap(),
        bytes,
        "never force-clear a real draft"
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn legacy_pending_placements_remain_unsaved_without_a_dirty_field() {
    let (mut session, dir, _) = fixture("legacy-pending");
    let draft = dir.join("storyboard/design-draft.json");
    let pending = json!({"placements":[{"id":"pending-image","page":4,"asset":"fixture","rect":[20,20,30,40]}]});
    fs::write(&draft, pending.to_string()).unwrap();
    let before = fs::read(&draft).unwrap();
    let result = session.execute("storyboard.design", json!({})).unwrap();
    assert_eq!(result["dirty"], true);
    assert_eq!(result["placements"][0]["id"], "pending-image");
    assert_eq!(fs::read(draft).unwrap(), before, "inspection must not clear a legacy pending draft");
    fs::remove_dir_all(dir).unwrap();
}
