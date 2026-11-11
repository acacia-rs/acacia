//! Re-solving a leap live, for the follower: from the state the bot is really in, not the planned one, within
//! the follower's per-tick rollout budget ([`Spend`]).

use acacia_physics::Vec3;

use super::leap::{JumpAt, LeapCtl, MAX_LEAP_TICKS};
use super::rollout::{Body, Spend};
use super::solve::{Attempt, MAX_COAST, MAX_TAKEOFF, Shape, Solver};
use crate::movement::StepIn;

/// Rollouts a takeoff bisection or a coast search may run (1 + log2 of the range, rounded up, plus slack).
const SEARCH_ROLLOUTS: u64 = 6;
/// Arriving this near the planned velocity (1/100 blocks/tick) ends the search.
const CLOSE_ENOUGH: u32 = 3;

impl<W: ?Sized> Solver<'_, W> {
    /// A leap that drifted, solved again from the live state: an airborne one by its coast, else by takeoff
    /// (earliest reaching one first, by bisection) and coast. The landing arriving nearest the planned
    /// horizontal velocity `want` (if any) wins, then the fastest, so the moves after it still start as
    /// planned. Within the budget only: the best landing found before it ran out.
    pub fn retry<S: StepIn<W>>(&mut self, ctl: &LeapCtl, live: &S, shape: Shape, want: Option<Vec3>) -> Option<(LeapCtl, Vec<Body>)> {
        let off = |v: Vec3| want.map_or(0, |w| ((v[0] - w[0]).hypot(v[2] - w[2]) * 100.0).round() as u32);
        let mut best: Option<((u32, u32), LeapCtl, Vec<Body>)> = None;
        let mut keep = |a: Attempt| {
            let key = (off(a.end.vel), a.ticks());
            if best.as_ref().is_none_or(|b| key < b.0) {
                best = Some((key, a.ctl, a.bodies));
            }
            key.0 <= CLOSE_ENOUGH
        };
        let search = u64::from(MAX_LEAP_TICKS) * SEARCH_ROLLOUTS;
        let coasts = || [None].into_iter().chain((0..=MAX_COAST).map(Some));
        if ctl.airborne() {
            for coast in coasts() {
                if !self.affords(u64::from(MAX_LEAP_TICKS)) {
                    break;
                }
                let mut c = *ctl;
                c.coast = coast;
                let a = self.attempt(live, c);
                if a.clean && keep(a) {
                    break;
                }
            }
            return best.map(|(_, c, b)| (c, b));
        }
        let fresh = |yaw: f32, jump: JumpAt| {
            let mut c = LeapCtl::new(ctl.target, ctl.feet, jump, None, ctl.aim);
            (c.wet, c.yaw_off, c.air_aim, c.slack) = (ctl.wet, yaw, shape.air_aim, ctl.slack);
            c
        };
        'yaws: for &yaw in shape.yaws {
            if !self.affords(search) {
                break;
            }
            // Even the edge jump short (a narrow stone with momentum): an early takeoff may still land.
            let first = self.first_reaching(live, fresh(yaw, JumpAt::Never)).map_or(0, |(k, _)| k);
            let mut jumps: Vec<JumpAt> = (first..=MAX_TAKEOFF).map(JumpAt::Tick).collect();
            if shape.drop || ctl.jump == JumpAt::Never {
                jumps.push(JumpAt::Never);
            }
            let (mut last, mut past_edge) = (None, false);
            for jump in jumps {
                let tick = jump != JumpAt::Never;
                if tick && past_edge {
                    continue;
                }
                if !self.affords(u64::from(MAX_LEAP_TICKS) + search) {
                    break 'yaws;
                }
                let a = self.attempt(live, fresh(yaw, jump));
                // A jump due after the edge never comes: the later ones neither.
                past_edge |= tick && a.takeoff.is_none();
                if tick && (past_edge || last.replace(a.takeoff) == Some(a.takeoff)) {
                    continue;
                }
                let Ok(a) = self.land(live, a) else { continue };
                let (landed, found) = (a.ctl, keep(a));
                if found {
                    break 'yaws;
                }
                // Landed, but off the planned velocity: the other coasts arrive at other speeds.
                for coast in coasts().filter(|&k| k != landed.coast) {
                    if !self.affords(u64::from(MAX_LEAP_TICKS)) {
                        break 'yaws;
                    }
                    let mut c = landed;
                    c.coast = coast;
                    let a = self.attempt(live, c);
                    if a.clean && keep(a) {
                        break 'yaws;
                    }
                }
            }
        }
        best.map(|(_, c, b)| (c, b))
    }
}

impl Spend {
    /// Books what `s` simulated.
    pub(crate) fn charge<W: ?Sized>(&mut self, s: &Solver<'_, W>) {
        self.ticks += s.ticks;
        self.short |= s.starved;
    }
}
