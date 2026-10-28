//! Entities around a headless flight, with a damage model: melee reach and cooldown, a creeper's fuse and blast
//! radius, skeleton arrows flown straight at where the bot stood, and the knockback of the bot's strikes. Coarse on
//! purpose: it judges whether the planner keeps clear, not vanilla mob AI; chasers are calibrated to live BDS.

use acacia_physics::Vec3;

use super::grid::Grid;
use super::pilot_sim::SIM_ME;
use crate::pathfind::hazard::{self, Class, Threat, Threats, INVULNERABLE_TICKS};

const FOLLOW: f32 = 16.0;
/// Centre distance a melee mob hits from: live BDS zombies hit from 2.3-2.5 as tracked (2026-10-28).
const MELEE_REACH: f32 = 2.3;
const MELEE_COOLDOWN: u32 = 20;
/// A live BDS zombie chasing its target (2026-10-28); kinds without a measurement chase at their table speed.
const LIVE_ZOMBIE_CHASE: f32 = 0.264;
/// Java's mob knockback: horizontal push and lift (blocks/tick), and the drag airborne and on the ground.
const KNOCKBACK: f32 = 0.4;
const GRAVITY: f32 = 0.08;
const AIR_DRAG: f32 = 0.91;
const GROUND_DRAG: f32 = 0.546;
const FUSE_START: f32 = 3.0;
const FUSE_STOP: f32 = 7.0;
const FUSE_TICKS: u32 = 30;
const BLAST: f32 = 6.0;
const SHOT_EVERY: u32 = 40;
const ARROW_SPEED: f32 = 1.6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Brain {
    Still,
    Chase,
    /// Walks back and forth between two points.
    Pace(Vec3, Vec3),
}

#[derive(Debug, Clone)]
pub struct Mob {
    pub kind: &'static str,
    pub pos: Vec3,
    pub brain: Brain,
    motion: Vec3,
    /// Knockback velocity, while it lasts.
    push: Vec3,
    ground: f32,
    hurt: u64,
    cooldown: u32,
    fuse: Option<u32>,
    gone: bool,
    toward_b: bool,
}

impl Mob {
    pub fn new(kind: &'static str, pos: Vec3, brain: Brain) -> Mob {
        Mob { kind, pos, brain, motion: [0.0; 3], push: [0.0; 3], ground: pos[1], hurt: 0, cooldown: 0, fuse: None, gone: false, toward_b: true }
    }

    /// A creeper whose fuse is already burning (a forced ignition).
    pub fn lit(self) -> Mob {
        Mob { fuse: Some(0), ..self }
    }

    fn kind(&self) -> hazard::Kind {
        hazard::kind(self.kind).expect("known kind")
    }

    fn chase_speed(&self) -> f32 {
        if self.kind == "zombie" { LIVE_ZOMBIE_CHASE } else { self.kind().speed }
    }

    fn airborne(&self) -> bool {
        self.pos[1] > self.ground + 1e-3 || self.push[1] > 0.0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Harm {
    pub melee: u32,
    pub blast: u32,
    pub arrows: u32,
    /// Strikes that landed (outside the mob's invulnerability).
    pub struck: u32,
}

struct Arrow {
    pos: Vec3,
    vel: Vec3,
    life: u32,
}

pub struct Crowd<'g> {
    pub mobs: Vec<Mob>,
    pub harm: Harm,
    grid: &'g Grid,
    arrows: Vec<Arrow>,
}

fn flat_dist(a: Vec3, b: Vec3) -> f32 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}

impl<'g> Crowd<'g> {
    pub fn new(grid: &'g Grid, mobs: Vec<Mob>) -> Self {
        Crowd { mobs, harm: Harm::default(), grid, arrows: Vec::new() }
    }

    fn open(&self, p: Vec3) -> bool {
        let t = self.grid.terrain();
        let cell = |dy: i32| [p[0].floor() as i32, p[1].floor() as i32 + dy, p[2].floor() as i32];
        t.cell(cell(0)).boxes.is_empty() && t.cell(cell(1)).boxes.is_empty()
    }

    fn sees(&self, a: Vec3, b: Vec3) -> bool {
        hazard::clear_line(&self.grid.terrain(), a, b)
    }

    /// Moves `m` by `(dx, dz)` where the way is open.
    fn shift(&self, m: &mut Mob, dx: f32, dz: f32) {
        if self.open([m.pos[0] + dx, m.pos[1], m.pos[2]]) {
            m.pos[0] += dx;
        }
        if self.open([m.pos[0], m.pos[1], m.pos[2] + dz]) {
            m.pos[2] += dz;
        }
    }

    fn walk(&self, m: &mut Mob, to: Vec3, speed: f32) {
        let d = flat_dist(m.pos, to);
        let step = speed.min(d);
        let (dx, dz) = if d > 1e-4 { ((to[0] - m.pos[0]) / d * step, (to[2] - m.pos[2]) / d * step) } else { (0.0, 0.0) };
        self.shift(m, dx, dz);
    }

    /// The bot at `from` struck mob `i`: knocked back unless still invulnerable.
    fn struck(&mut self, i: usize, from: Vec3, tick: u64) {
        let Some(m) = self.mobs.get_mut(i).filter(|m| !m.gone && tick >= m.hurt) else { return };
        let (dx, dz) = (m.pos[0] - from[0], m.pos[2] - from[2]);
        let d = dx.hypot(dz).max(1e-4);
        m.push = [m.push[0] / 2.0 + dx / d * KNOCKBACK, KNOCKBACK.min(m.push[1] / 2.0 + KNOCKBACK), m.push[2] / 2.0 + dz / d * KNOCKBACK];
        m.hurt = tick + INVULNERABLE_TICKS;
        self.harm.struck += 1;
    }

    /// Knockback for a tick; true while it keeps the mob from walking.
    fn fly(&self, m: &mut Mob) -> bool {
        if m.push == [0.0; 3] {
            return false;
        }
        let air = m.airborne();
        self.shift(m, m.push[0], m.push[2]);
        m.pos[1] = (m.pos[1] + m.push[1]).max(m.ground);
        m.push[1] = if m.pos[1] <= m.ground { 0.0 } else { (m.push[1] - GRAVITY) * 0.98 };
        let drag = if air { AIR_DRAG } else { GROUND_DRAG };
        (m.push[0], m.push[2]) = (m.push[0] * drag, m.push[2] * drag);
        if m.push[0].hypot(m.push[2]) < 0.003 && !m.airborne() {
            m.push = [0.0; 3];
        }
        air
    }

    fn think(&mut self, i: usize, tick: u32, feet: Vec3) {
        let mut m = self.mobs[i].clone();
        let kind = m.kind();
        let before = m.pos;
        m.cooldown = m.cooldown.saturating_sub(1);
        let flying = self.fly(&mut m);
        let d = flat_dist(m.pos, feet);
        match (kind.class, m.brain) {
            (Class::Creeper, _) if m.fuse.is_some() => {
                m.fuse = (d <= FUSE_STOP).then(|| m.fuse.unwrap_or(0) + 1);
                if m.fuse.is_some_and(|f| f >= FUSE_TICKS) {
                    m.gone = true;
                    self.harm.blast += u32::from(d < BLAST);
                }
            }
            (Class::Creeper, Brain::Chase) if d < FUSE_START => m.fuse = Some(0),
            (Class::Ranged, _) if d <= FOLLOW && tick % SHOT_EVERY == 0 && self.sees(eye(m.pos), chest(feet)) => {
                let (from, to) = (eye(m.pos), chest(feet));
                let len = (0..3).map(|i| (to[i] - from[i]).powi(2)).sum::<f32>().sqrt().max(1e-3);
                self.arrows.push(Arrow { pos: from, vel: [0, 1, 2].map(|k| (to[k] - from[k]) / len * ARROW_SPEED), life: 40 });
            }
            _ if flying => {}
            (_, Brain::Chase) if d <= FOLLOW && d > MELEE_REACH * 0.8 => {
                let speed = m.chase_speed();
                self.walk(&mut m, feet, speed);
            }
            (_, Brain::Pace(a, b)) => {
                let to = if m.toward_b { b } else { a };
                self.walk(&mut m, to, kind.speed);
                m.toward_b ^= flat_dist(m.pos, to) < 0.05;
            }
            _ => {}
        }
        m.motion = [0, 1, 2].map(|k| m.pos[k] - before[k]);
        let hits = matches!(kind.class, Class::Melee) || kind.class == Class::Neutral;
        if hits && m.cooldown == 0 && d <= MELEE_REACH && (m.pos[1] - feet[1]).abs() < 1.5 {
            self.harm.melee += 1;
            m.cooldown = MELEE_COOLDOWN;
        }
        self.mobs[i] = m;
    }

    fn fly_arrows(&mut self, feet: Vec3) {
        let grid_hit = |p: Vec3| !self.grid.terrain().cell(p.map(|v| v.floor() as i32)).boxes.is_empty();
        let mut hits = 0;
        self.arrows.retain_mut(|a| {
            for _ in 0..4 {
                a.pos = [0, 1, 2].map(|k| a.pos[k] + a.vel[k] * 0.25);
                let inside = (a.pos[0] - feet[0]).abs() < 0.425 && (a.pos[2] - feet[2]).abs() < 0.425 && (feet[1]..feet[1] + 1.8).contains(&a.pos[1]);
                if inside {
                    hits += 1;
                    return false;
                }
                if grid_hit(a.pos) {
                    return false;
                }
            }
            a.life -= 1;
            a.life > 0
        });
        self.harm.arrows += hits;
    }

    /// The crowd as the bot's tracker sees it at `tick`, the bot at `feet`.
    pub fn threats(&self, tick: u32, feet: Vec3) -> Threats {
        let list = self.mobs.iter().enumerate().filter(|(_, m)| !m.gone).map(|(i, m)| {
            let k = m.kind();
            let target = (m.brain == Brain::Chase && flat_dist(m.pos, feet) <= FOLLOW).then_some(SIM_ME);
            Threat { runtime_id: i as u64, class: k.class, feet: m.pos, size: k.size, motion: m.motion, speed: k.speed, age: 0, lit: m.fuse.is_some(), angry: false, target }
        });
        Threats { list: list.collect(), at: u64::from(tick) }
    }

    /// The bot's health after the harm so far (vanilla-ish: 3 a hit or arrow, 10 a blast).
    pub fn health(&self) -> f32 {
        (20.0 - 3.0 * (self.harm.melee + self.harm.arrows) as f32 - 10.0 * self.harm.blast as f32).max(1.0)
    }

    /// One tick: the bot's strike lands, mobs move and strike, arrows fly; returns the crowd and the bot's health.
    pub fn step(&mut self, tick: u32, feet: Vec3, struck: Option<u64>) -> (Threats, f32) {
        if let Some(i) = struck {
            self.struck(i as usize, feet, u64::from(tick));
        }
        for i in 0..self.mobs.len() {
            if !self.mobs[i].gone {
                self.think(i, tick, feet);
            }
        }
        self.fly_arrows(feet);
        // CROWD_TRACE=1: the bot, each mob and the harm every tick.
        if std::env::var_os("CROWD_TRACE").is_some() {
            let mobs: Vec<_> = self.mobs.iter().map(|m| (m.pos, flat_dist(m.pos, feet), m.push)).collect();
            eprintln!("CROWD {tick} bot {feet:.2?} struck {struck:?} mobs {mobs:.2?} {:?}", self.harm);
        }
        (self.threats(tick, feet), self.health())
    }
}

fn eye(p: Vec3) -> Vec3 {
    [p[0], p[1] + 1.5, p[2]]
}

fn chest(p: Vec3) -> Vec3 {
    [p[0], p[1] + 1.0, p[2]]
}
