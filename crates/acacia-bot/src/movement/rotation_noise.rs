//! Sub-degree noise on every turn, so sent rotations never repeat exact angles: anticheats flag turns that
//! land on 0.1° multiples and yaw turns with pitch held exactly (research/anticheat-interaction-checks.md).

use crate::cadence::splitmix;

/// Noise amplitude per axis on each turn, degrees.
const MIN_DEG: f32 = 0.005;
const MAX_DEG: f32 = 0.03;
/// A turn this close to a 0.1° multiple is redrawn.
const ROUND_SLACK: f32 = 1e-3;

/// The rotation actually sent for the requested one.
pub(crate) struct RotationNoise {
    rng: u64,
    requested: Option<(f32, f32)>,
    sent: (f32, f32),
}

impl RotationNoise {
    pub(crate) fn new(seed: u64) -> Self {
        Self { rng: seed, requested: None, sent: (0.0, 0.0) }
    }

    /// `(yaw, pitch)` to send this tick for the requested `(yaw, pitch)`. A request equal to the last one keeps
    /// the last sent rotation; any change renoises both axes, so pitch moves whenever yaw does.
    pub(crate) fn apply(&mut self, yaw: f32, pitch: f32) -> (f32, f32) {
        if self.requested == Some((yaw, pitch)) {
            return self.sent;
        }
        let first = self.requested.is_none();
        self.requested = Some((yaw, pitch));
        let prev = self.sent;
        self.sent = loop {
            let y = yaw + self.offset();
            let p = (pitch + self.offset()).clamp(-90.0, 90.0);
            if first || !(on_tenth(y - prev.0) || on_tenth(p - prev.1)) {
                break (y, p);
            }
        };
        self.sent
    }

    fn offset(&mut self) -> f32 {
        let r = splitmix(&mut self.rng);
        let magnitude = MIN_DEG + (MAX_DEG - MIN_DEG) * (r >> 40) as f32 / (1u64 << 24) as f32;
        if r & 1 == 0 { magnitude } else { -magnitude }
    }
}

fn on_tenth(d: f32) -> bool {
    let tenths = d * 10.0;
    ((tenths - tenths.round()) / 10.0).abs() < ROUND_SLACK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_aligned_turns_never_land_on_tenths() {
        let mut n = RotationNoise::new(7);
        n.apply(0.0, 0.0);
        let mut last = n.sent;
        for (i, &(yaw, pitch)) in [(90.0, 0.0), (180.0, 0.0), (-90.0, 90.0), (0.0, 0.0), (45.0, 30.0)].iter().cycle().take(500).enumerate() {
            let sent = n.apply(yaw, pitch);
            assert!(!on_tenth(sent.0 - last.0) && !on_tenth(sent.1 - last.1), "turn {i}: {last:?} -> {sent:?}");
            assert!((sent.0 - yaw).abs() <= MAX_DEG && (sent.1 - pitch).abs() <= MAX_DEG + 1e-6);
            last = sent;
        }
    }

    #[test]
    fn a_held_request_keeps_its_rotation_and_a_yaw_turn_moves_pitch() {
        let mut n = RotationNoise::new(3);
        let a = n.apply(10.0, 0.0);
        assert_eq!(n.apply(10.0, 0.0), a);
        let b = n.apply(20.0, 0.0);
        assert_ne!(a.1, b.1);
    }

    #[test]
    fn pitch_stays_in_range() {
        let mut n = RotationNoise::new(11);
        for i in 0..200 {
            let (_, p) = n.apply(i as f32, if i % 2 == 0 { 90.0 } else { -90.0 });
            assert!((-90.0..=90.0).contains(&p));
        }
    }
}
