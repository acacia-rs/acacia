//! What a ridden mount changes about the body [`Player`] physics move: its size, step, speed and input.

use crate::player::{Player, STEP_HEIGHT, WIDTH};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MountBody {
    pub width: f32,
    pub height: f32,
    /// `getMaxUpStep`: at least 1 under a player.
    pub step: f32,
    /// `setSpeed(getRiddenSpeed)` this tick; flying in air uses a tenth (`getFlyingSpeed`).
    pub speed: f32,
    /// `travel`'s input, which flying and swimming mounts give a vertical part.
    pub input: [f64; 3],
    pub climbs: bool,
    pub water_slowdown: f32,
    pub pushed_by_fluids: bool,
    /// Lava sources hold it up (`canStandOnFluid`), so it walks there instead of swimming.
    pub stands_on_lava: bool,
}

impl Player {
    /// Width and height.
    pub(crate) fn size(&self) -> (f32, f32) {
        self.mount.map_or((WIDTH, self.pose.height()), |m| (m.width, m.height))
    }

    pub(crate) fn step_height(&self) -> f32 {
        self.mount.map_or(STEP_HEIGHT, |m| m.step)
    }

    /// Mobs' default eye height; it only decides the 0.4 fluid jump threshold and eye-in-fluid flags.
    pub(crate) fn eye_height(&self) -> f32 {
        self.mount.map_or(self.pose.eye_height(), |m| m.height * 0.85)
    }
}
