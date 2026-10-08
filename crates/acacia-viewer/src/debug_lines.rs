//! What the debug screen (F3) lists, from the camera, the renderer's figures and the target.

use acacia_render::{Camera, FrameStats};
use acacia_world::World;

use crate::app::Mode;
use crate::pick::Target;
use crate::settings::LookChoice;

pub struct Facts<'a> {
    pub camera: &'a Camera,
    pub fps: f32,
    pub stats: &'a FrameStats,
    pub look: LookChoice,
    pub mode: Mode,
    pub target: Option<&'a Target>,
    pub world: Option<&'a World>,
}

/// Left and right columns.
pub fn lines(f: &Facts) -> (Vec<String>, Vec<String>) {
    let p = f.camera.position;
    let b = p.floor().as_ivec3();
    let (yaw, pitch) = (f.camera.yaw.to_degrees(), f.camera.pitch.to_degrees());
    let left = vec![
        format!("Acacia ({:?} look, {:?})", f.look, f.mode),
        format!("{:.0} fps", f.fps),
        format!("{} sections, {} drawn, {}k quads, {} meshing", f.stats.sections, f.stats.drawn, f.stats.quads / 1000, f.stats.pending),
        String::new(),
        format!("XYZ: {:.3} / {:.5} / {:.3}", p.x, p.y, p.z),
        format!("Block: {} {} {}", b.x, b.y, b.z),
        format!("Chunk: {} {} {} in {} {} {}", b.x & 15, b.y & 15, b.z & 15, b.x >> 4, b.y >> 4, b.z >> 4),
        format!("Facing: {} ({:.1} / {:.1})", facing(yaw), wrap(yaw), pitch),
    ];
    let mut right = Vec::new();
    if let (Some(t), Some(world)) = (f.target, f.world) {
        let id = world.get(t.block.x >> 4, t.block.z >> 4).map(|c| c.read().block(t.block.x, t.block.y, t.block.z));
        let state = id.and_then(|id| world.registry().get(id));
        right.push(format!("Targeted Block: {}, {}, {}", t.block.x, t.block.y, t.block.z));
        if let Some(s) = state {
            right.push(s.name.to_owned());
            right.extend(s.properties.trim_matches(|c| c == '[' || c == ']').split(',').filter(|p| !p.is_empty()).map(str::to_owned));
        }
    }
    (left, right)
}

fn wrap(degrees: f32) -> f32 {
    (degrees + 180.0).rem_euclid(360.0) - 180.0
}

/// Yaw 0 looks south (+Z), 90 west.
fn facing(yaw: f32) -> &'static str {
    match ((wrap(yaw) + 45.0).rem_euclid(360.0) / 90.0) as u32 {
        0 => "south (Towards positive Z)",
        1 => "west (Towards negative X)",
        2 => "north (Towards negative Z)",
        _ => "east (Towards positive X)",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facing_follows_the_yaw() {
        assert!(facing(0.0).starts_with("south"));
        assert!(facing(90.0).starts_with("west"));
        assert!(facing(-180.0).starts_with("north"));
        assert!(facing(270.0).starts_with("east"));
    }
}
