//! Java's keyframe animations (`KeyframeAnimation`, 26.3 client), as `AnimationExporter` wrote
//! them into the look (`java/entity_animations.json`, keyed `Class.FIELD`). Loaded once per
//! process by [`install`]; the keyframed models' `setupAnim` ([`super::anim`]) apply them.

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use serde::Deserialize;

use super::poses::Poses;
use super::state::RenderState;

pub const ANIMATIONS: &str = "java/entity_animations.json";

#[derive(Deserialize, Debug)]
pub struct Definition {
    length: f32,
    looping: bool,
    bones: HashMap<String, Vec<Channel>>,
}

#[derive(Deserialize, Debug)]
struct Channel {
    target: String,
    /// Timestamp, pre target xyz, post target xyz, interpolation.
    keyframes: Vec<(f32, f32, f32, f32, f32, f32, f32, String)>,
}

static DEFINITIONS: OnceLock<HashMap<String, Definition>> = OnceLock::new();

/// Reads the animations from a look's root (or the file itself) into the process-wide table, the
/// first time only; `false` when there are none to read.
pub fn install(path: &Path) -> bool {
    let file = if path.is_dir() { path.join(ANIMATIONS) } else { path.to_owned() };
    let Ok(text) = std::fs::read_to_string(&file) else { return false };
    match serde_json::from_str(&text) {
        Ok(table) => {
            let _ = DEFINITIONS.set(table);
            true
        }
        Err(e) => {
            tracing::warn!("{}: {e}", file.display());
            false
        }
    }
}

fn definition(name: &str) -> Option<&'static Definition> {
    DEFINITIONS.get()?.get(name)
}

/// `Mth.catmullrom`.
fn catmullrom(a: f32, p0: f32, p1: f32, p2: f32, p3: f32) -> f32 {
    0.5 * (2.0 * p1 + (p2 - p0) * a + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * a * a + (3.0 * p1 - p0 - 3.0 * p2 + p3) * a * a * a)
}

impl Channel {
    fn pre(&self, i: usize) -> [f32; 3] {
        let k = &self.keyframes[i];
        [k.1, k.2, k.3]
    }

    fn post(&self, i: usize) -> [f32; 3] {
        let k = &self.keyframes[i];
        [k.4, k.5, k.6]
    }

    /// `KeyframeAnimation.Entry.apply`'s interpolated target at `seconds`.
    fn sample(&self, seconds: f32, scale: f32) -> [f32; 3] {
        let frames = &self.keyframes;
        let first_after = frames.iter().position(|k| seconds <= k.0).unwrap_or(frames.len());
        let prev = first_after.saturating_sub(1);
        let next = (prev + 1).min(frames.len() - 1);
        let alpha = if next != prev { ((seconds - frames[prev].0) / (frames[next].0 - frames[prev].0)).clamp(0.0, 1.0) } else { 0.0 };
        if frames[next].7 == "catmullrom" {
            let points = [self.post(prev.saturating_sub(1)), self.post(prev), self.post(next), self.post((next + 1).min(frames.len() - 1))];
            std::array::from_fn(|i| catmullrom(alpha, points[0][i], points[1][i], points[2][i], points[3][i]) * scale)
        } else {
            let (from, to) = (self.post(prev), self.pre(next));
            // JOML's Vector3f.lerp: Math.fma, unfused unless -Djoml.useMathFma.
            std::array::from_fn(|i| ((to[i] - from[i]) * alpha + from[i]) * scale)
        }
    }
}

/// `KeyframeAnimation.apply(millis, scale)`: every channel added onto its part.
pub fn apply(p: &mut Poses, name: &str, millis: i64, scale: f32) {
    let Some(def) = definition(name) else { return };
    let seconds = millis as f32 / 1000.0;
    let seconds = if def.looping { seconds % def.length } else { seconds };
    for (bone, channels) in &def.bones {
        for channel in channels {
            if channel.keyframes.is_empty() {
                continue;
            }
            let [x, y, z] = channel.sample(seconds, scale);
            let pose = p.get(bone);
            match channel.target.as_str() {
                "position" => (pose.x, pose.y, pose.z) = (pose.x + x, pose.y + y, pose.z + z),
                "rotation" => (pose.x_rot, pose.y_rot, pose.z_rot) = (pose.x_rot + x, pose.y_rot + y, pose.z_rot + z),
                _ => (pose.x_scale, pose.y_scale, pose.z_scale) = (pose.x_scale + x, pose.y_scale + y, pose.z_scale + z),
            }
        }
    }
}

/// `KeyframeAnimation.applyWalk`.
pub fn apply_walk(p: &mut Poses, name: &str, pos: f32, speed: f32, speed_factor: f32, scale_factor: f32) {
    apply(p, name, (pos * 50.0 * speed_factor) as i64, (speed * scale_factor).min(1.0));
}

/// `KeyframeAnimation.apply(state, ageInTicks, speedFactor)`: when the render state's
/// `AnimationState` field (its start tick) is started.
pub fn apply_state(p: &mut Poses, name: &str, s: &RenderState, field: &str, speed_factor: f32) {
    if let Some(start) = started(s, field) {
        let millis = ((s.age() - start as f32) * 50.0) as i64;
        apply(p, name, (millis as f32 * speed_factor) as i64, 1.0);
    }
}

/// An `AnimationState`'s start tick, when started.
pub fn started(s: &RenderState, field: &str) -> Option<i32> {
    s.has(field).then(|| s.num(field) as i32)
}
