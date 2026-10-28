//! Striking back at a melee hostile that chases the bot into reach (design decision 5'): only one the profile
//! perceives (the field's `melee`), only while it closes in despite the route, within vanilla reach and with a
//! clear line, aimed for a tick before the click (the rotation reaches the server first, as en-route placing does).
//! The keys keep the course meanwhile.

use acacia_physics::Vec3;

use super::{Hazards, Threat};
use crate::cadence::splitmix;
use crate::human;
use crate::interact::{ray_box, ENTITY_REACH};
use crate::movement::Controls;
use crate::pathfind::MovementProfile;

/// Ticks a struck mob ignores further hits (`Optimal` clicks once each).
pub const INVULNERABLE_TICKS: u64 = 10;
/// Blocks beyond reach a chaser is aimed at already, so the click lands as it comes into reach.
const PRE_AIM: f32 = 2.0;
/// Speed (blocks/tick) towards the bot above which a mob counts as chasing it (a stroll is 0.11, a chase 0.26).
const CHASE_APPROACH: f32 = 0.18;
/// Speed (blocks/tick) the gap must shrink by for the hit to be coming.
const CLOSING: f32 = 0.02;
/// Centre distance (blocks) within which a chaser keeping pace is struck too: live zombies hit from 2.3-2.5.
const DANGER: f32 = 3.0;
/// Heights (blocks) between feet beyond which a mob cannot hit.
const MELEE_HEIGHT: f32 = 2.0;

/// The bot as the striker sees it this tick.
pub struct Body<'a> {
    pub eye: Vec3,
    pub feet: Vec3,
    /// Blocks per tick.
    pub vel: Vec3,
    /// The bot's unique id (what `TargetEid` names).
    pub me: i64,
    pub tick: u64,
    pub sees: &'a dyn Fn(Vec3, Vec3) -> bool,
}

#[derive(Debug)]
pub struct Striker {
    profile: MovementProfile,
    rng: u64,
    /// Runtime id the crosshair was on at the end of last tick.
    aimed: Option<u64>,
    /// The chaser struck at, from the tick it was picked.
    engaged: Option<u64>,
    /// (yaw, pitch) looked at last tick while striking.
    look: Option<(f32, f32)>,
    next_at: u64,
    strikes: u32,
}

impl Striker {
    pub fn new(profile: MovementProfile, seed: u64) -> Self {
        Striker { profile, rng: seed, aimed: None, engaged: None, look: None, next_at: 0, strikes: 0 }
    }

    /// Clicks sent so far.
    pub fn strikes(&self) -> u32 {
        self.strikes
    }

    /// After the steering: turns `controls` to a chaser closing in (keys kept on the course) and returns the
    /// runtime id to attack now, if the crosshair has been on it since last tick and it is in reach.
    pub fn tick(&mut self, hazards: Option<&Hazards>, body: &Body, controls: &mut Controls) -> Option<u64> {
        let Some(t) = hazards.and_then(|h| chaser(&h.melee, body)) else {
            (self.aimed, self.engaged, self.look) = (None, None, None);
            return None;
        };
        let course = *controls;
        let (min, max) = t.bounds();
        let mid = [0, 1, 2].map(|i| (min[i] + max[i]) * 0.5);
        let mut want = course;
        want.look_at(body.eye, mid);
        // A chaser on the course is struck with the pitch alone: the keys and sprint stay as steered.
        let on_course = crosshair(body.eye, &Controls { pitch: want.pitch, ..course }, min, max).is_some();
        if on_course {
            want.yaw = course.yaw;
        }
        if self.profile == MovementProfile::Optimal {
            (controls.yaw, controls.pitch) = (want.yaw, want.pitch);
        } else {
            // Eased from the look sent last tick: the steering set the course's yaw since.
            let (yaw, pitch) = self.look.unwrap_or((course.yaw, course.pitch));
            controls.yaw = if on_course { course.yaw } else { self.ease(yaw, want.yaw) };
            controls.pitch = self.ease(pitch, want.pitch).clamp(-90.0, 90.0);
        }
        self.look = Some((controls.yaw, controls.pitch));
        if !on_course {
            controls.keep_course(&course);
        }
        let hit = crosshair(body.eye, controls, min, max).filter(|h| (body.sees)(body.eye, *h));
        let ready = self.aimed == Some(t.runtime_id) && body.tick >= self.next_at;
        self.aimed = hit.map(|_| t.runtime_id);
        if self.engaged.replace(t.runtime_id) != Some(t.runtime_id) && self.profile == MovementProfile::Human {
            self.next_at = self.next_at.max(body.tick + self.between(human::STRIKE_REACT));
        }
        if !ready || hit.is_none_or(|h| dist(body.eye, h) > ENTITY_REACH) {
            return None;
        }
        let gap = if self.profile == MovementProfile::Optimal { INVULNERABLE_TICKS } else { self.between(human::STRIKE_GAP) };
        self.next_at = body.tick + gap;
        self.strikes += 1;
        Some(t.runtime_id)
    }

    fn ease(&mut self, from: f32, to: f32) -> f32 {
        let diff = (to - from + 180.0).rem_euclid(360.0) - 180.0;
        let step = (diff * human::AIM_EASE).clamp(-human::AIM_MAX_TURN_DEG, human::AIM_MAX_TURN_DEG);
        let unit = (splitmix(&mut self.rng) >> 40) as f32 / (1u64 << 24) as f32;
        from + step + (unit * 2.0 - 1.0) * human::AIM_JITTER_DEG * 0.5
    }

    fn between(&mut self, (lo, hi): (u64, u64)) -> u64 {
        lo + splitmix(&mut self.rng) % (hi - lo + 1)
    }
}

fn dist(a: Vec3, b: Vec3) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}

/// The nearest melee hostile that chases the bot and closes in on it (or keeps up within [`DANGER`]), within reach
/// and the pre-aim margin.
fn chaser<'t>(melee: &'t [Threat], body: &Body) -> Option<&'t Threat> {
    let threat = |t: &&Threat| {
        let (dx, dz) = (body.feet[0] - t.feet[0], body.feet[2] - t.feet[2]);
        let flat = dx.hypot(dz).max(1e-4);
        let toward = (t.motion[0] * dx + t.motion[2] * dz) / flat;
        let closing = toward - (body.vel[0] * dx + body.vel[2] * dz) / flat;
        let chasing = t.target == Some(body.me) || toward >= CHASE_APPROACH;
        let coming = closing > CLOSING || flat <= DANGER;
        chasing && coming && (t.feet[1] - body.feet[1]).abs() < MELEE_HEIGHT && reach(t, body.eye) <= ENTITY_REACH + PRE_AIM
    };
    melee.iter().filter(threat).min_by(|a, b| reach(a, body.eye).total_cmp(&reach(b, body.eye)))
}

/// Distance from `eye` to the nearest point of `t`'s box.
fn reach(t: &Threat, eye: Vec3) -> f32 {
    let (min, max) = t.bounds();
    dist(eye, [0, 1, 2].map(|i| eye[i].clamp(min[i], max[i])))
}

/// Where the look ray of `c` from `eye` enters the box, if it does.
fn crosshair(eye: Vec3, c: &Controls, min: Vec3, max: Vec3) -> Option<Vec3> {
    let (yaw, pitch) = (c.yaw.to_radians(), c.pitch.to_radians());
    let dir = [-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()];
    let (t, _) = ray_box(eye, dir, min, max)?;
    (t >= 0.0).then(|| [0, 1, 2].map(|i| eye[i] + dir[i] * t))
}
