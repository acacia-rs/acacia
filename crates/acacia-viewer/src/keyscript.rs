//! `ACACIA_KEYS="3 +Space; 3.15 -Space; ..."`: key presses (+) and releases (-) at seconds after play starts,
//! for unattended live tests of the controls (e.g. flight's double tap).

use std::time::{Duration, Instant};

use winit::keyboard::KeyCode;

pub struct KeyScript {
    /// (when, key, pressed), latest first.
    steps: Vec<(Duration, KeyCode, bool)>,
    start: Option<Instant>,
}

impl KeyScript {
    pub fn from_env() -> Option<Self> {
        let spec = std::env::var("ACACIA_KEYS").ok()?;
        let mut steps: Vec<_> = spec.split(';').filter_map(|s| parse(s.trim())).collect();
        steps.sort_by(|a, b| b.0.cmp(&a.0));
        Some(KeyScript { steps, start: None })
    }

    /// The key changes due by `now`; the clock starts on the first call.
    pub fn due(&mut self, now: Instant) -> Vec<(KeyCode, bool)> {
        let start = *self.start.get_or_insert(now);
        let mut out = Vec::new();
        while self.steps.last().is_some_and(|s| start + s.0 <= now) {
            let (_, key, pressed) = self.steps.pop().expect("checked");
            out.push((key, pressed));
        }
        out
    }
}

fn parse(step: &str) -> Option<(Duration, KeyCode, bool)> {
    let (at, key) = step.split_once(' ')?;
    let (pressed, name) = match key.trim().split_at(1) {
        ("+", name) => (true, name),
        ("-", name) => (false, name),
        _ => return None,
    };
    let key = match name {
        "Space" => KeyCode::Space,
        "Shift" => KeyCode::ShiftLeft,
        "Ctrl" => KeyCode::ControlLeft,
        "W" => KeyCode::KeyW,
        "A" => KeyCode::KeyA,
        "S" => KeyCode::KeyS,
        "D" => KeyCode::KeyD,
        _ => return None,
    };
    Some((Duration::from_secs_f32(at.parse().ok()?), key, pressed))
}
