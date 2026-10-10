//! What blends between snapshots: where an entity is, how it is turned and its stride.

use glam::DVec3;

use crate::entities::{Tracked, wrap_degrees};

/// Java's limb swing: how far the legs are through their stride, and how wide they swing.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct Walk {
    pub distance: f32,
    pub speed: f32,
}

impl Walk {
    /// One tick on, having moved `blocks` over the ground.
    pub fn step(self, blocks: f32) -> Walk {
        let speed = self.speed + ((blocks * 4.0).min(1.0) - self.speed) * 0.4;
        Walk { distance: self.distance + speed, speed }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Motion {
    pub position: DVec3,
    pub yaw: f32,
    pub head_yaw: f32,
    pub pitch: f32,
    pub walk: Walk,
}

impl Motion {
    pub fn of(e: &Tracked, walk: Walk) -> Motion {
        Motion { position: e.instance.position, yaw: e.instance.yaw, head_yaw: e.head_yaw, pitch: e.pitch, walk }
    }

    pub fn towards(self, to: Motion, t: f32) -> Motion {
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
