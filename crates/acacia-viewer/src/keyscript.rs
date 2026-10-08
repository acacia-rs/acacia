//! `ACACIA_KEYS="3 +Space; 3.15 -Space; 9 click 640 300; ..."`: key presses (+) and releases (-), and
//! left clicks at window pixels, at seconds after play starts, for unattended live tests of the
//! controls (e.g. flight's double tap, menu buttons).

use std::time::{Duration, Instant};

use winit::keyboard::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Step {
    Key(KeyCode, bool),
    Click([f64; 2]),
}

pub struct KeyScript {
    /// (when, step), latest first.
    steps: Vec<(Duration, Step)>,
    start: Option<Instant>,
}

impl KeyScript {
    pub fn from_env() -> Option<Self> {
        let spec = std::env::var("ACACIA_KEYS").ok()?;
        let mut steps: Vec<_> = spec.split(';').filter_map(|s| parse(s.trim())).collect();
        steps.sort_by(|a, b| b.0.cmp(&a.0));
        Some(KeyScript { steps, start: None })
    }

    /// The steps due by `now`; the clock starts on the first call.
    pub fn due(&mut self, now: Instant) -> Vec<Step> {
        let start = *self.start.get_or_insert(now);
        let mut out = Vec::new();
        while self.steps.last().is_some_and(|s| start + s.0 <= now) {
            out.push(self.steps.pop().expect("checked").1);
        }
        out
    }
}

fn parse(step: &str) -> Option<(Duration, Step)> {
    let (at, what) = step.split_once(' ')?;
    let at = Duration::from_secs_f32(at.parse().ok()?);
    if let Some(xy) = what.trim().strip_prefix("click ") {
        let (x, y) = xy.trim().split_once(' ')?;
        return Some((at, Step::Click([x.parse().ok()?, y.trim().parse().ok()?])));
    }
    let (pressed, name) = match what.trim().split_at(1) {
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
        "E" => KeyCode::KeyE,
        "Tab" => KeyCode::Tab,
        "Esc" => KeyCode::Escape,
        _ => return None,
    };
    Some((at, Step::Key(key, pressed)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_clicks_parse() {
        assert_eq!(parse("3.5 +Space"), Some((Duration::from_secs_f32(3.5), Step::Key(KeyCode::Space, true))));
        assert_eq!(parse("9 click 640 300"), Some((Duration::from_secs(9), Step::Click([640.0, 300.0]))));
        assert_eq!(parse("1 +Nope"), None);
    }
}
