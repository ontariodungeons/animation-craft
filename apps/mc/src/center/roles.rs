use super::*;

pub(super) fn assigned_booth(c: &Center) -> bool {
    c.scope.is_some()
}

pub(super) fn title(_c: &Center) -> String {
    "MAGIC STUDIO · BACKSTAGE".into()
}

pub(super) fn subtitle(_c: &Center) -> String {
    String::new()
}

pub(super) fn filter_actions(_c: &Center, actions: Vec<Value>) -> Vec<Value> {
    actions
}

pub(super) fn activity(_c: &Center, _pane_id: &str) -> String {
    String::new()
}

pub(super) fn key_error(_c: &Center, _key: crossterm::event::KeyEvent) -> Option<String> {
    None
}

pub(super) fn action_error(_c: &Center, _name: &str, _p: &Value) -> Option<String> {
    None
}

pub(super) fn prepare(_c: &mut Center, _p: &Value) -> Result<(), String> {
    Ok(())
}

pub(super) fn controls(_c: &Center) -> Value {
    json!([])
}

pub(super) fn render(_f: &mut Frame, _c: &Center, _area: ratatui::layout::Rect) {}

pub(super) fn render_onboarding(_f: &mut Frame, _c: &Center, _area: ratatui::layout::Rect) {}

pub(super) fn render_animal(_f: &mut Frame, _c: &Center, _area: ratatui::layout::Rect) {}
