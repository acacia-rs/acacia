//! Elytra autopilot (milestone M2, overworld cruise): take off, climb to a cruise height over the known terrain,
//! fly with a pitch solved by rollouts each tick, descend and land. Design: docs/pathfind-plan.md "M2 design".

mod bot;
mod pilot;
mod solve;
mod surface;
#[cfg(test)]
mod tests;

pub use pilot::{FlightPilot, Order};
pub use surface::{ChunkSurface, Surface, TopCache};

use super::profile::MovementProfile;

#[derive(Debug, Clone, PartialEq)]
pub struct FlyOpts {
    pub profile: MovementProfile,
    /// Blocks kept between the feet and the highest terrain under the corridor while cruising.
    pub cruise_margin: i32,
    /// Landing spots up to this far (blocks, horizontal) from the target are taken.
    pub land_radius: i32,
    /// Rockets kept back for the landing and for climbing over terrain found late.
    pub rocket_reserve: u32,
    /// Ticks the flight may take before it gives up.
    pub max_ticks: u64,
}

impl Default for FlyOpts {
    fn default() -> Self {
        Self { profile: MovementProfile::Optimal, cruise_margin: 24, land_radius: 3, rocket_reserve: 2, max_ticks: 20 * 60 * 5 }
    }
}
