//! One flight, tick by tick, network-free: sets the controls and says which action the bot takes this tick.

use acacia_physics::{BlockPos, Vec3, WorldView};

use super::solve::{Line, glide_reach, solve};
use super::surface::{Surface, corridor_top, known_ahead};
use super::FlyOpts;
use crate::cadence::splitmix;
use crate::movement::{Controls, Rollout};
use crate::pathfind::profile::{Aim, MovementProfile};

/// What the bot does this tick besides holding the controls.
#[derive(Debug, Clone, PartialEq)]
pub enum Order {
    Fly,
    /// Open the elytra (a jump press in the air).
    Open,
    /// Use a firework rocket.
    Rocket,
    /// On the ground with the glide ended.
    Landed,
    Fail(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Ground,
    Rising,
    Opening,
    Gliding,
    Landed,
}

/// Horizontal blocks out from the spot where the touchdown is scored instead of the line.
const LANDING_DIST: f32 = 56.0;
/// Height lost per block coming in: a glide's own sink at cruise speed, so the approach needs no stall to get down.
const SLOPE: f32 = 0.25;
/// Blocks of corridor ahead the cruise height is taken over.
const CORRIDOR: f32 = 160.0;
/// Below this speed (blocks/tick) a cruise wants a rocket.
const CRUISE_SPEED: f32 = 1.2;
/// Ticks of flight at the present speed that must be known ahead before a rocket speeds it up.
const HORIZON_TICKS: f32 = 40.0;
/// Attempts at opening the elytra before giving up.
const MAX_OPENS: u32 = 4;
/// Boost ticks a rocket is planned with: the shortest a flight-1 rocket gives (`movement/glide.rs`).
const ROCKET_TICKS: i64 = 20;
/// Human rocket delay, ticks after the want arises.
const HUMAN_ROCKET_DELAY: (u64, u64) = (3, 14);
/// Opened at a jump's top, a glide has about 6 ticks before the ground: the takeoff rocket comes within 3.
const HUMAN_TAKEOFF_DELAY: (u64, u64) = (1, 3);
/// Feet this close over the terrain count as taking off.
const TAKEOFF_HEIGHT: f32 = 4.0;

pub struct FlightPilot {
    target: BlockPos,
    spot: Option<Vec3>,
    opts: FlyOpts,
    phase: Phase,
    yaw_aim: Aim,
    pitch_aim: Aim,
    rng: u64,
    tick: u64,
    rocket_at: Option<u64>,
    cruise_y: f32,
    pitch: f32,
    opens: u32,
    reach_checked: bool,
}

impl FlightPilot {
    pub fn new(target: BlockPos, opts: FlyOpts, seed: u64) -> Self {
        let mut rng = seed;
        let (yaw_aim, pitch_aim) = (Aim::new(opts.profile, splitmix(&mut rng)), Aim::new(opts.profile, splitmix(&mut rng)));
        Self { target, spot: None, opts, phase: Phase::Ground, yaw_aim, pitch_aim, rng, tick: 0, rocket_at: None, cruise_y: f32::MIN, pitch: 0.0, opens: 0, reach_checked: false }
    }

    /// Feet of the landing spot, once one is found.
    pub fn spot(&self) -> Option<Vec3> {
        self.spot
    }

    /// Sets `c` for the tick after `r` (the live simulation forked) with `rockets` in the hotbar.
    pub fn tick(&mut self, r: &Rollout, w: &(impl WorldView + ?Sized), s: &mut dyn Surface, rockets: u32, c: &mut Controls) -> Order {
        self.tick += 1;
        if self.tick > self.opts.max_ticks {
            return Order::Fail(format!("no landing after {} ticks", self.opts.max_ticks));
        }
        let st = &r.st;
        let yaw = self.yaw_aim.turn(st.yaw, self.course_yaw(st.pos));
        match self.phase {
            Phase::Ground if st.gliding => self.phase = Phase::Gliding,
            // Starting in the air (off a cliff, a tower): open on the way down.
            Phase::Ground if !st.on_ground => {
                self.phase = Phase::Rising;
                c.yaw = yaw;
                return Order::Fly;
            }
            Phase::Ground => {
                if rockets == 0 {
                    return Order::Fail("on the ground with no rockets to take off with".into());
                }
                *c = Controls { forward: 1.0, jump: true, yaw, pitch: self.pitch_aim.turn(st.pitch, -15.0), ..Controls::default() };
                self.phase = Phase::Rising;
                return Order::Fly;
            }
            Phase::Rising => {
                c.jump = false;
                c.yaw = yaw;
                if st.on_ground && st.vel[1] <= 0.0 {
                    self.phase = Phase::Ground;
                } else if !st.on_ground && st.vel[1] < 0.1 {
                    self.phase = Phase::Opening;
                    return Order::Open;
                }
                return Order::Fly;
            }
            Phase::Opening if st.gliding => self.phase = Phase::Gliding,
            Phase::Opening => {
                self.opens += 1;
                if self.opens > MAX_OPENS {
                    return Order::Fail("the elytra did not open".into());
                }
                if st.on_ground {
                    self.phase = Phase::Ground;
                    return Order::Fly;
                }
                return Order::Open;
            }
            Phase::Gliding | Phase::Landed => {}
        }
        if !st.gliding {
            if st.on_ground {
                self.phase = Phase::Landed;
                c.stop();
                return Order::Landed;
            }
            self.phase = Phase::Opening;
            return Order::Open;
        }
        self.glide(r, w, s, rockets, yaw, c)
    }

    fn glide(&mut self, r: &Rollout, w: &(impl WorldView + ?Sized), s: &mut dyn Surface, rockets: u32, yaw: f32, c: &mut Controls) -> Order {
        let st = &r.st;
        let pos = st.pos;
        // Re-picked until the landing starts: a block can change under it, and blocks around the target arrive late.
        let near = self.spot.is_some_and(|p| (p[0] - pos[0]).hypot(p[2] - pos[2]) < LANDING_DIST);
        if !near {
            self.spot = find_spot(s, self.target, self.opts.land_radius);
        }
        let target = self.spot.unwrap_or([self.target[0] as f32 + 0.5, self.target[1] as f32, self.target[2] as f32 + 0.5]);
        let d = (target[0] - pos[0]).hypot(target[2] - pos[2]);
        let dir = if d > 0.0 { [(target[0] - pos[0]) / d, (target[2] - pos[2]) / d] } else { [0.0, 0.0] };
        let ahead = [pos[0] + dir[0] * d.min(CORRIDOR), pos[2] + dir[1] * d.min(CORRIDOR)];
        let (top, _) = corridor_top(s, [pos[0], pos[2]], ahead, 3.0);
        // Never lowered while far out: terrain behind the known edge may be as high as what was crossed.
        self.cruise_y = self.cruise_y.max((top + self.opts.cruise_margin) as f32).max(target[1] + self.opts.cruise_margin as f32);
        if rockets == 0 && !self.reach_checked {
            self.reach_checked = true;
            if glide_reach(r, yaw, target[1], w) + self.opts.land_radius as f32 * 2.0 < d {
                return Order::Fail(format!("no rockets, and a glide from y {:.0} does not reach {d:.0} blocks", pos[1]));
            }
        }
        let landing = self.spot.is_some() && d < LANDING_DIST;
        let line = Line { target, cruise_y: self.cruise_y, slope: SLOPE, margin: self.opts.cruise_margin as f32 / 2.0, landing };
        let speed = st.vel.iter().map(|v| v * v).sum::<f32>().sqrt();
        let known = known_ahead(s, [pos[0], pos[2]], dir, (speed * HORIZON_TICKS).max(48.0));
        let horizon = known >= (speed * HORIZON_TICKS).max(48.0) || known >= d;
        let low = pos[1] < line.height_at(d) - 4.0;
        let want = !landing && st.glide_boost_ticks == 0 && horizon && (low || speed < CRUISE_SPEED) && d > LANDING_DIST;
        // Short of the spot's height near it, a reserve rocket saves the landing.
        let save = st.glide_boost_ticks == 0 && landing && pos[1] < target[1] + 1.0 && d > 8.0;
        // Just off the ground (the takeoff) a rocket must come quickly; one into the ground is not boosted by the server.
        let low_down = pos[1] - (corridor_top(s, [pos[0], pos[2]], [pos[0], pos[2]], 0.0).0 + 1) as f32;
        let takeoff = low_down < TAKEOFF_HEIGHT;
        let touching = low_down < 0.5 && st.vel[1] < 0.0;
        let wanted = !touching && ((want && rockets > self.opts.rocket_reserve) || (save && rockets > 0));
        let order = if wanted { self.rocket_when_ready(takeoff) } else { Order::Fly };
        if !wanted {
            self.rocket_at = None;
        }
        // The pitch is solved with this tick's rocket (or one about to come) already pushing.
        let boosted;
        let from = if order == Order::Rocket || self.rocket_at.is_some() {
            let mut b = r.clone();
            b.st.glide_boost_ticks = ROCKET_TICKS;
            boosted = b;
            &boosted
        } else {
            r
        };
        self.pitch = solve(from, yaw, self.pitch, &line, w, s);
        tracing::trace!(tick = self.tick, ?pos, d, known, cruise_y = self.cruise_y, spot = ?self.spot, pitch = self.pitch, rockets, ?order, "glide");
        *c = Controls { forward: 1.0, glide: true, yaw, pitch: self.pitch_aim.turn(st.pitch, self.pitch).clamp(-90.0, 90.0), ..Controls::default() };
        order
    }

    fn rocket_when_ready(&mut self, takeoff: bool) -> Order {
        if self.opts.profile == MovementProfile::Optimal {
            return Order::Rocket;
        }
        let (lo, hi) = if takeoff { HUMAN_TAKEOFF_DELAY } else { HUMAN_ROCKET_DELAY };
        let at = *self.rocket_at.get_or_insert(self.tick + lo + splitmix(&mut self.rng) % (hi - lo));
        if self.tick < at {
            return Order::Fly;
        }
        self.rocket_at = None;
        Order::Rocket
    }

    fn course_yaw(&self, pos: Vec3) -> f32 {
        let to = self.spot.map_or([self.target[0] as f32 + 0.5, self.target[2] as f32 + 0.5], |s| [s[0], s[2]]);
        (pos[0] - to[0]).atan2(to[1] - pos[2]).to_degrees()
    }
}

/// The landable column nearest `target` within `radius`, feet on its top block; `None` until the target's own column
/// is known (before, only the near edge of the radius may be).
fn find_spot(s: &mut dyn Surface, target: BlockPos, radius: i32) -> Option<Vec3> {
    s.top(target[0], target[2])?;
    let mut best: Option<(i32, Vec3)> = None;
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            let (x, z) = (target[0] + dx, target[2] + dz);
            let d2 = dx * dx + dz * dz;
            if d2 > radius * radius || best.is_some_and(|(b, _)| b <= d2) || !s.landable(x, z) {
                continue;
            }
            let Some(top) = s.top(x, z) else { continue };
            best = Some((d2, [x as f32 + 0.5, (top + 1) as f32, z as f32 + 0.5]));
        }
    }
    best.map(|(_, p)| p)
}
