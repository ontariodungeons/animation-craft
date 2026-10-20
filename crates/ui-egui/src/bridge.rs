//! TCP JSON-lines `--control` bridge, headless frame runner, and viewport screenshot/vision capture.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::{AnimationCraftApp, ControlRequest, control_params};

pub struct SharedNativeApp {
    pub(crate) app: Arc<Mutex<AnimationCraftApp>>,
    pub(crate) last_ui_frame: Arc<Mutex<Instant>>,
}

impl eframe::App for SharedNativeApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        if let Ok(mut app) = self.app.lock() {
            app.raw_input_hook(ctx, input);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if let Ok(mut t) = self.last_ui_frame.lock() {
            *t = Instant::now();
        }
        if let Ok(mut app) = self.app.lock() {
            app.ui(ui, frame);
        }
    }
}

impl AnimationCraftApp {
    /// Start a localhost JSON-lines TCP control server (`--control <port>`).
    pub fn start_control_server(&mut self, port: u16) {
        let (tx, rx) = mpsc::channel::<ControlRequest>();
        self.control_rx = Some(rx);
        self.control_port = Some(port);
        self.ui_state.status_line = format!(
            "Live CLI Bridge listening on 127.0.0.1:{port} (`animationcraft-cli --bridge {port} ...`)"
        );
        let repaint_ctx = Arc::clone(&self.repaint_ctx);

        std::thread::spawn(move || {
            let Ok(listener) = TcpListener::bind(("127.0.0.1", port)) else {
                return;
            };
            for stream in listener.incoming().flatten() {
                let tx_clone = tx.clone();
                let repaint_clone = Arc::clone(&repaint_ctx);
                std::thread::spawn(move || {
                    let Ok(peer) = stream.try_clone() else {
                        return;
                    };
                    let reader = BufReader::new(stream);
                    let mut writer = peer;
                    for line in reader.lines().map_while(Result::ok) {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        let parsed: Value = match serde_json::from_str(trimmed) {
                            Ok(v) => v,
                            Err(e) => {
                                let err_line =
                                    json!({"ok": false, "error": e.to_string()}).to_string();
                                let _ = writeln!(writer, "{err_line}");
                                continue;
                            }
                        };
                        let id = parsed
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        let params = parsed.get("params").cloned().unwrap_or_else(|| json!({}));
                        let (reply_tx, reply_rx) = mpsc::channel();
                        if tx_clone
                            .send(ControlRequest {
                                id: id.clone(),
                                params,
                                reply: reply_tx,
                            })
                            .is_err()
                        {
                            break;
                        }
                        if let Ok(guard) = repaint_clone.lock()
                            && let Some(ctx) = guard.as_ref()
                        {
                            ctx.request_repaint();
                        }
                        match reply_rx.recv() {
                            Ok(Ok(res)) => {
                                let out = json!({"ok": true, "id": id, "result": res}).to_string();
                                let _ = writeln!(writer, "{out}");
                            }
                            Ok(Err(err)) => {
                                let out = json!({"ok": false, "id": id, "error": err}).to_string();
                                let _ = writeln!(writer, "{out}");
                            }
                            Err(e) => {
                                let out = json!({"ok": false, "id": id, "error": e.to_string()})
                                    .to_string();
                                let _ = writeln!(writer, "{out}");
                            }
                        }
                    }
                });
            }
        });
    }

    pub fn expire_stale_screenshots(&mut self, max_age: Duration) {
        let msg = "Viewport screenshot timed out because the window surface is suspended or unmapped by the compositor; focus the window or use headless vision.analyze";
        if self
            .pending_screenshot
            .as_ref()
            .is_some_and(|(_, _, t)| t.elapsed() >= max_age)
            && let Some((_, reply_tx, _)) = self.pending_screenshot.take()
        {
            let _ = reply_tx.send(Err(msg.to_string()));
        }
        if self
            .pending_vision
            .as_ref()
            .is_some_and(|(_, t)| t.elapsed() >= max_age)
            && let Some((reply_tx, _)) = self.pending_vision.take()
        {
            let _ = reply_tx.send(Err(msg.to_string()));
        }
    }

    pub fn render_headless_frame(&mut self, ctx: &egui::Context) {
        let mut events = Vec::new();
        for pos in self.pending_clicks.drain(..) {
            events.push(egui::Event::PointerMoved(pos));
            for pressed in [true, false] {
                events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            }
        }
        let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 980.0));
        if events.is_empty() {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen_rect),
                    ..Default::default()
                },
                |ui| {
                    self.render_workspace(ui);
                },
            );
            out.textures_delta.clear();
        } else {
            for ev in events {
                let mut out = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(screen_rect),
                        events: vec![ev],
                        ..Default::default()
                    },
                    |ui| {
                        self.render_workspace(ui);
                    },
                );
                out.textures_delta.clear();
            }
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen_rect),
                    ..Default::default()
                },
                |ui| {
                    self.render_workspace(ui);
                },
            );
            out.textures_delta.clear();
        }
    }

    fn take_control_requests(&mut self) -> Vec<ControlRequest> {
        let mut pending = Vec::new();
        if let Some(rx) = &self.control_rx {
            while let Ok(req) = rx.try_recv() {
                pending.push(req);
            }
        }
        pending
    }

    fn send_viewport_cmd_and_repaint(&self, live_ctx: Option<&egui::Context>, cmd: egui::ViewportCommand) -> bool {
        if let Some(ctx) = live_ctx {
            ctx.send_viewport_cmd(cmd);
            ctx.request_repaint();
            return true;
        }
        if let Ok(guard) = self.repaint_ctx.lock()
            && let Some(ctx) = guard.as_ref()
        {
            ctx.send_viewport_cmd(cmd);
            ctx.request_repaint();
            return true;
        }
        false
    }

    fn request_repaint(&self, live_ctx: Option<&egui::Context>) {
        if let Some(ctx) = live_ctx {
            ctx.request_repaint();
        } else if let Ok(guard) = self.repaint_ctx.lock()
            && let Some(ctx) = guard.as_ref()
        {
            ctx.request_repaint();
        }
    }

    /// Unified control request handler shared between live `eframe::App::ui` and headless ticks.
    pub(crate) fn handle_control_requests(
        &mut self,
        pending: Vec<ControlRequest>,
        live_ctx: Option<&egui::Context>,
        headless_ctx: Option<&egui::Context>,
    ) {
        for req in pending {
            if req.id == "ui.screenshot" {
                let out_path = req
                    .params
                    .get("output")
                    .and_then(Value::as_str)
                    .unwrap_or("exports/animationcraft_window.png")
                    .to_string();
                self.pending_screenshot = Some((out_path, req.reply, Instant::now()));
                self.send_viewport_cmd_and_repaint(
                    live_ctx,
                    egui::ViewportCommand::Screenshot(egui::UserData::default()),
                );
            } else if req.id == "ui.vision" {
                self.pending_vision = Some((req.reply, Instant::now()));
                self.send_viewport_cmd_and_repaint(
                    live_ctx,
                    egui::ViewportCommand::Screenshot(egui::UserData::default()),
                );
            } else if req.id == "app.quit" {
                if let Some(ctx) = live_ctx {
                    let _ = req.reply.send(Ok(json!({"closing": true})));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                } else {
                    let sent =
                        self.send_viewport_cmd_and_repaint(None, egui::ViewportCommand::Close);
                    let _ = req.reply.send(if sent {
                        Ok(json!({"closing": true}))
                    } else {
                        Err("app.quit needs the native window: start `animationcraft --control <port>` and use `--bridge <port>`".into())
                    });
                }
            } else {
                let reply_val = self.dispatch(&req.id, req.params);
                if let Some(hctx) = headless_ctx
                    && (!self.pending_clicks.is_empty() || req.id == "ui.set_view")
                {
                    self.render_headless_frame(hctx);
                }
                let _ = req.reply.send(reply_val);
                self.request_repaint(live_ctx);
            }
        }
    }

    pub(crate) fn drain_live_control_requests(&mut self, ctx: &egui::Context) {
        let pending = self.take_control_requests();
        self.handle_control_requests(pending, Some(ctx), None);
    }

    /// Service pending control requests and background jobs when the native window is hidden or
    /// suspended by the compositor (e.g., on an inactive Wayland workspace).
    pub fn service_headless_control_tick(
        &mut self,
        headless_ctx: &egui::Context,
        screenshot_timeout: Duration,
    ) {
        self.poll_background_jobs_for_test();
        self.expire_stale_screenshots(screenshot_timeout);

        let pending = self.take_control_requests();
        if pending.is_empty() {
            return;
        }
        if self.automation.elements.is_empty() {
            self.render_headless_frame(headless_ctx);
        }
        self.handle_control_requests(pending, None, Some(headless_ctx));
    }

    pub(crate) fn handle_screenshot_events(&mut self, ctx: &egui::Context) {
        let screenshot_img = ctx.input(|i| {
            for ev in &i.raw.events {
                if let egui::Event::Screenshot { image, .. } = ev {
                    return Some(image.clone());
                }
            }
            None
        });
        let Some(color_img) = screenshot_img else {
            return;
        };
        let w = color_img.width() as u32;
        let h = color_img.height() as u32;
        let mut raw = Vec::with_capacity((w * h * 4) as usize);
        for px in &color_img.pixels {
            raw.extend_from_slice(&px.to_srgba_unmultiplied());
        }
        let bg_colors: &[[u8; 3]] = if self.ui_state.light_backdrop {
            &[[235, 238, 244], [225, 228, 235], [191, 197, 208]]
        } else {
            &[[16, 18, 24], [20, 24, 32], [31, 37, 49]]
        };
        let ppp = ctx.pixels_per_point();
        let region = ["stage.viewport_2d", "pose.canvas"].iter().find_map(|id| {
            let el = self.automation.elements.iter().find(|e| e.id == *id)?;
            let x0 = ((el.rect[0] * ppp).round().max(0.0) as u32).min(w);
            let y0 = ((el.rect[1] * ppp).round().max(0.0) as u32).min(h);
            let x1 = (((el.rect[0] + el.rect[2]) * ppp).round() as u32).min(w);
            let y1 = (((el.rect[1] + el.rect[3]) * ppp).round() as u32).min(h);
            (x1 > x0 && y1 > y0).then(|| (*id, [x0, y0, x1 - x0, y1 - y0]))
        });
        let mut vision_metrics = match region {
            Some((id, [x0, y0, rw, rh])) => {
                let mut crop = Vec::with_capacity((rw * rh * 4) as usize);
                for y in y0..y0 + rh {
                    let start = ((y * w + x0) * 4) as usize;
                    crop.extend_from_slice(&raw[start..start + (rw * 4) as usize]);
                }
                let mut m = animationcraft_engine::commands::analyze_viewport_region(
                    &crop, rw, rh, bg_colors,
                );
                m["region"] = json!({ "id": id, "rect_px": [x0, y0, rw, rh] });
                m
            }
            None => {
                let mut m =
                    animationcraft_engine::commands::analyze_viewport_region(&raw, w, h, bg_colors);
                m["region"] = json!({ "id": "window", "rect_px": [0, 0, w, h] });
                m
            }
        };
        vision_metrics["note"] =
            json!("pixel coordinates are relative to region.rect_px within the window screenshot");
        if let Some((reply_tx, _)) = self.pending_vision.take() {
            let _ = reply_tx.send(Ok(json!({
                "active_asset": self.session.active_asset,
                "controls": control_params::snapshot(&self.ui_state),
                "tab": format!("{:?}", self.ui_state.center_tab),
                "anim_mode": format!("{:?}", self.ui_state.anim_mode),
                "viewport": vision_metrics,
                "element_count": self.automation.elements.len(),
            })));
        }
        if let Some((out_path, reply_tx, _)) = self.pending_screenshot.take() {
            let save_res = match image::RgbaImage::from_raw(w, h, raw) {
                Some(buf) => buf
                    .save(&out_path)
                    .map(|()| {
                        json!({
                            "saved": out_path,
                            "width": w,
                            "height": h,
                            "vision": vision_metrics,
                        })
                    })
                    .map_err(|e| e.to_string()),
                None => Err("Failed to construct RgbaImage from screenshot buffer".to_string()),
            };
            let _ = reply_tx.send(save_res);
        }
    }

    /// Wrap this app in a shared native runner that keeps `--control <port>` responsive even when
    /// the Wayland compositor suspends `eframe::App::ui` on an inactive workspace.
    pub fn into_native_app(self, ctx: egui::Context) -> Box<dyn eframe::App> {
        if let Ok(mut guard) = self.repaint_ctx.lock() {
            *guard = Some(ctx);
        }
        let has_control = self.control_port.is_some();
        let shared = Arc::new(Mutex::new(self));
        let last_ui_frame = Arc::new(Mutex::new(Instant::now()));

        if has_control {
            let app_clone = Arc::clone(&shared);
            let last_frame_clone = Arc::clone(&last_ui_frame);
            std::thread::spawn(move || {
                let headless_ctx = egui::Context::default();
                loop {
                    std::thread::sleep(Duration::from_millis(100));
                    let frame_age = last_frame_clone
                        .lock()
                        .map(|t| t.elapsed())
                        .unwrap_or(Duration::ZERO);
                    if frame_age >= Duration::from_millis(500)
                        && let Ok(mut app) = app_clone.try_lock()
                    {
                        app.service_headless_control_tick(
                            &headless_ctx,
                            Duration::from_millis(1200),
                        );
                    }
                }
            });
        }

        Box::new(SharedNativeApp {
            app: shared,
            last_ui_frame,
        })
    }
}
