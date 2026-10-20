//! Automation registry: every interactive element and rendered canvas layer registers a stable id
//! and its screen rect each frame, so agents can click by id (`ui.click {id:"preset.whiskers"}`),
//! query on-screen geometry (`ui.inspect` / `ui.elements`), and inspect rendered sprite/layer bounds
//! without a manual screenshot-download-review loop.

use egui::Rect;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Element {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub rect: [f32; 4],
    #[serde(rename = "widgetId", skip_serializing_if = "Option::is_none")]
    pub widget_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focusable: Option<bool>,
}

#[derive(Default)]
pub struct Registry {
    pub elements: Vec<Element>,
    /// Last frame's elements (for lookups from control handlers between frames).
    pub previous: Vec<Element>,
}

impl Registry {
    pub fn add(&mut self, id: impl Into<String>, kind: &str, label: impl Into<String>, rect: Rect) {
        self.elements.push(Element {
            id: id.into(),
            kind: kind.to_string(),
            label: label.into(),
            rect: [rect.min.x, rect.min.y, rect.width(), rect.height()],
            widget_id: None,
            enabled: None,
            focusable: None,
        });
    }

    /// Observe an egui widget response and record its screen rect and state.
    pub fn add_response(
        &mut self,
        id: impl Into<String>,
        kind: &str,
        label: impl Into<String>,
        response: &egui::Response,
    ) {
        self.add(id, kind, label, response.rect);
        if let Some(element) = self.elements.last_mut() {
            element.widget_id = Some(format!("{:016x}", response.id.value()));
            element.enabled = Some(response.enabled());
            element.focusable = Some(response.sense.is_focusable());
        }
    }

    pub fn begin_frame(&mut self) {
        self.previous = std::mem::take(&mut self.elements);
    }

    pub fn find(&self, id: &str) -> Option<&Element> {
        self.previous
            .iter()
            .chain(self.elements.iter())
            .find(|e| e.id == id)
    }

    /// Elements matching optional `filter` (by id, kind, or label substring).
    pub fn query(&self, filter: Option<&str>) -> Vec<Element> {
        let source = if self.previous.is_empty() {
            &self.elements
        } else {
            &self.previous
        };
        let f = filter.map(str::to_ascii_lowercase).unwrap_or_default();
        source
            .iter()
            .filter(|e| {
                f.is_empty()
                    || e.id.to_ascii_lowercase().contains(&f)
                    || e.kind.to_ascii_lowercase().contains(&f)
                    || e.label.to_ascii_lowercase().contains(&f)
            })
            .cloned()
            .collect()
    }

    /// Detect overlapping sibling widgets (same kind or interactive buttons/tabs) and widgets
    /// clipped outside their parent panel or window bounds (`[0, 0, win_w, win_h]`).
    pub fn layout_audit(&self, win_w: f32, win_h: f32) -> serde_json::Value {
        let source = if self.previous.is_empty() {
            &self.elements
        } else {
            &self.previous
        };
        let interactive: Vec<&Element> = source
            .iter()
            .filter(|e| e.kind != "panel" && e.rect[2] > 1.0 && e.rect[3] > 1.0)
            .collect();

        let mut overlaps = Vec::new();
        for i in 0..interactive.len() {
            for j in (i + 1)..interactive.len() {
                let a = interactive[i];
                let b = interactive[j];
                if a.id == b.id {
                    continue;
                }
                let x_overlap =
                    (a.rect[0] + a.rect[2]).min(b.rect[0] + b.rect[2]) - a.rect[0].max(b.rect[0]);
                let y_overlap =
                    (a.rect[1] + a.rect[3]).min(b.rect[1] + b.rect[3]) - a.rect[1].max(b.rect[1]);
                if x_overlap > 2.0 && y_overlap > 2.0 {
                    overlaps.push(serde_json::json!({
                        "a": { "id": a.id, "label": a.label, "rect": a.rect },
                        "b": { "id": b.id, "label": b.label, "rect": b.rect },
                        "overlap_px": [x_overlap, y_overlap],
                    }));
                }
            }
        }

        let mut clipped = Vec::new();
        for e in &interactive {
            let right = e.rect[0] + e.rect[2];
            let bottom = e.rect[1] + e.rect[3];
            if e.rect[0] < -1.0 || e.rect[1] < -1.0 || right > win_w + 1.0 {
                clipped.push(serde_json::json!({
                    "id": e.id,
                    "label": e.label,
                    "rect": e.rect,
                    "window_size": [win_w, win_h],
                    "clipped_bottom": bottom > win_h + 1.0,
                }));
            }
        }

        let mut multi_line_buttons = Vec::new();
        for e in &interactive {
            if e.kind == "button" && e.rect[3] > 42.0 && e.rect[2] < 110.0 {
                multi_line_buttons.push(serde_json::json!({
                    "id": e.id,
                    "label": e.label,
                    "rect": e.rect,
                }));
            }
        }

        serde_json::json!({
            "clean": overlaps.is_empty() && clipped.is_empty() && multi_line_buttons.is_empty(),
            "overlap_count": overlaps.len(),
            "overlaps": overlaps,
            "clipped_count": clipped.len(),
            "clipped": clipped,
            "wrapped_button_count": multi_line_buttons.len(),
            "wrapped_buttons": multi_line_buttons,
        })
    }
}
