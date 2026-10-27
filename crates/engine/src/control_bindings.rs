//! Explicit artwork control bindings. Coordinates are cropped-layer pixels;
//! gains multiply raw slider units (percent or degrees), never guessed anatomy.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ControlBindings {
    #[serde(default)]
    pub face: Option<FaceBinding>,
    #[serde(default)]
    pub joints: Vec<JointBinding>,
    #[serde(default)]
    pub pins: Vec<PinBinding>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceBinding {
    pub part: String,
    #[serde(default)]
    pub landmarks: Option<PathBuf>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JointBinding {
    pub part: String,
    pub control: String,
    #[serde(default)]
    pub rotation_deg: f32,
    #[serde(default)]
    pub translation: [f32; 2],
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinBinding {
    pub part: String,
    pub src: [f32; 2],
    #[serde(default)]
    pub control: Option<String>,
    #[serde(default)]
    pub delta: [f32; 2],
}
pub type ControlValues = BTreeMap<String, f32>;
pub const CONTROLS: &[&str] = &[
    "eye_blink",
    "eye_gaze",
    "smile_open",
    "head_yaw",
    "head_pitch",
    "ear_splay",
    "ear_pitch",
    "arm_spread",
    "elbow_bend",
    "knee_bend",
];
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PartAdjustment {
    pub rotation_deg: f32,
    pub translation: [f32; 2],
}
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct EvaluatedPin {
    pub src: [f32; 2],
    pub dst: [f32; 2],
}

impl ControlBindings {
    /// Whether a control has an explicit route. Facial raster preview owns only
    /// mouth opening; eyes require authored joints or pins.
    pub fn supports(&self, control: &str) -> bool {
        (control == "smile_open" && self.face.is_some())
            || self.joints.iter().any(|j| {
                j.control == control && (j.rotation_deg != 0.0 || j.translation != [0.0, 0.0])
            })
            || self
                .pins
                .iter()
                .any(|p| p.control.as_deref() == Some(control) && p.delta != [0.0, 0.0])
    }

    /// Reject unsupported controls, dangling part references and invalid arithmetic
    /// before a plan is installed. Missing mappings intentionally do nothing.
    pub fn validate(&self, parts: &[String]) -> Result<(), String> {
        let part = |p: &str| {
            if parts.iter().any(|id| id == p) {
                Ok(())
            } else {
                Err(format!("control binding references missing part '{p}'"))
            }
        };
        let control = |c: &str| {
            if CONTROLS.contains(&c) {
                Ok(())
            } else {
                Err(format!("unknown control '{c}'"))
            }
        };
        if let Some(face) = &self.face {
            part(&face.part)?;
        }
        for j in &self.joints {
            part(&j.part)?;
            control(&j.control)?;
            if !j.rotation_deg.is_finite() || !j.translation.iter().all(|x| x.is_finite()) {
                return Err("joint binding gains must be finite".into());
            }
        }
        for p in &self.pins {
            part(&p.part)?;
            if let Some(c) = &p.control {
                control(c)?;
            }
            if !p.src.iter().chain(&p.delta).all(|x| x.is_finite()) {
                return Err("pin coordinates and gains must be finite".into());
            }
            if p.control.is_none() && p.delta != [0.0, 0.0] {
                return Err("fixed pins must have zero delta".into());
            }
        }
        Ok(())
    }
    pub fn evaluate_part(&self, part: &str, values: &ControlValues) -> PartAdjustment {
        let mut result = PartAdjustment::default();
        for j in self.joints.iter().filter(|j| j.part == part) {
            let v = value(values, &j.control);
            result.rotation_deg += j.rotation_deg * v;
            for axis in 0..2 {
                result.translation[axis] += j.translation[axis] * v;
            }
        }
        result
    }
    pub fn evaluate_pins(&self, part: &str, values: &ControlValues) -> Vec<EvaluatedPin> {
        self.pins
            .iter()
            .filter(|p| p.part == part)
            .map(|p| {
                let v = p.control.as_ref().map(|c| value(values, c)).unwrap_or(0.0);
                EvaluatedPin {
                    src: p.src,
                    dst: [p.src[0] + p.delta[0] * v, p.src[1] + p.delta[1] * v],
                }
            })
            .collect()
    }
}
fn value(values: &ControlValues, key: &str) -> f32 {
    values
        .get(key)
        .copied()
        .filter(|v| v.is_finite())
        .unwrap_or(0.0)
}
