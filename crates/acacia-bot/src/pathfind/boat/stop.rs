//! Where a leg stops: a boat centre from which BDS's exit search (`riding::Mount::exit_feet`) puts the rider on
//! land near the target, whatever the boat's yaw and a little drift.

use acacia_physics::{BlockPos, BoatState, Vec3, WorldView};

use super::grid::WaterGrid;
use crate::riding::{Mount, STILL_SQ};

/// Blocks around the target searched for a stop, and the grid step.
const REACH: f32 = 4.0;
const STEP: f32 = 0.1;
/// How far a resting boat may sit from the stop and still give the same exit: a boat beside a bank has 0.3 of its
/// block to rest in for the exit to find that bank.
pub(crate) const DRIFT: f32 = 0.1;
/// A floating boat's wire height above its water block (y 70.95–70.99 on water in the capture).
const FLOAT: f32 = 0.96;
/// Blocks of distance an exit into water counts for.
const WET: f32 = 4.0;

/// Stops nearest first by where the rider lands: (boat centre, exit feet).
pub(crate) fn stops<W: WorldView>(grid: &WaterGrid<'_, W>, world: &W, target: BlockPos, seat: Vec3) -> Vec<([f32; 2], Vec3)> {
    let centre = [target[0] as f32 + 0.5, target[2] as f32 + 0.5];
    let n = (REACH / STEP) as i32;
    let mut out = Vec::new();
    for i in -n..=n {
        for k in -n..=n {
            let p = [centre[0] + i as f32 * STEP, centre[1] + k as f32 * STEP];
            if !grid.clear(p[0], p[1], DRIFT) {
                continue;
            }
            if let Some(feet) = steady_exit(grid.y, p, seat, world) {
                // An exit standing in shallow water is taken only when no bank is near.
                let wet = world.block(feet.map(|c| c.floor() as i32)).liquid.map_or(0.0, |_| WET);
                let d = off_target(feet, target) + (feet[1] - target[1] as f32).abs() + wet;
                out.push((d, p, feet));
            }
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.into_iter().map(|(_, p, feet)| (p, feet)).collect()
}

/// Blocks past the planned exit's distance from the target an exit may land and still end the leg.
const EXIT_SLACK: f32 = 1.0;

/// Where getting off ends the leg: dry ground at the target's level, about as near it as the planned exit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Arrival {
    target: BlockPos,
    reach: f32,
    seat: Vec3,
}

impl Arrival {
    pub(crate) fn new(planned: Vec3, target: BlockPos, seat: Vec3) -> Self {
        Self { target, reach: off_target(planned, target) + EXIT_SLACK, seat }
    }

    /// The boat `st` is at rest where getting off lands within reach.
    pub(crate) fn rests_for<W: WorldView + ?Sized>(&self, st: &BoatState, world: &W) -> bool {
        let motion = [st.vel[0], st.vel[2]];
        if motion[0] * motion[0] + motion[1] * motion[1] > STILL_SQ {
            return false;
        }
        let mount = Mount { vehicle: st.pos, yaw: st.yaw, motion, seat_offset: self.seat };
        mount.exit_feet(world).is_some_and(|f| {
            let dry = world.block(f.map(|c| c.floor() as i32)).liquid.is_none();
            dry && f[1].floor() as i32 == self.target[1] && off_target(f, self.target) <= self.reach
        })
    }
}

fn off_target(feet: Vec3, target: BlockPos) -> f32 {
    (feet[0] - (target[0] as f32 + 0.5)).hypot(feet[2] - (target[2] as f32 + 0.5))
}

/// The exit with the boat at rest at `p`, when any yaw and a little drift give the same spot (the seat aside):
/// `None` otherwise.
fn steady_exit<W: WorldView>(water_y: i32, p: [f32; 2], seat: Vec3, world: &W) -> Option<Vec3> {
    let spread = 2.0 * (seat[0].hypot(seat[2]) + DRIFT) + 0.01;
    let mut found: Option<Vec3> = None;
    for (dx, dz) in [(0.0, 0.0), (DRIFT, 0.0), (-DRIFT, 0.0), (0.0, DRIFT), (0.0, -DRIFT)] {
        for yaw in [0.0, 90.0, 180.0, 270.0] {
            let mount = Mount { vehicle: [p[0] + dx, water_y as f32 + FLOAT, p[1] + dz], yaw, motion: [0.0; 2], seat_offset: seat };
            let feet = mount.exit_feet(world)?;
            match found {
                Some(f) if f[1] != feet[1] || (f[0] - feet[0]).hypot(f[2] - feet[2]) > spread => return None,
                Some(_) => {}
                None => found = Some(feet),
            }
        }
    }
    found
}
