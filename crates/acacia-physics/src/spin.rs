//! Riptide spin, as BDS runs it (docs/research/riptide-impact-exit.md §1): the launch impulse and the stop rules.

use crate::math::{PI32, mc_cos, mc_sin};
use crate::sim::Sim;
use crate::state::PlayerState;
use crate::world::{LiquidKind, WorldView};

/// Spin ticks after which the spin ends, and after which landing ends it (`PostDamageNearbyMobsSystem`).
const SPIN_TICKS: u8 = 19;
const SPIN_LANDING_TICKS: u8 = 5;
const WATER_DRAG: f32 = 0.8;
const WATER_GRAVITY_DRAG: f32 = 0.98;
const GRAVITY: f32 = 0.08;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spin {
    /// A released Riptide trident's level: the next movement tick launches.
    pub launch: Option<u8>,
    ticks: Option<u8>,
}

impl Spin {
    /// The DAMAGENEARBYMOBS flag: the box is horizontal.
    pub fn spinning(&self) -> bool {
        self.ticks.is_some()
    }

    pub(crate) fn end_tick(&mut self, horizontal_collision: bool, on_ground: bool) {
        let Some(t) = self.ticks else { return };
        let done = horizontal_collision || t >= SPIN_TICKS || (t >= SPIN_LANDING_TICKS && on_ground);
        self.ticks = (!done).then_some(t + 1);
    }
}

impl<W: WorldView + ?Sized> Sim<'_, W> {
    /// `StartSpinAttackActionSystem`: adds the look times `(L + 1) · 0.75` to the velocity before travel. On the ground it
    /// pre-undoes this tick's water drag (feet in water, head out) or gravity.
    pub(crate) fn launch_spin(&self, st: &mut PlayerState, in_water: bool) {
        let Some(level) = st.spin.launch.take() else { return };
        let radians = PI32 / 180.0;
        let (yaw, pitch) = (st.yaw * radians, st.pitch * radians);
        let pitch_cos = mc_cos(pitch);
        let (mut x, mut y, mut z) = (-mc_sin(yaw) * pitch_cos, mc_sin(pitch), pitch_cos * mc_cos(yaw));
        let len = (z * z + y * y + x * x).sqrt();
        if len < 0.0001 {
            (x, y, z) = (0.0, 0.0, 0.0);
        } else {
            (x, y, z) = (x / len, -y / len, z / len);
        }
        let f = (f32::from(level) + 1.0) * 0.25 * 3.0;
        let mut vy = y * f;
        if st.on_ground {
            if in_water && !self.eyes_in(st, LiquidKind::Water) {
                vy = vy / WATER_DRAG * WATER_GRAVITY_DRAG;
            } else {
                vy += GRAVITY;
            }
        }
        let v = st.vel;
        st.set_vel([v[0] + x * f, v[1] + vy, v[2] + z * f]);
        st.spin.ticks = Some(0);
    }
}
