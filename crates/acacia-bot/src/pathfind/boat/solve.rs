//! The keys a paddling boat holds next: candidates are rolled out with `boat_tick` and scored on progress along
//! the route, distance off it, bank contact and, near the end, how close to the stop it comes to rest.

use acacia_physics::{BoatState, WorldView, boat_tick};

use super::grid::WaterGrid;
use super::line::{Polyline, dist};
use super::stop::Arrival;

/// [strafe, forward]; strafe + is the left paddle's key (A), which turns the bow left.
pub(crate) type Keys = [f32; 2];

pub(crate) const W: Keys = [0.0, 1.0];
pub(crate) const S: Keys = [0.0, -1.0];
pub(crate) const NONE: Keys = [0.0, 0.0];
const FIRSTS: [Keys; 9] = [W, [1.0, 1.0], [-1.0, 1.0], [1.0, 0.0], [-1.0, 0.0], NONE, S, [1.0, -1.0], [-1.0, -1.0]];

/// Look-ahead distances (blocks) of the follow policies tried, as zuri's pure pursuit.
const LOOKS: [f32; 4] = [2.5, 4.0, 6.0, 9.0];
const ROLLOUT_TICKS: u32 = 40;
/// Segments ahead of the last known one a rollout may place the boat on.
const WINDOW: usize = 6;

/// Heading error (degrees) under which the policy paddles straight, and over which it turns on one paddle.
const STRAIGHT: f32 = 5.0;
const PIVOT: f32 = 55.0;
/// Forward speed over which a turn past `PIVOT` / 2 is taken on one paddle, and one past `PIVOT` braking.
const TURN_SPEED: f32 = 0.2;
/// Ticks of turning momentum the policy allows for: it steers by where the bow will point.
const LEAD: f32 = 3.0;
/// Forward speed (blocks/tick) under which braking stops.
pub(crate) const STOPPED: f32 = 0.01;
/// Route left (blocks) where the approach starts, and the ramp's speed per block to the stop.
const APPROACH: f32 = 5.0;
const APPROACH_GAIN: f32 = 0.09;
const RAMP_SLACK: f32 = 0.03;
const CRUISE: f32 = 0.38;
/// Within this of the stop the boat only brakes.
const ARRIVED: f32 = 0.06;
/// A stop behind the bow nearer than this is backed onto, not turned round for.
const REVERSE_REACH: f32 = 3.0;

const WALL: f32 = 1e4;
const OFF_W: f32 = 0.3;
/// Weight per block of braking distance past the stop, and per tick not yet at rest there.
const SPEED_W: f32 = 20.0;
const TIME_W: f32 = 0.01;
/// Score over the route's length for coming to rest on the exit: any arrival beats any near miss.
const ARRIVAL: f32 = 1.0;
/// Blocks S takes to stop the boat per block/tick of speed (1.11 from the 0.378 cruise, `boat_shapes`), rounded up.
const BRAKE: f32 = 3.0;
/// Touching distance kept from banks in rollouts.
const CONTACT_PAD: f32 = 0.02;
/// The stop lies against its bank: this near it, nudging a bank at up to this speed (blocks/tick) is no contact.
const NUDGE_REACH: f32 = 1.5;
const NUDGE_SPEED: f32 = 0.15;
/// Only this near the stop is a rest checked against the exit: wide, as Human dithered 200 ticks lining up on
/// the stop when resting short of it beside the same bank lands as well.
const REST_REACH: f32 = 2.5;

/// What a rollout is scored against.
pub(crate) struct Course<'a, 'w, V: WorldView + ?Sized> {
    pub line: &'a Polyline,
    pub grid: &'a WaterGrid<'w, V>,
    /// Distance off the line allowed without cost.
    pub tolerance: f32,
    /// The line ends at a stop to come to rest at, not just where the known water ends.
    pub stop: Option<Arrival>,
    /// Ticks the driver may take to act on a change: the approach allows for coasting that long before braking.
    pub lag: u32,
}

impl<V: WorldView + ?Sized> Course<'_, '_, V> {
    /// The boat rests where getting off lands on the planned exit.
    pub fn arrived(&self, st: &BoatState, world: &V) -> bool {
        self.stop.is_some_and(|a| dist([st.pos[0], st.pos[2]], self.line.end()) < REST_REACH && a.rests_for(st, world))
    }
}

/// The bow's direction in radians, as `atan2(x, z)` of where it points.
pub(crate) fn bow(yaw: f32) -> f32 {
    (90.0 - yaw).to_radians()
}

pub(crate) fn wrap(deg: f32) -> f32 {
    (deg + 180.0).rem_euclid(360.0) - 180.0
}

/// Speed along the bow.
pub(crate) fn forward_speed(st: &BoatState) -> f32 {
    let a = bow(st.yaw);
    st.vel[0] * a.sin() + st.vel[2] * a.cos()
}

/// Pure pursuit with an approach: steer for the point `look` blocks ahead; near the stop, hold the speed under a
/// ramp down to it (S over the ramp, W under it).
pub(crate) fn policy<V: WorldView + ?Sized>(st: &BoatState, seg: &mut usize, look: f32, course: &Course<'_, '_, V>) -> Keys {
    let p = [st.pos[0], st.pos[2]];
    let place = course.line.locate(p, *seg, WINDOW);
    *seg = place.seg;
    let to_end = dist(p, course.line.end());
    let aim = course.line.at(place.s + look.min(to_end));
    let want = (aim[0] - p[0]).atan2(aim[1] - p[1]).to_degrees();
    // The bow angle grows as the yaw falls.
    let e = wrap(want - (bow(st.yaw).to_degrees() - st.yaw_vel * LEAD));
    let (v, left) = (forward_speed(st), course.line.length() - place.s);
    if course.stop.is_some() && left < APPROACH {
        let ramp = (APPROACH_GAIN * (to_end - course.lag as f32 * v.max(0.0)).max(0.0)).min(CRUISE);
        if to_end < REVERSE_REACH && e.abs() > 90.0 && v <= STOPPED {
            // Behind and near: back onto it, the stern steered (a side key with S swings the bow the same way).
            let stern = wrap(e - 180.0);
            return if -v > ramp { NONE } else if stern.abs() < STRAIGHT { S } else { [stern.signum(), -1.0] };
        }
        return match e {
            _ if to_end < ARRIVED => if v > STOPPED { S } else { NONE },
            _ if v > ramp + RAMP_SLACK => S,
            e if e.abs() > PIVOT / 2.0 => [e.signum(), 0.0],
            _ if v >= ramp => NONE,
            e if e.abs() < STRAIGHT => W,
            e => [e.signum(), 1.0],
        };
    }
    match e {
        e if e.abs() < STRAIGHT => W,
        // S with a side key turns the same way while it brakes: a sharp turn is made slow.
        e if e.abs() > PIVOT && v > TURN_SPEED => [e.signum(), -1.0],
        e if e.abs() > PIVOT || (e.abs() > PIVOT / 2.0 && v > TURN_SPEED) => [e.signum(), 0.0],
        e => [e.signum(), 1.0],
    }
}

/// Ticks a choice of keys is held: [straight, steering] (a side key is tapped, a straight paddle held).
pub(crate) type Holds = [u32; 2];

pub(crate) fn hold_for(keys: Keys, holds: Holds) -> u32 {
    holds[usize::from(keys[0] != 0.0)].max(1)
}

/// One candidate: `prefix` ticks of `held`, then `first`, then the policy at `look`, each choice held its `holds`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Plan {
    pub held: Keys,
    pub prefix: u32,
    pub first: Option<Keys>,
    pub holds: Holds,
    pub look: f32,
}

/// A rollout's driver: `plan`'s keys, then the policy's, each choice made `lag` ticks before it is pressed, as the
/// pilot's driver does (a rollout must not steer finer than the driver can).
struct Driver {
    plan: Plan,
    follow: u32,
    lag: u32,
    keys: Keys,
    next: u32,
    due: Option<(u32, Keys)>,
}

impl Driver {
    fn new(plan: Plan, lag: u32) -> Self {
        let follow = plan.prefix + plan.first.map_or(0, |k| hold_for(k, plan.holds));
        Self { plan, follow, lag, keys: plan.held, next: follow, due: None }
    }

    fn keys<V: WorldView + ?Sized>(&mut self, t: u32, b: &BoatState, seg: &mut usize, course: &Course<'_, '_, V>) -> Keys {
        let p = self.plan;
        if t < p.prefix {
            return p.held;
        }
        if let Some(first) = p.first.filter(|_| t < self.follow) {
            self.keys = first;
            return first;
        }
        if t == self.next && self.due.is_none() {
            // A follow plan's own first choice is the candidate, pressed once the prefix is over.
            let lag = if p.first.is_none() && t == self.follow { 0 } else { self.lag };
            self.due = Some((t + lag, policy(b, seg, p.look, course)));
        }
        if let Some((_, keys)) = self.due.filter(|&(at, _)| t >= at) {
            (self.keys, self.due, self.next) = (keys, None, t + hold_for(keys, p.holds));
        }
        self.keys
    }
}

/// Simulates a plan and scores it: higher is better. Returns the keys it holds once the prefix is over.
pub(crate) fn score<V: WorldView + ?Sized>(st: &BoatState, seg: usize, plan: Plan, course: &Course<'_, '_, V>, world: &V) -> (f32, Keys) {
    let (mut b, mut seg) = (*st, seg);
    let (mut off, mut remain, mut chosen) = (0.0, 0.0, None);
    let (end, length) = (course.line.end(), course.line.length());
    // A boat already touching a bank would end every rollout at once: only pressing into it counts then.
    let contact = if course.grid.clear(st.pos[0], st.pos[2], CONTACT_PAD) { CONTACT_PAD } else { 0.0 };
    let mut driver = Driver::new(plan, course.lag);
    for t in 0..ROLLOUT_TICKS {
        let keys = driver.keys(t, &b, &mut seg, course);
        if t == plan.prefix {
            chosen = Some(keys);
        }
        let speed = b.vel[0].hypot(b.vel[2]);
        boat_tick(&mut b, keys, world);
        if !course.grid.clear(b.pos[0], b.pos[2], contact) && !(speed <= NUDGE_SPEED && dist([b.pos[0], b.pos[2]], end) < NUDGE_REACH) {
            return (-WALL + t as f32, chosen.unwrap_or(keys));
        }
        let place = course.line.locate([b.pos[0], b.pos[2]], seg, WINDOW);
        seg = place.seg;
        // On the approach only arriving counts: weighed against the line, arrivals kept slipping past the horizon.
        if course.stop.is_none() || length - place.s > APPROACH {
            off += (place.off - course.tolerance).max(0.0).powi(2);
        }
        remain += (length - place.s).max(dist([b.pos[0], b.pos[2]], end));
        if course.arrived(&b, world) {
            return (length + ARRIVAL - TIME_W * t as f32 - OFF_W * off, chosen.unwrap_or(keys));
        }
    }
    let p = [b.pos[0], b.pos[2]];
    let place = course.line.locate(p, seg, WINDOW);
    if course.stop.is_none() {
        return (place.s - OFF_W * off, chosen.unwrap_or(plan.held));
    }
    // Progress, or the distance to the stop once nearer it than the route left (an overshoot scores worse), half at
    // the end and half on average: a plan that gets there later scores worse, or waiting would always tie. Speed
    // costs only past what S can still brake before the stop, so coming in fast never scores worse than holding off.
    let (left, to_end) = (length - place.s, dist(p, end));
    let overrun = ((BRAKE + course.lag as f32) * b.vel[0].hypot(b.vel[2]) - to_end).max(0.0);
    let short = 0.5 * (to_end.max(left) + remain / ROLLOUT_TICKS as f32);
    let s = length - short - OFF_W * off - SPEED_W * overrun - TIME_W * ROLLOUT_TICKS as f32;
    (s, chosen.unwrap_or(plan.held))
}

/// The best keys to press at the end of `prefix`: the follow policies at each look-ahead, and each first key before
/// the middle one takes over.
pub(crate) fn solve<V: WorldView + ?Sized>(st: &BoatState, seg: usize, held: Keys, prefix: u32, holds: Holds, course: &Course<'_, '_, V>, world: &V) -> Keys {
    let policies = LOOKS.iter().map(|&look| Plan { held, prefix, first: None, holds, look });
    let firsts = FIRSTS.iter().map(|&k| Plan { held, prefix, first: Some(k), holds, look: LOOKS[1] });
    let mut best = (f32::MIN, W);
    for plan in policies.chain(firsts) {
        let (s, keys) = score(st, seg, plan, course, world);
        if s > best.0 {
            best = (s, keys);
        }
    }
    best.1
}
