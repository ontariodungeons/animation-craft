//! Cached, per-part deformation jobs shared by native preview and export.
use crate::{AnimationCraftApp, UiState, face_preview::FacePreview};
use crate::panels::center::center_motion::control_values;
use animationcraft_engine::session::{CutoutRigPart, LoadedCutoutRigPlan};
use serde_json::{Value, json};
use std::{collections::{BTreeMap, HashMap}, path::PathBuf};

#[derive(Default)]
pub(crate) struct ControlPreview {
    jobs: HashMap<String, FacePreview>,
}

fn recipe(rig: &LoadedCutoutRigPlan, part: &CutoutRigPart, state: &UiState) -> Option<Value> {
    let pins = rig.control_bindings.evaluate_pins(&part.id, &control_values(state));
    let pin_change = pins.iter().any(|p| p.src != p.dst);
    let mouth_changed = state.mouth_viseme != "smile" || (state.smile_open - 20.0).abs() > 0.1;
    let face = rig.control_bindings.face.as_ref().filter(|face| face.part == part.id && mouth_changed);
    if !pin_change && face.is_none() { return None; }
    Some(json!({"pins": pins, "face": face.map(|f| json!({"landmarks": f.landmarks, "revision": f.landmarks.as_ref().and_then(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok()).map(|t| format!("{t:?}"))}))}))
}

impl ControlPreview {
    pub(crate) fn path(&mut self, ctx: &egui::Context, rig: &LoadedCutoutRigPlan, part: &CutoutRigPart, state: &UiState) -> (PathBuf, String) {
        let Some(recipe) = recipe(rig, part, state) else { return (part.cropped_path.clone(), String::new()); };
        let worker = self.jobs.entry(part.id.clone()).or_default();
        let path = worker.path_recipe(ctx, &part.cropped_path, state, Some(recipe));
        (path, worker.status.clone())
    }

    fn ready(&mut self, rig: &LoadedCutoutRigPlan, state: &UiState) -> Result<BTreeMap<String, Vec<PathBuf>>, String> {
        let mut textures = BTreeMap::new();
        for part in &rig.parts {
            if let Some(recipe) = recipe(rig, part, state) {
                let worker = self.jobs.get_mut(&part.id).ok_or_else(|| format!("Preview the deformation on '{}' before exporting", part.label))?;
                textures.insert(part.id.clone(), worker.ready_recipe(&part.cropped_path, state, &recipe)?);
            }
        }
        Ok(textures)
    }
}

impl AnimationCraftApp {
    pub(crate) fn export_control_textures(&mut self) -> Result<BTreeMap<String, Vec<PathBuf>>, String> {
        let rig = self.session().current_asset().and_then(|a| a.cutout_rig.as_ref()).cloned()
            .ok_or("Load a cutout or layered rig before exporting the preview")?;
        self.control_preview.ready(&rig, &self.ui_state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use animationcraft_engine::control_bindings::{ControlBindings, FaceBinding, PinBinding};

    #[test]
    fn arbitrary_part_mapping_isolated_and_pending_texture_cannot_export() {
        let part = CutoutRigPart { id: "surface_17".into(), cropped_path: "unused.png".into(), ..Default::default() };
        let other = CutoutRigPart { id: "surface_18".into(), ..Default::default() };
        let rig = LoadedCutoutRigPlan { parts: vec![part.clone(), other.clone()], control_bindings: ControlBindings {
            face: Some(FaceBinding { part: part.id.clone(), landmarks: None }),
            pins: vec![PinBinding { part: other.id.clone(), src: [10.0, 10.0], control: Some("eye_gaze".into()), delta: [0.2, 0.0] }],
            ..Default::default()
        }, ..Default::default() };
        let mut state = UiState::default();
        assert!(recipe(&rig, &part, &state).is_none());
        state.smile_open = 60.0;
        assert!(recipe(&rig, &part, &state).unwrap()["face"].is_object());
        assert!(recipe(&rig, &other, &state).is_none());
        assert!(ControlPreview::default().ready(&rig, &state).is_err());
        state.smile_open = 20.0;
        state.eye_gaze = 20.0;
        assert!(recipe(&rig, &part, &state).is_none());
        assert_eq!(recipe(&rig, &other, &state).unwrap()["pins"][0]["dst"], json!([14.0, 10.0]));
    }
}
