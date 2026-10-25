//! What a player's cape and elytra remember between ticks: `ClientAvatarState`'s cloak (a point
//! trailing the player a quarter of the way each tick) and bob, `AvatarRenderer.extractCapeState`
//! reading them, and `ElytraAnimationState` easing the wings (26.3). Checked by the parity
//! harness `capes_elytra`.

use glam::DVec3;

use super::mth::{cos, lerp, pi_div, sin};

/// Blocks a cloak trails by before it jumps to the player instead.
const TELEPORT: f64 = 10.0;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Cloak {
    pub at: DVec3,
    pub old: DVec3,
    pub bob: f32,
    pub bob_old: f32,
    pub walk: f32,
    pub walk_old: f32,
}

impl Cloak {
    /// `ClientAvatarState.tick` at the player's `position`, then `updateBob` with `bob` (the
    /// horizontal speed capped at 0.1 on the ground, else 0).
    pub fn tick(&mut self, position: DVec3, bob: f32) {
        self.walk_old = self.walk;
        self.old = self.at;
        for axis in 0..3 {
            let d = position[axis] - self.at[axis];
            if (-TELEPORT..=TELEPORT).contains(&d) {
                self.at[axis] += d * 0.25;
            } else {
                self.at[axis] = position[axis];
                self.old[axis] = position[axis];
            }
        }
        self.bob_old = self.bob;
        self.bob += (bob - self.bob) * 0.4;
    }

    /// `addWalkDistance`: the local player's, 0.6 of each horizontal move.
    pub fn walked(&mut self, distance: f32) {
        self.walk += distance;
    }
}

/// `capeFlap`, `capeLean`, `capeLean2` for a player at `position` (interpolated), its body turned
/// `body_yaw` degrees, `partial` into the tick, `fall_flying` its `fallFlyingScale`.
pub fn cape(cloak: &Cloak, position: DVec3, body_yaw: f32, partial: f32, fall_flying: f32) -> [f32; 3] {
    let t = f64::from(partial);
    let d = cloak.old + (cloak.at - cloak.old) * t - position;
    let yaw = body_yaw * pi_div(180.0);
    let (forward_x, forward_z) = (f64::from(sin(yaw)), f64::from(-cos(yaw)));
    let flap = (d.y as f32 * 10.0).clamp(-6.0, 32.0);
    let lean = ((d.x * forward_x + d.z * forward_z) as f32 * 100.0 * (1.0 - fall_flying)).clamp(0.0, 150.0);
    let lean2 = ((d.x * forward_z - d.z * forward_x) as f32 * 100.0).clamp(-20.0, 20.0);
    let pow = lerp(partial, cloak.bob_old, cloak.bob);
    let walk = lerp(partial, cloak.walk_old, cloak.walk);
    [flap + sin(walk * 6.0) * 32.0 * pow, lean, lean2]
}

/// `ElytraAnimationState`: (x, y, z) rotations now and a tick ago.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Wings {
    pub rot: [f32; 3],
    pub old: [f32; 3],
}

impl Wings {
    /// One tick: gliding with `movement` (blocks a tick), or crouching, or neither.
    pub fn tick(&mut self, gliding: Option<DVec3>, crouching: bool) {
        self.old = self.rot;
        let rest = (pi_div(12.0), 0.0, -pi_div(12.0));
        let (x, y, z) = match gliding {
            Some(movement) => {
                // Vec3.normalize: anything shorter than 1e-5 is zero.
                let down = if movement.length() < 1e-5 { 0.0 } else { -(movement / movement.length()).y };
                let ratio = if movement.y < 0.0 { 1.0 - down.powf(1.5) as f32 } else { 1.0 };
                (lerp(ratio, pi_div(12.0), pi_div(9.0)), 0.0, lerp(ratio, -pi_div(12.0), -pi_div(2.0)))
            }
            None if crouching => ((std::f64::consts::PI * 2.0 / 9.0) as f32, 0.087_266_46, -pi_div(4.0)),
            None => rest,
        };
        for (r, target) in self.rot.iter_mut().zip([x, y, z]) {
            *r += (target - *r) * 0.3;
        }
    }

    /// `elytraRotX`, `elytraRotY`, `elytraRotZ`.
    pub fn at(&self, partial: f32) -> [f32; 3] {
        std::array::from_fn(|i| lerp(partial, self.old[i], self.rot[i]))
    }
}
