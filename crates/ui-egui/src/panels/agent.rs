//! The agent pane: an opencode session (through `opencode api`: the user's own subscription,
//! models and MCP servers) docked on the right, and pointing: click any element to attach its
//! picture, id and label, then send to that session or to an agent in Herdr (`herdr agent prompt`).
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use egui::{Color32, RichText, Sense, Ui};
use serde_json::{Value, json};

use super::chrome::{self, MUTED, TEXT};
use crate::AnimationCraftApp;

/// Target value for the opencode session (any other target is a Herdr agent name or pane id).
const SESSION: &str = "session";

#[derive(Default)]
struct Shared {
    /// Session messages, oldest first.
    messages: Vec<Value>,
    permissions: Vec<Value>,
    /// Questions the agent is waiting on (opencode forms).
    forms: Vec<Value>,
    /// (providerID, id, name)
    models: Vec<(String, String, String)>,
    /// (target, label) of agents running in Herdr panes.
    herdr: Vec<(String, String)>,
    session: Option<String>,
    status: String,
    polling: bool,
}

#[derive(Default)]
pub(crate) struct AgentPane {
    pub(crate) open: bool,
    pub(crate) pointing: bool,
    input: String,
    target: String,
    shared: Arc<Mutex<Shared>>,
    last_poll: Option<Instant>,
    loaded: bool,
    /// Pictures to send with the next message: (file, what it shows).
    attachments: Vec<(PathBuf, String)>,
    /// Typed answers to agent questions, by form id and field key.
    answers: std::collections::HashMap<String, String>,
    /// A pointing capture in flight: (reply, what it shows, file).
    point_rx: Option<(Receiver<Result<Value, String>>, String, PathBuf)>,
}

/// `opencode api <args…> [-d data]` against the user's opencode background service.
fn opencode(cwd: &Path, args: &[String], data: Option<&Value>) -> Result<Value, String> {
    let mut cmd = std::process::Command::new("opencode");
    cmd.current_dir(cwd).arg("api").args(args);
    if let Some(d) = data {
        cmd.arg("-d").arg(d.to_string());
    }
    let out = cmd.output().map_err(|e| format!("opencode: {e}"))?;
    if !out.status.success() {
        return Err(format!("opencode {}: {}", args.first().map_or("", String::as_str), String::from_utf8_lossy(&out.stderr).trim()));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&text).map_err(|e| format!("opencode reply: {e}"))
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// The project folder (where the session works and pictures are kept), else the home folder.
fn workdir(app: &AnimationCraftApp) -> PathBuf {
    app.session
        .project_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into())))
}

/// The session id kept beside the project, so the conversation survives a restart.
fn session_file(dir: &Path) -> PathBuf {
    dir.join(".agent").join("opencode-session")
}

impl AgentPane {
    fn spawn(&self, job: impl FnOnce(&Arc<Mutex<Shared>>) + Send + 'static) {
        let shared = self.shared.clone();
        std::thread::spawn(move || job(&shared));
    }

    fn set_status(&self, s: String) {
        if let Ok(mut sh) = self.shared.lock() {
            sh.status = s;
        }
    }

    /// Models from opencode and agents from Herdr, once per opening.
    fn load(&mut self, dir: &Path) {
        self.loaded = true;
        if self.target.is_empty() {
            self.target = SESSION.into();
        }
        if let Ok(mut sh) = self.shared.lock()
            && sh.session.is_none()
        {
            sh.session = std::fs::read_to_string(session_file(dir)).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        }
        let dir = dir.to_path_buf();
        self.spawn(move |shared| {
            let models = opencode(&dir, &args(&["model.list"]), None)
                .map(|v| {
                    v["data"].as_array().into_iter().flatten()
                        .filter_map(|m| Some((m["providerID"].as_str()?.to_string(), m["id"].as_str()?.to_string(), m["name"].as_str().unwrap_or("").to_string())))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let herdr = std::process::Command::new("herdr").args(["agent", "list"]).output().ok()
                .and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok())
                .map(|v| {
                    v["result"]["agents"].as_array().into_iter().flatten().filter_map(|a| {
                        let pane = a["pane_id"].as_str()?;
                        let kind = a["agent"].as_str().unwrap_or("agent");
                        Some(match a["name"].as_str() {
                            Some(name) => (name.to_string(), format!("{name} ({kind}, Herdr)")),
                            None => (pane.to_string(), format!("{kind} in {pane} (Herdr)")),
                        })
                    }).collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if let Ok(mut sh) = shared.lock() {
                sh.models = models;
                sh.herdr = herdr;
            }
        });
    }

    /// Messages and permission requests, every two seconds while the pane is open.
    fn poll(&mut self, dir: &Path, ctx: &egui::Context) {
        let Some(session) = self.shared.lock().ok().and_then(|s| (!s.polling).then(|| s.session.clone()).flatten()) else { return };
        if self.last_poll.is_some_and(|t| t.elapsed() < Duration::from_secs(2)) {
            return;
        }
        self.last_poll = Some(Instant::now());
        if let Ok(mut sh) = self.shared.lock() {
            sh.polling = true;
        }
        let (dir, ctx) = (dir.to_path_buf(), ctx.clone());
        self.spawn(move |shared| {
            let sid = format!("sessionID={session}");
            let messages = opencode(&dir, &args(&["session.message.list", "--param", &sid, "--param", "order=desc", "--param", "limit=40"]), None);
            let permissions = opencode(&dir, &args(&["session.permission.list", "--param", &sid]), None);
            let forms = opencode(&dir, &args(&["session.form.list", "--param", &sid]), None);
            if let Ok(mut sh) = shared.lock() {
                match messages {
                    Ok(v) => sh.messages = v["data"].as_array().cloned().unwrap_or_default().into_iter().rev().collect(),
                    Err(e) => sh.status = e,
                }
                sh.permissions = permissions.ok().and_then(|v| v["data"].as_array().cloned()).unwrap_or_default();
                sh.forms = forms.ok().and_then(|v| v["data"].as_array().cloned()).unwrap_or_default();
                sh.polling = false;
            }
            ctx.request_repaint();
        });
    }

    /// Sends the input (and attached pictures) to the chosen target.
    fn send(&mut self, dir: &Path, project: String) {
        let text = self.input.trim().to_string();
        if text.is_empty() && self.attachments.is_empty() {
            return;
        }
        let attachments = std::mem::take(&mut self.attachments);
        self.input.clear();
        let dir = dir.to_path_buf();
        if self.target != SESSION {
            let target = self.target.clone();
            let mut body = text;
            for (path, what) in &attachments {
                body.push_str(&format!("\n[Pointed at {what} in AnimationCraft — picture: {}]", path.display()));
            }
            self.set_status(format!("Sending to {target} in Herdr…"));
            self.spawn(move |shared| {
                let out = std::process::Command::new("herdr").args(["agent", "prompt", &target, &body]).output();
                let status = match out {
                    Ok(o) if o.status.success() => format!("Sent to {target} in Herdr."),
                    Ok(o) => format!("Herdr: {}", String::from_utf8_lossy(&o.stderr).trim()),
                    Err(e) => format!("herdr: {e}"),
                };
                if let Ok(mut sh) = shared.lock() {
                    sh.status = status;
                }
            });
            return;
        }
        let files: Vec<Value> = attachments
            .iter()
            .map(|(p, what)| json!({"uri": format!("file://{}", p.display()), "name": p.file_name().and_then(|n| n.to_str()).unwrap_or("point.png"), "description": what}))
            .collect();
        let pointed: Vec<String> = attachments.iter().map(|(_, what)| format!("[Pointed at {what} in AnimationCraft; picture attached]")).collect();
        let text = [vec![text], pointed].concat().join("\n");
        self.last_poll = None;
        self.set_status("Sending…".into());
        self.spawn(move |shared| {
            let session = shared.lock().ok().and_then(|s| s.session.clone());
            let session = match session {
                Some(s) => s,
                None => match opencode(&dir, &args(&["session.create"]), Some(&json!({"title": format!("AnimationCraft · {project}")}))) {
                    Ok(v) => match v["data"]["id"].as_str().or(v["id"].as_str()) {
                        Some(id) => {
                            let _ = std::fs::create_dir_all(dir.join(".agent"));
                            let _ = std::fs::write(session_file(&dir), id);
                            if let Ok(mut sh) = shared.lock() {
                                sh.session = Some(id.to_string());
                            }
                            id.to_string()
                        }
                        None => return,
                    },
                    Err(e) => {
                        if let Ok(mut sh) = shared.lock() {
                            sh.status = e;
                        }
                        return;
                    }
                },
            };
            let res = opencode(&dir, &args(&["session.prompt", "--param", &format!("sessionID={session}")]), Some(&json!({"text": text, "files": files})));
            if let Ok(mut sh) = shared.lock() {
                sh.status = match res {
                    Ok(_) => "Sent. The agent is working…".into(),
                    Err(e) => e,
                };
            }
        });
    }
}

/// The header's Agent button: opens or closes the docked pane.
pub(crate) fn toggle(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let r = ui.selectable_label(app.agent.open, "Agent").on_hover_text("Chat with your opencode agent; point at anything to show it");
    app.automation.add_response("toolbar.agent", "button", "Agent pane", &r);
    if r.clicked() {
        app.agent.open = !app.agent.open;
    }
}

/// The pane itself, drawn into the strip docked on the right.
pub(crate) fn agent_pane(app: &mut AnimationCraftApp, ui: &mut Ui) {
    let dir = workdir(app);
    if !app.agent.loaded {
        app.agent.load(&dir);
    }
    app.agent.poll(&dir, ui.ctx());
    let shared = app.agent.shared.clone();
    let Ok(sh) = shared.lock() else { return };

    ui.horizontal(|ui| {
        ui.label(RichText::new("Agent").size(15.0).strong().color(TEXT));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let r = ui.small_button("Close");
            app.automation.add_response("agent.close", "button", "Close agent pane", &r);
            if r.clicked() {
                app.agent.open = false;
            }
        });
    });
    // Where the message goes: the opencode session, or an agent running in Herdr.
    let label_of = |t: &str| if t == SESSION { "opencode session (this app)".to_string() } else {
        sh.herdr.iter().find(|(id, _)| id == t).map_or(t.to_string(), |(_, l)| l.clone())
    };
    egui::ComboBox::from_id_salt("agent.target").width(ui.available_width() - 8.0).selected_text(label_of(&app.agent.target)).show_ui(ui, |ui| {
        ui.selectable_value(&mut app.agent.target, SESSION.to_string(), label_of(SESSION));
        for (id, label) in &sh.herdr {
            ui.selectable_value(&mut app.agent.target, id.clone(), label);
        }
    });
    // The session's model; anyone (you, the agent, another tool) can switch it.
    if app.agent.target == SESSION {
        let current = sh.messages.iter().rev().find_map(|m| Some((m["model"]["providerID"].as_str()?.to_string(), m["model"]["id"].as_str()?.to_string())));
        let shown = current.as_ref().and_then(|(p, id)| sh.models.iter().find(|m| &m.0 == p && &m.1 == id)).map_or_else(
            || current.as_ref().map_or("default model".to_string(), |c| c.1.clone()),
            |m| m.2.clone(),
        );
        let mut picked = None;
        egui::ComboBox::from_id_salt("agent.model").width(ui.available_width() - 8.0).selected_text(shown).show_ui(ui, |ui| {
            for (p, id, name) in &sh.models {
                if ui.selectable_label(current.as_ref() == Some(&(p.clone(), id.clone())), format!("{name}  ·  {p}")).clicked() {
                    picked = Some((p.clone(), id.clone()));
                }
            }
        });
        if let (Some((p, id)), Some(session)) = (picked, sh.session.clone()) {
            let d = dir.clone();
            app.agent.spawn(move |shared| {
                let r = opencode(&d, &args(&["session.switchModel", "--param", &format!("sessionID={session}")]), Some(&json!({"model": {"id": id, "providerID": p}})));
                if let Ok(mut sh) = shared.lock() {
                    sh.status = r.map_or_else(|e| e, |_| "Model switched.".into());
                }
            });
        }
    }
    ui.separator();

    let input_h = 96.0;
    let list_h = (ui.available_height() - input_h - 70.0).max(80.0);
    egui::ScrollArea::vertical().id_salt("agent.messages").max_height(list_h).stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
        if app.agent.target != SESSION {
            ui.label(RichText::new("Messages go to that agent's pane in Herdr; watch the reply there.").size(11.0).color(MUTED));
            return;
        }
        if sh.messages.is_empty() {
            ui.label(RichText::new("Ask the agent anything about this project. Point at something to show it what you mean.").size(11.0).color(MUTED));
        }
        for m in &sh.messages {
            match m["type"].as_str() {
                Some("user") => {
                    ui.add_space(6.0);
                    ui.label(RichText::new(m["text"].as_str().unwrap_or("")).size(12.0).color(chrome::TEAL));
                }
                Some("assistant") => {
                    for part in m["content"].as_array().into_iter().flatten() {
                        match part["type"].as_str() {
                            Some("text") => {
                                ui.label(RichText::new(part["text"].as_str().unwrap_or("")).size(12.0).color(TEXT));
                            }
                            Some(kind) if kind.contains("tool") => {
                                let name = part["name"].as_str().or(part["tool"].as_str()).unwrap_or(kind);
                                ui.label(RichText::new(format!("· {name}")).size(10.5).color(MUTED));
                            }
                            _ => {}
                        }
                    }
                }
                Some("idle") => {
                    ui.label(RichText::new("— done").size(10.0).color(MUTED));
                }
                _ => {}
            }
        }
        // Permission requests wait here for an answer.
        for p in &sh.permissions {
            let (Some(id), Some(sid)) = (p["id"].as_str(), p["sessionID"].as_str()) else { continue };
            ui.add_space(4.0);
            ui.label(RichText::new(format!("Permission: {} {}", p["action"].as_str().unwrap_or(""), p["resources"].as_array().map_or(String::new(), |r| r.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")))).size(11.5).color(Color32::from_rgb(240, 168, 60)));
            ui.horizontal(|ui| {
                for (decision, label) in [("once", "Allow once"), ("always", "Always"), ("reject", "Reject")] {
                    if ui.small_button(label).clicked() {
                        let (d, id, sid) = (dir.clone(), id.to_string(), sid.to_string());
                        app.agent.spawn(move |shared| {
                            let r = opencode(&d, &args(&["session.permission.reply", "--param", &format!("sessionID={sid}"), "--param", &format!("requestID={id}")]), Some(&json!({"decision": decision})));
                            if let Ok(mut sh) = shared.lock() {
                                sh.status = r.map_or_else(|e| e, |_| format!("Permission: {decision}."));
                            }
                        });
                    }
                }
            });
        }
        // Questions the agent asked: pick an option or type your own answer.
        for form in &sh.forms {
            let (Some(fid), Some(sid)) = (form["id"].as_str(), form["sessionID"].as_str()) else { continue };
            for field in form["fields"].as_array().into_iter().flatten() {
                let Some(key) = field["key"].as_str() else { continue };
                ui.add_space(6.0);
                ui.label(RichText::new(field["title"].as_str().unwrap_or("Question")).size(12.0).strong().color(Color32::from_rgb(240, 168, 60)));
                if let Some(d) = field["description"].as_str() {
                    ui.label(RichText::new(d).size(11.0).color(TEXT));
                }
                let mut chosen: Option<String> = None;
                for (n, opt) in field["options"].as_array().into_iter().flatten().enumerate() {
                    let label = opt["label"].as_str().or(opt["value"].as_str()).unwrap_or("");
                    let b = ui.button(label);
                    app.automation.add_response(format!("agent.question.{key}.{n}"), "button", label, &b);
                    let b = match opt["description"].as_str() { Some(d) => b.on_hover_text(d), None => b };
                    if b.clicked() {
                        chosen = opt["value"].as_str().map(str::to_owned);
                    }
                }
                if field["custom"].as_bool().unwrap_or(true) {
                    let slot = format!("{fid}/{key}");
                    let mut text = app.agent.answers.get(&slot).cloned().unwrap_or_default();
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut text).hint_text("Your own answer").desired_width(ui.available_width() - 70.0));
                        if ui.button("Answer").clicked() && !text.trim().is_empty() {
                            chosen = Some(text.trim().to_string());
                        }
                    });
                    app.agent.answers.insert(slot, text);
                }
                if let Some(value) = chosen {
                    let (d, fid, sid, key) = (dir.clone(), fid.to_string(), sid.to_string(), key.to_string());
                    app.agent.last_poll = None;
                    app.agent.spawn(move |shared| {
                        let r = opencode(&d, &args(&["session.form.reply", "--param", &format!("sessionID={sid}"), "--param", &format!("formID={fid}")]), Some(&json!({"answer": {key: value}})));
                        if let Ok(mut sh) = shared.lock() {
                            sh.status = r.map_or_else(|e| e, |_| "Answered.".into());
                        }
                    });
                }
            }
        }
    });
    ui.separator();

    // Pictures attached by pointing.
    let mut remove = None;
    for (i, (path, what)) in app.agent.attachments.clone().iter().enumerate() {
        ui.horizontal(|ui| {
            if let Some(tex) = app.load_texture(ui.ctx(), &path.to_string_lossy()) {
                ui.add(egui::Image::new(&tex).max_size(egui::vec2(48.0, 32.0)));
            }
            ui.label(RichText::new(what).size(10.5).color(MUTED));
            if ui.small_button("Remove").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        app.agent.attachments.remove(i);
    }
    let input = ui.add(egui::TextEdit::multiline(&mut app.agent.input).desired_rows(3).desired_width(f32::INFINITY).hint_text("Message (Ctrl+Enter to send)"));
    app.automation.add_response("agent.input", "text", "Agent message", &input);
    let ctrl_enter = input.has_focus() && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter));
    let mut send = ctrl_enter;
    ui.horizontal(|ui| {
        let point = ui.button(if app.agent.point_rx.is_some() { "Capturing…" } else { "Point" }).on_hover_text("Click anything in the app to attach a picture of it (Esc cancels)");
        app.automation.add_response("agent.point", "button", "Point at something", &point);
        if point.clicked() {
            app.agent.pointing = true;
        }
        let s = ui.button("Send");
        app.automation.add_response("agent.send", "button", "Send to agent", &s);
        send |= s.clicked();
    });
    let status = sh.status.clone();
    let session = sh.session.clone();
    drop(sh);
    if !status.is_empty() {
        ui.label(RichText::new(status).size(10.5).color(MUTED));
    }
    if let Some(s) = session {
        let r = ui.add(egui::Label::new(RichText::new(format!("Watch it in a terminal: opencode --session {s}")).size(10.0).color(MUTED)).sense(Sense::click()))
            .on_hover_text("Click to copy");
        if r.clicked() {
            ui.ctx().copy_text(format!("opencode --session {s}"));
        }
    }
    if send {
        let project = app.session.project_path.as_ref().and_then(|p| p.file_stem()).and_then(|n| n.to_str()).unwrap_or("no project").to_string();
        app.agent.send(&dir, project);
    }
}

/// Pointing: while on, the next click anywhere picks the smallest element under the mouse and
/// captures a picture of it (through the same path as `ui.look`) to attach to the next message.
pub(crate) fn pointing_overlay(app: &mut AnimationCraftApp, ctx: &egui::Context) {
    if let Some((rx, what, path)) = &app.agent.point_rx {
        match rx.try_recv() {
            Ok(Ok(_)) => {
                app.agent.attachments.push((path.clone(), what.clone()));
                app.agent.point_rx = None;
            }
            Ok(Err(e)) => {
                app.agent.set_status(format!("Could not capture: {e}"));
                app.agent.point_rx = None;
            }
            Err(_) => ctx.request_repaint(),
        }
    }
    if !app.agent.pointing {
        return;
    }
    let screen = ctx.content_rect();
    let hover = ctx.pointer_hover_pos();
    let under = hover.and_then(|p| {
        app.automation
            .elements
            .iter()
            .filter(|e| !e.id.starts_with("panel.") && !e.id.starts_with("agent.") && !e.id.ends_with(".screen"))
            .filter(|e| egui::Rect::from_min_size(egui::pos2(e.rect[0], e.rect[1]), egui::vec2(e.rect[2], e.rect[3])).contains(p))
            .min_by(|a, b| (a.rect[2] * a.rect[3]).total_cmp(&(b.rect[2] * b.rect[3])))
            .map(|e| (e.id.clone(), e.label.clone(), e.rect))
    });
    let mut clicked = false;
    egui::Area::new(egui::Id::new("agent.point.overlay")).order(egui::Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
        let r = ui.allocate_rect(screen, Sense::click());
        ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(36));
        if let Some((id, label, rect)) = &under {
            let rect = egui::Rect::from_min_size(egui::pos2(rect[0], rect[1]), egui::vec2(rect[2], rect[3]));
            ui.painter().rect_stroke(rect.expand(2.0), 3.0, egui::Stroke::new(2.0, chrome::TEAL), egui::StrokeKind::Outside);
            ui.painter().text(rect.left_top() - egui::vec2(0.0, 4.0), egui::Align2::LEFT_BOTTOM, format!("{label}  ·  {id}"), egui::FontId::proportional(12.0), chrome::TEAL);
        }
        ui.painter().text(screen.center_top() + egui::vec2(0.0, 8.0), egui::Align2::CENTER_TOP, "Click what you mean · Esc cancels", egui::FontId::proportional(13.0), Color32::WHITE);
        clicked = r.clicked();
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.agent.pointing = false;
        return;
    }
    if !clicked {
        return;
    }
    app.agent.pointing = false;
    let Some((id, label, _)) = under else { return };
    if app.pending_look.is_some() {
        app.agent.set_status("A capture is already running; point again in a moment.".into());
        return;
    }
    let dir = workdir(app).join(".agent").join("points");
    let _ = std::fs::create_dir_all(&dir);
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let path = dir.join(format!("point-{stamp}.png"));
    let (tx, rx) = channel();
    app.pending_look = Some((json!({"region": id, "output": path, "max_side": 1000, "pad": 80}), tx, Instant::now(), Some(2)));
    app.agent.point_rx = Some((rx, format!("“{label}” ({id})"), path));
    ctx.request_repaint();
}
