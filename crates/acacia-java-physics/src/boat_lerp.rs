//! The driven boat's `LinearInterpolationHandler`: a server teleport moves it over three ticks, and the boat's
//! own moves meanwhile carry the target along.

use crate::aabb::Aabb;
use crate::boat::{Boat, HEIGHT, WIDTH};
use crate::collide::Blocks;
use crate::mth::length_sqr;

const STEPS: u32 = 3;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct BoatLerp {
    target: [f64; 3],
    y_rot: f32,
    x_rot: f32,
    remaining: u32,
    /// Position and rotation after the last step, which the boat's own moves are measured from.
    last: ([f64; 3], f32, f32),
}

impl Boat {
    /// `moveOrInterpolateTo`, as `handleTeleportEntity` calls it for a ticking entity.
    pub fn interpolate_to(&mut self, pos: [f64; 3], yaw: f32, pitch: f32) {
        let l = &self.lerp;
        if l.remaining > 0 && (l.target, l.y_rot, l.x_rot) == (pos, yaw, pitch) {
            return;
        }
        self.lerp = BoatLerp { target: pos, y_rot: yaw, x_rot: pitch, remaining: STEPS, last: (self.pos, self.yaw, self.pitch) };
    }

    /// `AbstractInterpolationHandler.interpolate`, from `commonTick` before the boat ticks.
    pub(crate) fn interpolate(&mut self, world: &impl Blocks) {
        if self.lerp.remaining == 0 {
            return;
        }
        let (last_pos, last_yaw, last_pitch) = self.lerp.last;
        let moved = [0, 1, 2].map(|a| self.pos[a] - last_pos[a]);
        if length_sqr(moved) > 1.0E-5_f32 as f64 {
            let target = [0, 1, 2].map(|a| self.lerp.target[a] + moved[a]);
            if self.colliders(world).no_collision(&Aabb::of_entity(target, WIDTH, HEIGHT)) {
                self.lerp.target = target;
            }
        }
        self.lerp.y_rot += self.yaw - last_yaw;
        self.lerp.x_rot += self.pitch - last_pitch;
        let alpha = 1.0 / self.lerp.remaining as f64;
        let l = &self.lerp;
        self.pos = [0, 1, 2].map(|a| self.pos[a] + alpha * (l.target[a] - self.pos[a]));
        let yaw = self.yaw as f64;
        self.yaw = (yaw + alpha * wrap_degrees(l.y_rot as f64 - yaw)) as f32;
        self.pitch = (self.pitch as f64 + alpha * (l.x_rot as f64 - self.pitch as f64)) as f32;
        self.lerp.remaining -= 1;
        self.lerp.last = (self.pos, self.yaw, self.pitch);
    }
}

/// `Mth.wrapDegrees(double)`.
fn wrap_degrees(d: f64) -> f64 {
    let d = d % 360.0;
    let d = if d >= 180.0 { d - 360.0 } else { d };
    if d < -180.0 { d + 360.0 } else { d }
}
