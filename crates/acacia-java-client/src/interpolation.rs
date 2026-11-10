//! Other entities' positions as a vanilla client holds them: `SteppedInterpolationHandler` (26.3), positions
//! only; rotations are taken as sent. Anticheats judge reach against this lagging view, not the newest packet.
//! Non-living types get vanilla's linear or no-op handlers; this treats them as stepped, or snaps fast-updating
//! ones (projectiles, items), which nothing reaches for.

use std::collections::VecDeque;

use acacia_java_proto::entity::PositionPath;

/// Squared drift above which the entity moved outside interpolation (a snap) since the last tick.
const MOVED: f64 = 9.999999747378752E-6;

#[derive(Debug, Clone, PartialEq)]
pub struct Lerp {
    /// Ticks per server update (`EntityType.updateInterval`); 0 snaps.
    steps: u32,
    queue: VecDeque<([f64; 3], u32)>,
    /// Final target and its rotation.
    target: ([f64; 3], f32, f32),
    last_step: [f64; 3],
    current: f32,
    remaining: f32,
    speed: f32,
    last: [f64; 3],
}

impl Lerp {
    /// The handler for an entity type, by registry name.
    pub fn for_kind(name: &str) -> Self {
        Lerp { steps: steps(name), queue: VecDeque::new(), target: ([0.0; 3], 0.0, 0.0), last_step: [0.0; 3], current: 0.0, remaining: 0.0, speed: 1.0, last: [0.0; 3] }
    }

    /// `interpolateTo`: queues `path` from the entity at `pos` facing `facing`; without a path, a hold where it is
    /// headed, and without a rotation, the one it is headed to.
    pub fn interpolate_to(&mut self, pos: &mut [f64; 3], facing: (f32, f32), path: Option<PositionPath>, rot: Option<(f32, f32)>) {
        let active = !self.queue.is_empty();
        let (base, by, bx) = if active { self.target } else { (*pos, facing.0, facing.1) };
        let path = path.unwrap_or(PositionPath::Linear(base));
        let (y, x) = rot.unwrap_or((by, bx));
        let end = path.end();
        if self.steps == 0 {
            *pos = end;
            self.cancel();
            return;
        }
        if active && self.target == (end, y, x) {
            return;
        }
        if !active {
            (self.last_step, self.current) = (*pos, 1.0);
        }
        match path {
            _ if end == self.target.0 => self.push(end, self.steps),
            PositionPath::Linear(end) => self.push(end, self.steps),
            PositionPath::Stepped(steps) => steps.into_iter().for_each(|(p, t)| self.push(p, t)),
        }
        self.target = (end, y, x);
        self.last = *pos;
    }

    /// `interpolate`, once a client tick before the entity's own tick: moves `pos` along the queue. A snap (setting
    /// `pos` directly) since the last call moves the queued steps along by the jump, as vanilla's `snapTo` does.
    pub fn interpolate(&mut self, pos: &mut [f64; 3]) {
        if self.queue.is_empty() {
            self.cancel();
            return;
        }
        let delta = [0, 1, 2].map(|i| pos[i] - self.last[i]);
        // Vanilla moves the queue only if the entity's box fits there; no collision check here.
        if delta.iter().map(|d| d * d).sum::<f64>() > MOVED {
            let shift = |p: &mut [f64; 3]| (0..3).for_each(|i| p[i] += delta[i]);
            shift(&mut self.target.0);
            shift(&mut self.last_step);
            self.queue.iter_mut().for_each(|(p, _)| shift(p));
        }
        *pos = self.next();
        let n = self.steps as f32;
        let catch_up = (self.remaining / n).max(1.0);
        self.speed += 1.0 / n * (catch_up - self.speed);
        let advance = if self.speed < self.remaining {
            self.speed
        } else {
            let all = self.remaining;
            self.speed = 1.0;
            all
        };
        self.current += advance;
        self.remaining -= advance;
        self.last = *pos;
    }

    fn next(&mut self) -> [f64; 3] {
        while let Some(&(p, t)) = self.queue.front() {
            if self.current < t as f32 {
                let a = f64::from(self.current / t as f32);
                return [0, 1, 2].map(|i| self.last_step[i] + a * (p[i] - self.last_step[i]));
            }
            self.current -= t as f32;
            self.last_step = p;
            self.queue.pop_front();
        }
        self.target.0
    }

    fn push(&mut self, pos: [f64; 3], ticks: u32) {
        self.queue.push_back((pos, ticks));
        self.remaining += ticks as f32;
    }

    fn cancel(&mut self) {
        self.queue.clear();
        (self.remaining, self.speed) = (0.0, 1.0);
    }
}

/// `EntityType.updateInterval` where it is under 4 (26.3 `EntityTypes`; living ones default to 3); 0 for the
/// fast-updating rest, which snap.
fn steps(name: &str) -> u32 {
    match name.strip_prefix("minecraft:").unwrap_or(name) {
        "player" | "allay" | "mannequin" | "evoker_fangs" => 2,
        "block_display" | "item_display" | "text_display" => 1,
        "arrow" | "spectral_arrow" | "trident" | "item" | "experience_orb" | "falling_block" | "fishing_bobber" | "eye_of_ender"
        | "breeze_wind_charge" | "wind_charge" | "cushion" | "dragon_fireball" | "fireball" | "small_fireball" | "wither_skull"
        | "egg" | "ender_pearl" | "snowball" | "experience_bottle" | "splash_potion" | "lingering_potion" | "firework_rocket"
        | "llama_spit" | "tnt" => 0,
        _ => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zombie_reaches_each_target_in_three_ticks() {
        let mut lerp = Lerp::for_kind("minecraft:zombie");
        let mut pos = [0.0, 64.0, 0.0];
        lerp.interpolate_to(&mut pos, (0.0, 0.0), Some(PositionPath::Stepped(vec![([0.3, 64.0, 0.0], 3)])), None);
        assert_eq!(pos[0], 0.0);
        let seen: Vec<f64> = (0..4).map(|_| { lerp.interpolate(&mut pos); pos[0] }).collect();
        assert_eq!(seen, [0.3 * f64::from(1.0_f32 / 3.0), 0.3 * f64::from(2.0_f32 / 3.0), 0.3, 0.3]);
    }

    #[test]
    fn a_packet_mid_way_queues_and_speeds_up() {
        let mut lerp = Lerp::for_kind("minecraft:zombie");
        let mut pos = [0.0; 3];
        let step = |x: f64| Some(PositionPath::Stepped(vec![([x, 0.0, 0.0], 3)]));
        lerp.interpolate_to(&mut pos, (0.0, 0.0), step(3.0), None);
        lerp.interpolate(&mut pos);
        lerp.interpolate_to(&mut pos, (0.0, 0.0), step(6.0), None);
        let seen: Vec<f64> = (0..5).map(|_| { lerp.interpolate(&mut pos); pos[0] }).collect();
        assert_eq!((seen[0], seen[4]), (3.0 * f64::from(2.0_f32 / 3.0), 6.0));
        assert!((3.2..3.25).contains(&seen[1]), "no catch-up: {seen:?}");
    }

    #[test]
    fn fast_types_snap() {
        let mut lerp = Lerp::for_kind("minecraft:arrow");
        let mut pos = [0.0; 3];
        lerp.interpolate_to(&mut pos, (0.0, 0.0), Some(PositionPath::Linear([1.0, 2.0, 3.0])), None);
        assert_eq!(pos, [1.0, 2.0, 3.0]);
    }
}
