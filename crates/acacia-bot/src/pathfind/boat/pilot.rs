//! One boat leg, tick by tick, network-free: from the live (corrected) boat it sets the paddle keys and the
//! rider's look, and says when the boat has come to rest at the stop.

use acacia_physics::{BoatState, Vec3, WorldView};

use super::Leg;
use super::grid::WaterGrid;
use super::line::{Polyline, dist};
use super::solve::{Course, Keys, NONE, hold_for, solve, wrap};
use super::stop::Arrival;
use crate::cadence::splitmix;
use crate::human;
use crate::movement::Controls;
use crate::pathfind::profile::{Aim, MovementProfile};

#[derive(Debug, Clone, PartialEq)]
pub enum BoatOrder {
    Paddle,
    /// At rest at the stop.
    Arrived,
    Fail(String),
}

/// Ticks without a block of progress before the leg gives up.
const STUCK_TICKS: u64 = 200;
/// Distance off the route allowed without cost: Optimal follows the smoothed line, Human may wander a little.
const TOLERANCE: [f32; 2] = [0.3, 0.8];
const WINDOW: usize = 6;
/// Degrees either side of the bow the Human rider looks at most (a seated player turns up to 90).
const LOOK_LIMIT: f32 = 85.0;
/// Human rider pitch: a little down, at the water ahead.
const HUMAN_PITCH: f32 = 10.0;

pub struct BoatPilot {
    line: Polyline,
    /// The route's water level, and the exit when it ends at the stop.
    y: i32,
    arrival: Option<Arrival>,
    /// The rider's `RiderSeatPosition`.
    seat: Vec3,
    profile: MovementProfile,
    /// Bank padding the route was planned with.
    pad: f32,
    max_ticks: u64,
    tick: u64,
    seg: usize,
    held: Keys,
    /// Human: the held keys' window ends at this tick.
    until: u64,
    /// Human: keys decided on, pressed at the tick given, held for the ticks given.
    pending: Option<(u64, Keys, u32)>,
    aim: Aim,
    rng: u64,
    best: (f32, u64),
}

impl BoatPilot {
    /// Paddles `leg` for a rider seated at `seat` (`RiderSeatPosition`).
    pub fn new(leg: &Leg, seat: Vec3, profile: MovementProfile, max_ticks: u64, seed: u64) -> Self {
        let mut rng = seed;
        let aim = Aim::new(profile, splitmix(&mut rng));
        let mut pilot = Self {
            line: Polyline::new(Vec::new()),
            y: 0,
            arrival: None,
            seat,
            profile,
            pad: 0.0,
            max_ticks,
            tick: 0,
            seg: 0,
            held: NONE,
            until: 0,
            pending: None,
            aim,
            rng,
            best: (f32::MIN, 0),
        };
        pilot.reroute(leg);
        pilot
    }

    /// Follows `leg` from now on (planned from where the boat is), keeping the keys held and the clock.
    pub fn reroute(&mut self, leg: &Leg) {
        (self.line, self.y, self.pad) = (Polyline::new(leg.route.points.clone()), leg.route.y, leg.pad);
        self.arrival = leg.exit.map(|exit| Arrival::new(exit, leg.target, self.seat));
        (self.seg, self.best) = (0, (f32::MIN, self.tick));
    }

    /// The route ends at the stop.
    pub fn complete(&self) -> bool {
        self.arrival.is_some()
    }

    /// Route left ahead of the boat at `p` (boat centre, horizontal).
    pub fn left(&self, p: [f32; 2]) -> f32 {
        self.line.length() - self.line.locate(p, self.seg, WINDOW).s
    }

    /// Sets `c` for the boat's next tick from `st`, its state now.
    pub fn tick<V: WorldView + ?Sized>(&mut self, st: &BoatState, world: &V, c: &mut Controls) -> BoatOrder {
        self.tick += 1;
        let p = [st.pos[0], st.pos[2]];
        let place = self.line.locate(p, self.seg, WINDOW);
        self.seg = place.seg;
        if place.s > self.best.0 + 1.0 {
            self.best = (place.s, self.tick);
        }
        let grid = WaterGrid::new(world, self.y);
        if self.course(&grid).arrived(st, world) {
            (c.forward, c.strafe) = (0.0, 0.0);
            return BoatOrder::Arrived;
        }
        if self.tick > self.max_ticks {
            return BoatOrder::Fail(format!("boat leg not done after {} ticks", self.max_ticks));
        }
        if self.tick - self.best.1 > STUCK_TICKS {
            return BoatOrder::Fail(format!("no progress for {STUCK_TICKS} ticks at {p:?}"));
        }
        match self.profile {
            MovementProfile::Optimal => self.held = solve(st, self.seg, self.held, 0, [1, 1], &self.course(&grid), world),
            MovementProfile::Human => self.human_keys(st, &grid, world),
        }
        [c.strafe, c.forward] = self.held;
        self.look(st, place.s, c);
        BoatOrder::Paddle
    }

    fn course<'a, 'w, V: WorldView + ?Sized>(&'a self, grid: &'a WaterGrid<'w, V>) -> Course<'a, 'w, V> {
        let human = self.profile == MovementProfile::Human;
        let tolerance = TOLERANCE[usize::from(human)] + self.pad;
        // Human brakes a reaction late at worst.
        let lag = if human { human::BOAT_TURN_REACTION.1 } else { 0 };
        Course { line: &self.line, grid, tolerance, stop: self.arrival, lag }
    }

    /// Keys held for a window, and new ones pressed a reaction time after they are chosen.
    fn human_keys<V: WorldView + ?Sized>(&mut self, st: &BoatState, grid: &WaterGrid<'_, V>, world: &V) {
        if let Some((at, keys, hold)) = self.pending.filter(|&(at, ..)| self.tick >= at) {
            (self.held, self.until, self.pending) = (keys, at + u64::from(hold), None);
        }
        if self.pending.is_none() && self.tick >= self.until {
            let reaction = self.draw(human::BOAT_TURN_REACTION);
            let near = self.arrival.is_some() && self.left([st.pos[0], st.pos[2]]) < human::BOAT_TAP_REACH;
            let tap = self.draw(human::BOAT_TAP_HOLD);
            let holds = [if near { tap } else { self.draw(human::BOAT_KEY_HOLD) }, tap];
            let keys = solve(st, self.seg, self.held, reaction, holds, &self.course(grid), world);
            let hold = hold_for(keys, holds);
            self.pending = Some((self.tick + u64::from(reaction), keys, hold));
        }
    }

    fn draw(&mut self, (lo, hi): (u32, u32)) -> u32 {
        lo + (splitmix(&mut self.rng) % u64::from(hi - lo + 1)) as u32
    }

    /// Optimal looks along the bow; Human eases towards the route ahead, within the 90° either side of the bow
    /// a seated player can turn.
    fn look(&mut self, st: &BoatState, s: f32, c: &mut Controls) {
        // A player's yaw faces (-sin, cos), so the bow's is the boat's yaw less 90.
        let bow_yaw = wrap(st.yaw - 90.0);
        let (yaw, pitch) = match self.profile {
            MovementProfile::Optimal => (bow_yaw, 0.0),
            MovementProfile::Human => {
                let p = [st.pos[0], st.pos[2]];
                let ahead = self.line.at(s + human::BOAT_LOOK_LEAD);
                let want = if dist(ahead, p) > 0.5 { (p[0] - ahead[0]).atan2(ahead[1] - p[1]).to_degrees() } else { bow_yaw };
                let off = wrap(want - bow_yaw).clamp(-LOOK_LIMIT, LOOK_LIMIT);
                let yaw = self.aim.turn(c.yaw, bow_yaw + off);
                (wrap(yaw), c.pitch + (HUMAN_PITCH - c.pitch) * human::AIM_EASE)
            }
        };
        (c.yaw, c.pitch) = (yaw, pitch);
    }
}
