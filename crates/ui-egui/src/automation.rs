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

    /// Elements matching optional `filter` (by id or label substring).
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
                    || e.label.to_ascii_lowercase().contains(&f)
            })
            .cloned()
            .collect()
    }
}
