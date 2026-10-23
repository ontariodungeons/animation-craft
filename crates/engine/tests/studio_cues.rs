//! `studio.cues` runs the Bun window sampler as a child process. It lives in its own test binary
//! because a child spawned beside the lib tests' concurrent project-lease claims can hold their
//! lock descriptors and fail those tests.
use magicstudio_engine::Session;
use serde_json::json;

#[test]
fn cues_at_time_and_clip_uses_read_composition_and_timeline() {
    let dir = std::env::temp_dir().join(format!("ac-studio-cues-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("hyperframes")).unwrap();
    std::fs::create_dir_all(dir.join("artifacts")).unwrap();
    std::fs::write(dir.join("Song.magic"), "{\"assets\":[]}").unwrap();
    std::fs::write(dir.join("hyperframes/index.html"), concat!(
        "<div id=\"root\" data-composition-id=\"main\" data-start=\"0\" data-duration=\"20\">",
        "<img id=\"sea\" class=\"clip\" data-start=\"0\" data-duration=\"10\" src=\"sea.png\" aria-label=\"a > b\">",
        "<img id=\"sea-again\" class=\"clip\" data-start=\"12\" data-duration=\"4\" src=\"sea.png\">",
        "<img id=\"boat\" class=\"clip\" data-start=\"0\" data-duration=\"20\" src=\"boat.png\">",
        "<img id=\"bed\" class=\"clip\" data-start=\"0\" data-duration=\"20\" src=\"bed.png\">",
        "<div id=\"group\"><img id=\"hat\" class=\"clip\" data-start=\"0\" data-duration=\"20\" src=\"hat.png\"></div>",
        "<img id=\"cat\" class=\"clip\" data-start=\"0\" data-duration=\"20\" src=\"cat.png\">",
        "<script src=\"film.js\"></script>",
        "<div id=\"word\" class=\"clip lyric-word-clip\" data-start=\"4\" data-duration=\"1\"></div>",
        "<audio id=\"song\" data-start=\"0\" data-duration=\"20\" src=\"song.wav\"></audio></div>",
    )).unwrap();
    // Boat shows twice, bed never, hat through its parent group; cat is tweened by GSAP.
    std::fs::write(dir.join("hyperframes/film.js"), concat!(
        "const $=id=>document.getElementById(id);gsap.timeline({paused:true}).to('#cat',{opacity:0});\n",
        "window.renderAt=t=>{$('boat').style.opacity=(t>=2&&t<6)||t>=8&&t<9?1:0;$('bed').style.opacity=0;",
        "$('group').style.opacity=t<3?0:.5;};",
    )).unwrap();
    std::fs::write(dir.join("artifacts/action_timeline.json"), r#"{"scenes":[
        {"scene_id":"s01","start_seconds":0,"end_seconds":12,"actions":[{"at_seconds":3,"duration_seconds":2,"character_id":"fish","action":"swim"}]},
        {"scene_id":"s02","start_seconds":12,"end_seconds":20,"actions":[]}]}"#).unwrap();
    let mut s = Session::default();
    let project = dir.join("Song.magic");

    let at = s.execute("studio.cues", json!({"project": &project, "at": 4.5})).unwrap();
    assert_eq!(at["scene"]["id"], "s01");
    let cues = at["cues"].as_array().unwrap();
    let ids: Vec<_> = cues.iter().map(|c| c["id"].as_str().or(c["character"].as_str()).unwrap()).collect();
    let timing = |id: &str| cues.iter().find(|c| c["id"] == id).map(|c| c["timing"].as_str().unwrap().to_string());
    assert_eq!(cues.last().unwrap()["local"], json!(1.5), "fish action is 1.5 s in");
    if at["timing_source"]["ok"] == json!(true) {
        // Word captions, root and audio are not clips; bed is never shown.
        assert_eq!(ids, ["sea", "boat", "hat", "cat", "fish"]);
        let boat = &cues[1];
        assert_eq!((boat["start"].as_f64(), boat["end"].as_f64(), boat["local"].as_f64()), (Some(2.0), Some(6.0), Some(2.5)));
        assert_eq!(cues[2]["start"], json!(3.0), "hat inherits its parent's opacity");
        assert_eq!(timing("cat").unwrap(), "unresolved (GSAP tween on #cat; composition times)");
        let boat = s.execute("studio.cues", json!({"project": &project, "clip": "boat"})).unwrap();
        assert_eq!(boat["uses"].as_array().unwrap().iter().map(|u| (u["start"].as_f64().unwrap(), u["end"].as_f64().unwrap())).collect::<Vec<_>>(), [(2.0, 6.0), (8.0, 9.0)]);
        let bed = s.execute("studio.cues", json!({"project": &project, "clip": "bed"})).unwrap();
        assert_eq!(bed["uses"][0]["timing"], "never shown");
    } else {
        // Without Bun every clip keeps its composition times and says so.
        assert_eq!(ids, ["sea", "boat", "bed", "hat", "cat", "fish"]);
        assert!(cues.iter().filter(|c| c["kind"] == "clip").all(|c| c["timing"] == "unresolved (sampler unavailable; composition times)"));
    }

    let uses = s.execute("studio.cues", json!({"project": &project, "clip": "sea"})).unwrap();
    assert_eq!(uses["uses"].as_array().unwrap().len(), 2);
    assert_eq!(uses["uses"][1]["scenes"], json!(["s02"]));
    assert_eq!(uses["uses"][0]["fit"], json!({"under_run": 2.0, "scene": "s01"}));
    assert_eq!(uses["uses"][1]["fit"], json!({"under_run": 4.0, "scene": "s02"}));
    assert!(s.execute("studio.cues", json!({"project": &project, "clip": "missing"})).is_err());
    assert!(s.execute("studio.cues", json!({"project": &project})).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
