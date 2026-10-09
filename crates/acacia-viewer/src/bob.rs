//! View bobbing, the hurt and death roll and the movement field of view, as Java's `GameRenderer`
//! (`bobView`, `bobHurt`, `tickFov`) and `AbstractClientPlayer.getFieldOfViewModifier` run them:
//! ticked from the simulated player, eased between ticks.

use std::time::{Duration, Instant};

use glam::{DVec3, Mat4, Vec3};

use crate::me::Me;

const TICK: Duration = Duration::from_millis(50);

#[derive(Default)]
pub struct Bob {
    last_eye: Option<DVec3>,
    walk: f32,
    walk_before: f32,
    bob: f32,
    bob_before: f32,
    fov: f32,
    fov_before: f32,
    ticked: Option<Instant>,
    hurts: Option<u32>,
    hurt_at: Option<Instant>,
    dead_since: Option<Instant>,
}

impl Bob {
    pub fn tick(&mut self, me: &Me, now: Instant) {
        let moved = self.last_eye.replace(me.eye).map_or(0.0, |last| (me.eye - last).with_y(0.0).length() as f32);
        self.walk_before = self.walk;
        self.walk += moved * 0.6;
        self.bob_before = self.bob;
        let target = if me.on_ground && me.alive { moved.min(0.1) } else { 0.0 };
        self.bob += (target - self.bob) * 0.4;
        self.fov_before = self.fov;
        let fov = if self.fov == 0.0 { 1.0 } else { self.fov };
        self.fov = fov + (fov_modifier(me.sprinting, me.flying) - fov) * 0.5;
        if self.hurts.replace(me.hurts).is_some_and(|before| me.hurts > before) {
            self.hurt_at = Some(now);
        }
        self.dead_since = if me.alive { None } else { self.dead_since.or(Some(now)) };
        self.ticked = Some(now);
    }

    /// The camera's field of view (from the chosen `base` degrees) and sway at `now`; without
    /// `bobbing` only the hurt roll is left, as in Java.
    pub fn apply(&self, base: f32, bobbing: bool, fov_y: &mut f32, bob: &mut Mat4, now: Instant) {
        let t = self.ticked.map_or(1.0, |at| (now.saturating_duration_since(at).as_secs_f32() / TICK.as_secs_f32()).min(1.0));
        let fov = if self.fov == 0.0 { 1.0 } else { self.fov_before + (self.fov - self.fov_before) * t };
        *fov_y = (base * fov.max(0.1)).to_radians();
        let ticks_since = |at: Option<Instant>| at.map(|at| now.saturating_duration_since(at).as_secs_f32() / TICK.as_secs_f32());
        let roll = hurt_roll(ticks_since(self.hurt_at), ticks_since(self.dead_since));
        let amount = if bobbing { self.bob_before + (self.bob - self.bob_before) * t } else { 0.0 };
        *bob = roll * sway(-(self.walk + (self.walk - self.walk_before) * t), amount);
    }
}

/// `bobHurt`: a 14° roll easing out over the 10 hurt ticks (Bedrock sends no hurt direction, so
/// none), and the tilt toward 40° while dead.
fn hurt_roll(hurt_ticks: Option<f32>, dead_ticks: Option<f32>) -> Mat4 {
    if let Some(dead) = dead_ticks {
        return Mat4::from_rotation_z((40.0 - 8000.0 / (dead + 200.0)).to_radians());
    }
    let left = hurt_ticks.map_or(0.0, |ticks| (10.0 - ticks) / 10.0);
    if left <= 0.0 {
        return Mat4::IDENTITY;
    }
    Mat4::from_rotation_z((-(left.powi(4) * std::f32::consts::PI).sin() * 14.0).to_radians())
}

/// Sprinting moves at 1.3× the walking speed, which widens the view by half that; flying by a tenth.
fn fov_modifier(sprinting: bool, flying: bool) -> f32 {
    let flying = if flying { 1.1 } else { 1.0 };
    let speed = if sprinting { 1.3 } else { 1.0 };
    flying * (speed + 1.0) / 2.0
}

/// `bobView`: `walked` is the negated walk distance, `amount` the eased bob.
fn sway(walked: f32, amount: f32) -> Mat4 {
    use std::f32::consts::PI;
    let (s, c) = (walked * PI).sin_cos();
    Mat4::from_translation(Vec3::new(s * amount * 0.5, -(c * amount).abs(), 0.0))
        * Mat4::from_rotation_z((s * amount * 3.0).to_radians())
        * Mat4::from_rotation_x(((walked * PI - 0.2).cos() * amount).abs() * 5.0_f32.to_radians())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standing_still_neither_sways_nor_widens() {
        assert_eq!(sway(1.3, 0.0), Mat4::IDENTITY);
        assert!((fov_modifier(true, false) - 1.15).abs() < 1e-6);
        assert_eq!(fov_modifier(false, false), 1.0);
        assert_eq!((hurt_roll(None, None), hurt_roll(Some(10.0), None)), (Mat4::IDENTITY, Mat4::IDENTITY));
    }

    #[test]
    fn a_hurt_rolls_the_view_and_death_tilts_it() {
        let roll = |m: Mat4| m.transform_vector3(Vec3::X).y.asin().to_degrees();
        // sin(g⁴π) peaks at g⁴ = 0.5: about 1.6 ticks into the hurt.
        let peak = (1.0 - 0.5f32.powf(0.25)) * 10.0;
        assert!((roll(hurt_roll(Some(peak), None)) + 14.0).abs() < 0.01);
        assert!((roll(hurt_roll(None, Some(0.0))).abs() - 0.0).abs() < 0.01 && (roll(hurt_roll(None, Some(1e6))) - 40.0).abs() < 0.1);
    }
}
