//! The navigator's core ([`Pilot`]) driven in the headless simulation: searches run inline, answered at once or
//! after a delay, and the planner sees only chunks near the bot (a server's load horizon).

use acacia_physics::constants::DEFAULT_PLAYER_HEIGHT_OFFSET;
use acacia_physics::{BlockPos, Vec3};
use acacia_world::ChunkView;
use tokio::sync::oneshot;

use super::grid::Grid;
use super::sim::perform;
use crate::movement::Movement;
use crate::pathfind::hazard::{self, Threats, View};
use crate::pathfind::{Blocks, Goal, GotoOpts, NavStatus, Path, Pilot, SearchJob, SearchOpts, Terrain, answered};

/// The entities around a flight: moved once a tick for the bot at `feet`; returns them and the bot's health.
pub type Scene<'a> = dyn FnMut(u32, Vec3) -> (Threats, f32) + 'a;

/// The grid as the planner sees it: chunks more than `radius` from `centre` (chunk coordinates) are unloaded.
#[derive(Clone, Copy)]
pub struct Horizon<'a> {
    view: &'a ChunkView,
    centre: (i32, i32),
    radius: Option<i32>,
}

impl Blocks for Horizon<'_> {
    fn ids(&self, p: BlockPos) -> Option<(u32, u32)> {
        let far = |r: i32| (p[0] >> 4).abs_diff(self.centre.0) as i32 > r || (p[2] >> 4).abs_diff(self.centre.1) as i32 > r;
        if self.radius.is_some_and(far) {
            return None;
        }
        self.view.ids(p)
    }
}

/// How the world around the pilot behaves.
#[derive(Clone, Copy)]
pub struct Sky {
    /// Load radius in chunks around the bot; `None`: the whole grid.
    pub horizon: Option<i32>,
    /// Ticks a background search takes to answer.
    pub delay: u32,
    pub max_ticks: u32,
}

pub struct Flight {
    pub result: Result<(), String>,
    pub ticks: u32,
    /// Feet position at every tick.
    pub track: Vec<Vec3>,
    pub joins: Vec<BlockPos>,
    pub searches: u32,
    pub replans: u32,
    pub divergences: u32,
}

impl Flight {
    pub fn end(&self) -> Vec3 {
        *self.track.last().expect("ticked")
    }

    /// Ticks (and positions) moving back against the way from start to end faster than `speed` blocks/tick.
    pub fn backwards(&self, speed: f32) -> Vec<(usize, Vec3)> {
        let (a, b) = (self.track[0], self.end());
        let len = (b[0] - a[0]).hypot(b[2] - a[2]).max(1e-3);
        let (ux, uz) = ((b[0] - a[0]) / len, (b[2] - a[2]) / len);
        let along = |t: usize| (self.track[t + 1][0] - self.track[t][0]) * ux + (self.track[t + 1][2] - self.track[t][2]) * uz;
        (0..self.track.len() - 1).filter(|&t| along(t) < -speed).map(|t| (t, self.track[t])).collect()
    }

    /// Slowest horizontal speed (blocks/tick) within `window` ticks of passing each join.
    pub fn join_speeds(&self, window: usize) -> Vec<f32> {
        let speed = |t: usize| (self.track[t + 1][0] - self.track[t][0]).hypot(self.track[t + 1][2] - self.track[t][2]);
        self.joins.iter().map(|j| {
            let d = |p: &Vec3| (p[0] - j[0] as f32 - 0.5).hypot(p[2] - j[2] as f32 - 0.5);
            let at = (0..self.track.len()).min_by(|&a, &b| d(&self.track[a]).total_cmp(&d(&self.track[b]))).unwrap_or(0);
            let range = at.saturating_sub(window)..(at + window).min(self.track.len() - 1);
            range.map(speed).fold(f32::INFINITY, f32::min)
        }).collect()
    }
}

/// Drives a [`Pilot`] from `m` to `goal` until it arrives, fails or `sky.max_ticks` pass.
pub fn fly(g: &Grid, m: &mut Movement, goal: Goal, opts: GotoOpts, sky: Sky) -> Flight {
    fly_with(g, m, goal, opts, sky, &mut |_, _| (Threats::default(), 20.0))
}

/// [`fly`] among entities, perceived as the profile does; the solid ones collide.
pub fn fly_with(g: &Grid, m: &mut Movement, goal: Goal, opts: GotoOpts, sky: Sky, scene: &mut Scene) -> Flight {
    let world = g.physics();
    let mut pilot = Pilot::new(goal, opts);
    let mut late: Vec<(u32, oneshot::Sender<Path>, Path)> = Vec::new();
    let (mut track, mut result) = (Vec::new(), Err(format!("not arrived after {} ticks", sky.max_ticks)));
    let mut searching = Vec::new();
    // PILOT_TRACE=from..to: per-tick state over those ticks.
    let trace = std::env::var("PILOT_TRACE").ok().and_then(|r| r.split_once("..").and_then(|(a, b)| Some(a.parse::<u32>().ok()?..b.parse::<u32>().ok()?))).unwrap_or(0..0);
    for tick in 0..sky.max_ticks {
        let (due, waiting): (Vec<_>, Vec<_>) = late.drain(..).partition(|(t, ..)| *t <= tick);
        late = waiting;
        due.into_iter().for_each(|(_, tx, p)| _ = tx.send(p));
        let pos = m.position().expect("started");
        track.push(pos);
        let blocks = Horizon { view: &g.view, centre: (pos[0].floor() as i32 >> 4, pos[2].floor() as i32 >> 4), radius: sky.horizon };
        let (threats, health) = scene(tick, pos);
        m.set_solids(hazard::solid_boxes(&threats));
        let sees = |a, b| hazard::clear_line(&g.terrain(), a, b);
        let eye = [pos[0], pos[1] + DEFAULT_PLAYER_HEIGHT_OFFSET, pos[2]];
        let yaw = m.state().map_or(0.0, |s| s.yaw);
        let hazards = pilot.perceive(threats, &View { eye, yaw, health, sees: &sees });
        let terrain = Terrain::new(blocks, &g.registry).with_hazards(hazards.clone());
        let opts = SearchOpts { hazards, ..pilot.search_opts(None, 20.0) };
        let mut spawn = |job: SearchJob| {
            let path = job.run(&Terrain::new(blocks, &g.registry).with_hazards(job.opts.hazards.clone()));
            if sky.delay == 0 {
                return answered(path);
            }
            let (tx, rx) = oneshot::channel();
            late.push((tick + sky.delay, tx, path));
            rx
        };
        let diverged = pilot.divergences();
        let status = pilot.drive(&terrain, m, opts, &mut spawn);
        if pilot.divergences() > diverged {
            eprintln!("tick {tick}: diverged at {pos:?}");
        }
        if trace.contains(&tick) {
            let next = pilot.follower().and_then(|f| f.ahead().first().map(|n| (f.index(), n.pos, n.kind, n.entry)));
            eprintln!("TRACE {tick} {pos:.3?} vel {:.3?} {status:?} next {next:?} controls {:?}", m.state().map(|s| s.vel), m.controls);
        }
        match status {
            Ok(NavStatus::Searching) if tick > 0 => searching.push(tick),
            Ok(NavStatus::Arrived) => {
                result = Ok(());
                break;
            }
            Ok(NavStatus::Working) => {
                let node = pilot.pending_work().expect("work due");
                perform(g, &world, m, &node);
                pilot.work_done();
            }
            Ok(_) => {}
            Err(e) => {
                result = Err(e.to_string());
                break;
            }
        }
        m.tick(&world);
    }
    let ticks = track.len() as u32;
    let (joins, searches, replans, divergences) = (pilot.joins().to_vec(), pilot.searches(), pilot.replans(), pilot.divergences());
    eprintln!("FLIGHT {result:?} after {ticks} ticks at {:?}: {searches} searches, {replans} re-plans, {divergences} diverged, joins {joins:?}", track.last());
    eprintln!("searching after the start at ticks {searching:?}");
    Flight { result, ticks, track, joins, searches, replans, divergences }
}
