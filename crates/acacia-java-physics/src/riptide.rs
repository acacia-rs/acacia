//! Riptide: `TridentItem.releaseUsing`'s launch and `LivingEntity`'s auto spin attack.

use crate::aabb::Aabb;
use crate::collide::Blocks;
use crate::mth::{cos, sin, sqrt, DEG_TO_RAD};
use crate::player::Player;

/// `startAutoSpinAttack(20, …)`.
const SPIN_TICKS: i32 = 20;

/// An entity near the player while it spins, as `checkAutoSpinAttack` sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Nearby {
    pub bounds: Aabb,
    pub living: bool,
}

impl Player {
    /// Released a riptide trident held long enough, in water or rain: the push along the look, the spin, and a lift
    /// off the ground. `strength` is the enchantment's `trident_spin_attack_strength`.
    pub fn riptide(&mut self, world: &impl Blocks, strength: f32) {
        let (y_rot, x_rot) = ((self.yaw * DEG_TO_RAD) as f64, (self.pitch * DEG_TO_RAD) as f64);
        let mut xd = -sin(y_rot) * cos(x_rot);
        let mut yd = -sin(x_rot);
        let mut zd = cos(y_rot) * cos(x_rot);
        let dist = sqrt(xd * xd + yd * yd + zd * zd);
        xd *= strength / dist;
        yd *= strength / dist;
        zd *= strength / dist;
        self.delta = [self.delta[0] + xd as f64, self.delta[1] + yd as f64, self.delta[2] + zd as f64];
        self.auto_spin_ticks = SPIN_TICKS;
        if self.on_ground {
            self.move_by(world, [0.0, 1.1999999_f32 as f64, 0.0]);
        }
    }

    pub fn auto_spin_ticks(&self) -> i32 {
        self.auto_spin_ticks
    }

    /// `aiStep`'s countdown and `checkAutoSpinAttack`: the first entity touched ends the spin (a living one bounces
    /// the player back); with none touched, a wall ends it.
    pub(crate) fn tick_auto_spin(&mut self, before: Aabb) {
        if self.auto_spin_ticks <= 0 {
            return;
        }
        self.auto_spin_ticks -= 1;
        let swept = before.minmax(&self.bounding_box());
        let mut touched = self.nearby.iter().filter(|e| e.bounds.intersects(&swept)).peekable();
        if touched.peek().is_some() {
            if touched.any(|e| e.living) {
                self.auto_spin_ticks = 0;
                self.delta = self.delta.map(|v| v * -0.2);
            }
        } else if self.horizontal_collision {
            self.auto_spin_ticks = 0;
        }
    }
}
