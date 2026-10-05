//! Entities between snapshots on the window thread: [`Smoother`] blends each one from where it
//! was drawn at the previous snapshot towards the newest, and poses it for the frame.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use acacia_render::entity::{EntityInstance, EntityModels, Value};
use glam::DVec3;

use crate::entities::{SNAPSHOT_SECS, Tracked, wrap_degrees};

/// The bot's own body is hidden while the camera is this close to its eyes.
const OWN_HEAD_RADIUS: f64 = 0.6;

/// Java's limb swing: how far the legs are through their stride, and how wide they swing.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Walk {
    distance: f32,
    speed: f32,
}

impl Walk {
    /// One tick on, having moved `blocks` over the ground.
    fn step(self, blocks: f32) -> Walk {
        let speed = self.speed + ((blocks * 4.0).min(1.0) - self.speed) * 0.4;
        Walk { distance: self.distance + speed, speed }
    }
}

/// What blends between snapshots.
#[derive(Clone, Copy)]
struct Motion {
    position: DVec3,
    yaw: f32,
    head_yaw: f32,
    pitch: f32,
    walk: Walk,
}

impl Motion {
    fn of(e: &Tracked, walk: Walk) -> Motion {
        Motion { position: e.instance.position, yaw: e.instance.yaw, head_yaw: e.head_yaw, pitch: e.pitch, walk }
    }

    fn towards(self, to: Motion, t: f32) -> Motion {
        let angle = |a: f32, b: f32| a + wrap_degrees(b - a) * t;
        let mix = |a: f32, b: f32| a + (b - a) * t;
        Motion {
            position: self.position.lerp(to.position, f64::from(t)),
            yaw: angle(self.yaw, to.yaw),
            head_yaw: angle(self.head_yaw, to.head_yaw),
            pitch: angle(self.pitch, to.pitch),
            walk: Walk { distance: mix(self.walk.distance, to.walk.distance), speed: mix(self.walk.speed, to.walk.speed) },
        }
    }
}

#[derive(Default)]
pub struct Smoother {
    models: Arc<EntityModels>,
    from: HashMap<u64, Motion>,
    to: Vec<(Tracked, Motion)>,
    since: Option<Instant>,
    /// When each entity was first seen, for `query.life_time`.
    born: HashMap<u64, Instant>,
}

impl Smoother {
    pub fn set_models(&mut self, models: Arc<EntityModels>) {
        self.models = models;
    }

    pub fn push(&mut self, snapshot: Vec<Tracked>) {
        let t = self.progress();
        let now = Instant::now();
        let shown: HashMap<u64, Motion> = self.to.iter().map(|(e, to)| (e.runtime_id, self.blend(e.runtime_id, *to, t))).collect();
        let last: HashMap<u64, Motion> = self.to.iter().map(|(e, to)| (e.runtime_id, *to)).collect();
        self.born.retain(|id, _| last.contains_key(id));
        self.to = snapshot
            .into_iter()
            .map(|e| {
                self.born.entry(e.runtime_id).or_insert(now);
                let walk = last.get(&e.runtime_id).map_or(Walk::default(), |before| {
                    let moved = e.instance.position - before.position;
                    before.walk.step(moved.x.hypot(moved.z) as f32)
                });
                let motion = Motion::of(&e, walk);
                (e, motion)
            })
            .collect();
        self.from = shown;
        self.since = Some(now);
    }

    fn progress(&self) -> f32 {
        self.since.map_or(1.0, |s| (s.elapsed().as_secs_f32() / SNAPSHOT_SECS).min(1.0))
    }

    fn blend(&self, id: u64, to: Motion, t: f32) -> Motion {
        self.from.get(&id).map_or(to, |from| from.towards(to, t))
    }

    pub fn instances(&self, camera: DVec3) -> Vec<EntityInstance> {
        let t = self.progress();
        let visible = |(e, _): &&(Tracked, Motion)| e.own_eyes.is_none_or(|eyes| eyes.distance(camera) > OWN_HEAD_RADIUS);
        let posed = |(e, to): &(Tracked, Motion)| {
            let m = self.blend(e.runtime_id, *to, t);
            let life = self.born.get(&e.runtime_id).map_or(0.0, |b| b.elapsed().as_secs_f32());
            let query = |name: &str| {
                Value::Num(match name {
                    "life_time" => life,
                    "modified_distance_moved" => m.walk.distance,
                    "modified_move_speed" => m.walk.speed,
                    "target_x_rotation" => m.pitch,
                    "target_y_rotation" => wrap_degrees(m.head_yaw - m.yaw),
                    "is_on_ground" | "is_alive" => 1.0,
                    _ => return e.facts.query(name),
                })
            };
            let pose = e.instance.layers.first().map(|l| self.models.pose(&e.kind, l.model, &query)).unwrap_or_default();
            EntityInstance { position: m.position, yaw: m.yaw, pose, ..e.instance.clone() }
        };
        self.to.iter().filter(visible).map(posed).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legs_swing_while_walking_and_settle_when_standing() {
        let walking = (0..40).fold(Walk::default(), |w, _| w.step(0.1));
        assert!((walking.speed - 0.4).abs() < 1e-3 && walking.distance > 10.0, "{walking:?}");
        let stopped = (0..40).fold(walking, |w, _| w.step(0.0));
        assert!(stopped.speed < 1e-3 && stopped.distance - walking.distance < 1.0, "{stopped:?}");
    }
}
