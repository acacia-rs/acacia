//! Boat legs over still water (milestone M3): a surface route ([`route`]), a stop BDS's exit search puts the rider
//! on land from ([`stop`]), and a [`BoatPilot`] that paddles it with keys solved by rollouts each tick.
//! Design and decisions: research/pathfind-boats-design.md, docs/pathfind-plan.md "M3".

mod bot;
mod grid;
mod launch;
mod line;
mod pilot;
mod route;
mod solve;
mod stop;
#[cfg(test)]
mod tests;

pub use pilot::{BoatOrder, BoatPilot};
pub use route::Route;

use acacia_physics::{BlockPos, Vec3, WorldView};

use super::profile::MovementProfile;
use crate::human;
use grid::WaterGrid;
pub(crate) use grid::water_level;

#[derive(Debug, Clone, PartialEq)]
pub struct BoatOpts {
    pub profile: MovementProfile,
    /// Leave the boat at the far bank instead of breaking it and picking it up.
    pub keep_boat: bool,
    /// Ticks the paddling may take before it gives up.
    pub max_ticks: u64,
}

impl Default for BoatOpts {
    fn default() -> Self {
        Self { profile: MovementProfile::Optimal, keep_boat: false, max_ticks: 20 * 60 * 5 }
    }
}

/// A planned leg: the route, the bank padding it keeps and where the rider gets off.
#[derive(Debug, Clone, PartialEq)]
pub struct Leg {
    pub route: Route,
    pub pad: f32,
    /// Feet after the dismount at the stop; `None` on a partial route, which ends where the known water does.
    pub exit: Option<Vec3>,
    /// The feet block the rider is to get off by.
    pub target: BlockPos,
}

/// Stops tried, best first, before the leg is given up.
const STOP_TRIES: usize = 8;

/// A leg for a boat floating at `boat` to get its rider off near `target` (feet block), with the rider's seat
/// `seat` (`RiderSeatPosition`). Human keeps a wider berth from banks where the water allows. With no stop known
/// near the target yet (its chunks not loaded), a partial leg towards it.
pub fn plan_leg<W: WorldView>(world: &W, boat: Vec3, target: BlockPos, seat: Vec3, profile: MovementProfile) -> Option<Leg> {
    let grid = WaterGrid::new(world, water_level(boat[1]));
    let pads: &[f32] = match profile {
        MovementProfile::Optimal => &[0.0],
        MovementProfile::Human => &[human::BOAT_BANK_PAD, 0.0],
    };
    let from = [boat[0], boat[2]];
    let stops = stop::stops(&grid, world, target, seat);
    for &pad in pads {
        for (at, exit) in stops.iter().take(STOP_TRIES) {
            if let Some(route) = route::plan(&grid, from, *at, pad).filter(|r| r.complete) {
                return Some(Leg { route, pad, exit: Some(*exit), target });
            }
        }
    }
    let toward = [target[0] as f32 + 0.5, target[2] as f32 + 0.5];
    pads.iter().find_map(|&pad| route::plan(&grid, from, toward, pad).filter(|r| !r.complete).map(|route| Leg { route, pad, exit: None, target }))
}
