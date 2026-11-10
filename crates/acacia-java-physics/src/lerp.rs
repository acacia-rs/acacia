//! A driven vehicle's `LinearInterpolationHandler`: a server teleport moves it over three ticks, and the
//! vehicle's own moves meanwhile carry the target along.

use crate::aabb::Aabb;
use crate::boat::{Boat, HEIGHT, WIDTH};
use crate::collide::Blocks;
use crate::mth::length_sqr;

const STEPS: u32 = 3;

/// Where a vehicle stands: position, yaw, pitch.
pub(crate) type Pose = ([f64; 3], f32, f32);

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Lerp {
    target: [f64; 3],
    y_rot: f32,
    x_rot: f32,
    remaining: u32,
    /// The pose after the last step, which the vehicle's own moves are measured from.
    last: Pose,
}

impl Lerp {
    /// `moveOrInterpolateTo`, as `handleTeleportEntity` calls it for a ticking entity.
    pub(crate) fn start(&mut self, now: Pose, target: Pose) {
        let (pos, yaw, pitch) = target;
        if self.remaining > 0 && (self.target, self.y_rot, self.x_rot) == (pos, yaw, pitch) {
            return;
        }
        *self = Lerp { target: pos, y_rot: yaw, x_rot: pitch, remaining: STEPS, last: now };
    }

    /// `AbstractInterpolationHandler.interpolate`, from `commonTick`; `fits` says whether the vehicle fits at a spot.
    pub(crate) fn step(&mut self, pose: &mut Pose, fits: impl Fn([f64; 3]) -> bool) {
        if self.remaining == 0 {
            return;
        }
        let (last_pos, last_yaw, last_pitch) = self.last;
        let (pos, yaw, pitch) = pose;
        let moved = [0, 1, 2].map(|a| pos[a] - last_pos[a]);
        if length_sqr(moved) > 1.0E-5_f32 as f64 {
            let target = [0, 1, 2].map(|a| self.target[a] + moved[a]);
            if fits(target) {
                self.target = target;
            }
        }
        self.y_rot += *yaw - last_yaw;
        self.x_rot += *pitch - last_pitch;
        let alpha = 1.0 / self.remaining as f64;
        *pos = [0, 1, 2].map(|a| pos[a] + alpha * (self.target[a] - pos[a]));
        let old_yaw = *yaw as f64;
        *yaw = (old_yaw + alpha * wrap_degrees(self.y_rot as f64 - old_yaw)) as f32;
        *pitch = (*pitch as f64 + alpha * (self.x_rot as f64 - *pitch as f64)) as f32;
        self.remaining -= 1;
        self.last = *pose;
    }
}

impl Boat {
    pub fn interpolate_to(&mut self, pos: [f64; 3], yaw: f32, pitch: f32) {
        self.lerp.start((self.pos, self.yaw, self.pitch), (pos, yaw, pitch));
    }

    pub(crate) fn interpolate(&mut self, world: &impl Blocks) {
        let mut pose = (self.pos, self.yaw, self.pitch);
        let colliders = self.colliders(world);
        let mut lerp = self.lerp.clone();
        lerp.step(&mut pose, |at| colliders.no_collision(&Aabb::of_entity(at, WIDTH, HEIGHT)));
        self.lerp = lerp;
        (self.pos, self.yaw, self.pitch) = pose;
    }
}

/// `Mth.wrapDegrees(double)`.
fn wrap_degrees(d: f64) -> f64 {
    let d = d % 360.0;
    let d = if d >= 180.0 { d - 360.0 } else { d };
    if d < -180.0 { d + 360.0 } else { d }
}
