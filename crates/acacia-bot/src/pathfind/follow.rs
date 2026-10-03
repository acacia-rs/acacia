//! Per-tick path following: turns a path into [`Controls`], network-free so it can run against the
//! physics simulation headlessly.

use acacia_physics::Vec3;

use super::moves::MoveKind;
use super::PathNode;
use crate::movement::Controls;

/// No progress towards the current node for this long means stuck (1 s).
const STUCK_TICKS: u32 = 20;
const MIN_PROGRESS: f32 = 0.05;
/// Farther than this from the current segment (horizontally) means knocked off the path.
const OFF_PATH: f32 = 1.5;
/// Ground drag per tick (block friction 0.6 × 0.91): coasting from speed `v` covers `v·k/(1-k)`.
const GROUND_DRAG: f32 = 0.546;
/// Straight nodes ahead before sprinting pays off.
const SPRINT_RUN: usize = 3;

/// What the follower knows about the player this tick.
#[derive(Debug, Clone, Copy)]
pub struct Sense {
    pub pos: Vec3,
    pub on_ground: bool,
    pub in_water: bool,
}

/// Within this of the segment start (horizontally), the bot does a move's work from where it is.
const WORK_RADIUS: f32 = 0.5;
/// Horizontal speed (blocks per tick) below which the bot counts as standing for work.
const SETTLED: f32 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowStatus {
    Moving,
    Arrived,
    Stuck,
    OffPath,
    /// Standing at the start of a move whose [`Work`](super::Work) must be done first: do
    /// [`Follower::pending_work`], then call [`Follower::work_done`].
    Work,
}

pub struct Follower {
    path: Vec<PathNode>,
    index: usize,
    /// The current node's work is done.
    worked: bool,
    /// Start of the current segment: the previous node's centre, or where the bot started.
    origin: Vec3,
    last_pos: Option<Vec3>,
    best_dist: f32,
    since_progress: u32,
    tolerance: f32,
}

fn center(n: &PathNode) -> Vec3 {
    [n.pos[0] as f32 + 0.5, n.feet, n.pos[2] as f32 + 0.5]
}

fn hdist(a: Vec3, b: Vec3) -> f32 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}

fn dir(a: Vec3, b: Vec3) -> (i32, i32) {
    ((b[0] - a[0]).round() as i32, (b[2] - a[2]).round() as i32)
}

impl Follower {
    /// `tolerance`: how close (horizontally, blocks) to the final node's centre counts as arrived.
    pub fn new(path: Vec<PathNode>, start: Vec3, tolerance: f32) -> Self {
        Self { path, index: 0, worked: false, origin: start, last_pos: None, best_dist: f32::INFINITY, since_progress: 0, tolerance }
    }

    /// The node whose work is due before moving on, if any.
    pub fn pending_work(&self) -> Option<&PathNode> {
        self.has_pending(self.index).then(|| &self.path[self.index])
    }

    pub fn work_done(&mut self) {
        self.worked = true;
        (self.best_dist, self.since_progress) = (f32::INFINITY, 0);
    }

    fn has_pending(&self, i: usize) -> bool {
        self.path.get(i).is_some_and(|n| !n.work.is_empty()) && !(i == self.index && self.worked)
    }

    /// The nodes ahead whose footing exists already: up to the first one waiting for work.
    pub fn standing_ahead(&self) -> &[PathNode] {
        let end = (self.index..self.path.len()).find(|&i| self.has_pending(i)).unwrap_or(self.path.len());
        &self.path[self.index..end]
    }

    pub fn path(&self) -> &[PathNode] {
        &self.path
    }

    /// Index of the node being walked to.
    pub fn index(&self) -> usize {
        self.index
    }

    pub fn remaining(&self) -> usize {
        self.path.len() - self.index
    }

    /// The nodes still ahead, the current target first.
    pub fn ahead(&self) -> &[PathNode] {
        &self.path[self.index..]
    }

    pub fn tick(&mut self, s: &Sense, c: &mut Controls) -> FollowStatus {
        let vel = self.last_pos.map_or([0.0; 3], |l| [s.pos[0] - l[0], s.pos[1] - l[1], s.pos[2] - l[2]]);
        self.last_pos = Some(s.pos);
        c.pitch = 0.0;
        if self.path.is_empty() {
            c.stop();
            return FollowStatus::Arrived;
        }
        self.advance(s);
        let node = self.path[self.index];
        let target = center(&node);
        if self.off_path(s.pos, target) {
            c.stop();
            return FollowStatus::OffPath;
        }
        let last = self.index + 1 == self.path.len();
        let dist = hdist(s.pos, target);
        let speed = vel[0].hypot(vel[2]);
        if self.has_pending(self.index) {
            return self.approach_work(s, c, speed);
        }
        if last && dist < self.tolerance && speed < 0.03 && self.reached(&node, s, self.tolerance) {
            c.stop();
            return FollowStatus::Arrived;
        }
        let dist3 = dist.hypot(target[1] - s.pos[1]);
        if dist3 < self.best_dist - MIN_PROGRESS {
            (self.best_dist, self.since_progress) = (dist3, 0);
        } else {
            self.since_progress += 1;
            if self.since_progress > STUCK_TICKS {
                c.stop();
                return FollowStatus::Stuck;
            }
        }
        self.steer(s, c, &node, target, dist, speed, last);
        FollowStatus::Moving
    }

    /// Work is done from the segment start: walk back there if needed and come to a stop.
    fn approach_work(&mut self, s: &Sense, c: &mut Controls, speed: f32) -> FollowStatus {
        let d = hdist(s.pos, self.origin);
        c.stop();
        c.sneak = false;
        if d < WORK_RADIUS && s.on_ground {
            return if speed < SETTLED { FollowStatus::Work } else { FollowStatus::Moving };
        }
        self.since_progress += 1;
        if self.since_progress > STUCK_TICKS {
            return FollowStatus::Stuck;
        }
        c.yaw = (s.pos[0] - self.origin[0]).atan2(self.origin[2] - s.pos[2]).to_degrees();
        c.forward = 1.0;
        FollowStatus::Moving
    }

    #[allow(clippy::too_many_arguments)]
    fn steer(&self, s: &Sense, c: &mut Controls, node: &PathNode, target: Vec3, dist: f32, speed: f32, last: bool) {
        let (dx, dz) = (target[0] - s.pos[0], target[2] - s.pos[2]);
        if dist > 0.05 {
            c.yaw = (-dx).atan2(dz).to_degrees();
        }
        c.strafe = 0.0;
        c.sneak = false;
        c.forward = if last {
            // Bedrock ground friction lets the bot coast: release the key once coasting covers the rest.
            let coast = speed * GROUND_DRAG / (1.0 - GROUND_DRAG);
            if dist < coast + 0.05 { 0.0 } else { 1.0 }
        } else if matches!(node.kind, MoveKind::ClimbUp | MoveKind::ClimbDown | MoveKind::Pillar | MoveKind::Down) && dist < 0.15 {
            0.0
        } else {
            1.0
        };
        let rise = node.feet - s.pos[1];
        c.jump = match node.kind {
            MoveKind::Ascend => rise > 0.3 && dist < 1.3,
            MoveKind::ClimbUp => true,
            MoveKind::Parkour => s.on_ground && self.along(s.pos) > 0.35,
            _ if s.in_water => rise > -0.3,
            _ => false,
        };
        let wanted = !s.in_water
            && match node.kind {
                MoveKind::Parkour => true,
                MoveKind::Walk => self.straight_run() >= SPRINT_RUN,
                _ => false,
            };
        // The sprint key only starts a sprint; Movement keeps it going until forward is released.
        c.sprint = wanted;
    }

    /// Distance travelled from the segment origin along the segment direction.
    fn along(&self, pos: Vec3) -> f32 {
        let t = center(&self.path[self.index]);
        let (sx, sz) = (t[0] - self.origin[0], t[2] - self.origin[2]);
        let len = sx.hypot(sz).max(1e-3);
        ((pos[0] - self.origin[0]) * sx + (pos[2] - self.origin[2]) * sz) / len
    }

    /// Consecutive walk nodes from the current one heading the same way.
    fn straight_run(&self) -> usize {
        let mut prev = self.origin;
        let heading = dir(prev, center(&self.path[self.index]));
        let mut run = 0;
        for n in &self.path[self.index..] {
            if n.kind != MoveKind::Walk || !n.work.is_empty() || dir(prev, center(n)) != heading {
                break;
            }
            prev = center(n);
            run += 1;
        }
        run
    }

    fn advance(&mut self, s: &Sense) {
        let before = self.index;
        while self.index + 1 < self.path.len() && !self.has_pending(self.index) {
            let node = self.path[self.index];
            let next = self.path[self.index + 1];
            let straight = next.kind == MoveKind::Walk && dir(self.origin, center(&node)) == dir(center(&node), center(&next));
            if !self.reached(&node, s, if straight { 0.45 } else { 0.25 }) {
                break;
            }
            self.origin = center(&node);
            self.index += 1;
            self.worked = false;
        }
        // Overshot onto a later node (momentum, a fall): continue from there, but not past work.
        let end = self.index + self.standing_ahead().len();
        let window = (self.index + 1..end.min(self.index + 4)).rev();
        for j in window {
            let n = self.path[j];
            let feet_cell = [s.pos[0].floor() as i32, s.pos[2].floor() as i32];
            if feet_cell == [n.pos[0], n.pos[2]] && (s.pos[1] - n.feet).abs() < 0.3 && (s.on_ground || s.in_water) {
                self.origin = center(&self.path[j - 1]);
                self.index = j;
                self.worked = false;
                break;
            }
        }
        if self.index != before {
            (self.best_dist, self.since_progress) = (f32::INFINITY, 0);
        }
    }

    fn reached(&self, n: &PathNode, s: &Sense, radius: f32) -> bool {
        let h = hdist(s.pos, center(n));
        let dy = s.pos[1] - n.feet;
        match n.kind {
            MoveKind::ClimbUp => dy > -0.05 && h < 0.5,
            MoveKind::ClimbDown => dy < 0.1 && h < 0.5,
            MoveKind::Ascend => h < radius && dy > -0.25 && dy < 0.6,
            MoveKind::Parkour | MoveKind::Descend | MoveKind::Down => h < radius.max(0.35) && dy.abs() < 0.5 && (s.on_ground || s.in_water),
            MoveKind::Pillar => h < 0.5 && dy > -0.1 && dy < 0.6 && s.on_ground,
            MoveKind::Walk | MoveKind::Swim => h < radius && dy.abs() < 0.6,
        }
    }

    fn off_path(&self, pos: Vec3, target: Vec3) -> bool {
        let (ax, az) = (self.origin[0], self.origin[2]);
        let (sx, sz) = (target[0] - ax, target[2] - az);
        let len2 = sx * sx + sz * sz;
        let t = if len2 > 0.0 { (((pos[0] - ax) * sx + (pos[2] - az) * sz) / len2).clamp(0.0, 1.0) } else { 0.0 };
        let off = (pos[0] - ax - sx * t).hypot(pos[2] - az - sz * t);
        let (lo, hi) = (self.origin[1].min(target[1]), self.origin[1].max(target[1]));
        off > OFF_PATH || pos[1] < lo - 1.5 || pos[1] > hi + 2.5
    }
}
