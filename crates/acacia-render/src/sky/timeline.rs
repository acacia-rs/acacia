//! Java's timelines (`data/minecraft/timeline/*.json`): keyframe tracks sampled by the clock's
//! ticks (`KeyframeTrackSampler`, `EasingType`), each applied to an attribute by its modifier.

use serde_json::Value;

/// An attribute value: a number in `[0]`, a colour as `[r, g, b, a]` in 0..1 (sRGB), a moon
/// phase as its index in `[0]`.
pub type Value4 = [f32; 4];

/// `MoonPhase` in the timeline's order.
pub const MOON_PHASES: [&str; 8] =
    ["full_moon", "waning_gibbous", "third_quarter", "waning_crescent", "new_moon", "waxing_crescent", "first_quarter", "waxing_gibbous"];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    Linear,
    Constant,
    Bezier([f32; 4]),
}

impl Easing {
    pub fn apply(self, x: f32) -> f32 {
        match self {
            Easing::Linear => x,
            Easing::Constant => 0.0,
            Easing::Bezier(control) => bezier(control, x),
        }
    }
}

/// `EasingType.cubicBezier`: Newton steps on x, then bisection when they stall.
fn bezier([x1, y1, x2, y2]: [f32; 4], x: f32) -> f32 {
    let curve = |c1: f32, c2: f32| (3.0 * c1 - 3.0 * c2 + 1.0, -6.0 * c1 + 3.0 * c2, 3.0 * c1);
    let (cx, cy) = (curve(x1, x2), curve(y1, y2));
    let sample = |(a, b, c): (f32, f32, f32), t: f32| ((a * t + b) * t + c) * t;
    let gradient = |t: f32| (3.0 * cx.0 * t + 2.0 * cx.1) * t + cx.2;
    let mut t = x;
    for _ in 0..4 {
        let error = sample(cx, t) - x;
        if error.abs() < 1e-5 {
            return sample(cy, t);
        }
        let g = gradient(t);
        if g < 1e-5 {
            break;
        }
        t -= (error / g).clamp(-0.25, 0.25);
    }
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    while t0 < t1 {
        let error = sample(cx, t) - x;
        if error.abs() < 1e-5 {
            break;
        }
        if error < 0.0 { t0 = t } else { t1 = t }
        let next = (t0 + t1) / 2.0;
        if next == t {
            break;
        }
        t = next;
    }
    sample(cy, t)
}

/// How a layer's argument changes the value under it (`AttributeModifier`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    Override,
    Multiply,
    Add,
    Subtract,
    Minimum,
    Maximum,
}

impl Modifier {
    pub fn parse(name: Option<&str>) -> Option<Modifier> {
        Some(match name.unwrap_or("override") {
            "override" => Modifier::Override,
            "multiply" => Modifier::Multiply,
            "add" => Modifier::Add,
            "subtract" => Modifier::Subtract,
            "minimum" => Modifier::Minimum,
            "maximum" => Modifier::Maximum,
            _ => return None,
        })
    }

    pub fn apply(self, base: Value4, argument: Value4) -> Value4 {
        let each = |f: fn(f32, f32) -> f32| std::array::from_fn(|i| f(base[i], argument[i]));
        match self {
            Modifier::Override => argument,
            Modifier::Multiply => each(|a, b| a * b),
            Modifier::Add => each(|a, b| a + b),
            Modifier::Subtract => each(|a, b| a - b),
            Modifier::Minimum => each(f32::min),
            Modifier::Maximum => each(f32::max),
        }
    }
}

/// A number, a boolean, `#rrggbb` (alpha 1), `#aarrggbb`, or a moon phase.
pub fn parse_value(v: &Value) -> Option<Value4> {
    match v {
        Value::Number(n) => Some([n.as_f64()? as f32, 0.0, 0.0, 0.0]),
        Value::Bool(b) => Some([f32::from(u8::from(*b)), 0.0, 0.0, 0.0]),
        Value::String(s) => match s.strip_prefix('#') {
            Some(hex) => {
                let c = u32::from_str_radix(hex, 16).ok()?;
                let f = |shift: u32| (c >> shift & 255) as f32 / 255.0;
                Some([f(16), f(8), f(0), if hex.len() == 8 { f(24) } else { 1.0 }])
            }
            None => MOON_PHASES.iter().position(|p| p == s).map(|i| [i as f32, 0.0, 0.0, 0.0]),
        },
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub keyframes: Vec<(i64, Value4)>,
    pub easing: Easing,
    pub modifier: Modifier,
}

impl Track {
    /// `None` for a track of values that are not numbers, colours or moon phases.
    pub fn parse(v: &Value) -> Option<Track> {
        let keyframes: Vec<_> = v["keyframes"].as_array()?.iter().map(|k| Some((k["ticks"].as_i64()?, parse_value(&k["value"])?))).collect::<Option<_>>()?;
        // Moon phases are steps: an enum has nothing between its values.
        let stepped = v["keyframes"][0]["value"].as_str().is_some_and(|s| !s.starts_with('#'));
        let easing = match &v["ease"] {
            _ if stepped => Easing::Constant,
            Value::String(s) if s == "constant" => Easing::Constant,
            Value::Object(o) => match o.get("cubic_bezier").and_then(Value::as_array) {
                Some(a) if a.len() == 4 => Easing::Bezier(std::array::from_fn(|i| a[i].as_f64().unwrap_or(0.0) as f32)),
                _ => Easing::Linear,
            },
            _ => Easing::Linear,
        };
        (!keyframes.is_empty()).then_some(Track { keyframes, easing, modifier: Modifier::parse(v["modifier"].as_str())? })
    }

    /// `KeyframeTrackSampler.sample` at the clock's whole `ticks` (it has no partial tick), the
    /// timeline repeating every `period` ticks.
    pub fn sample(&self, ticks: i64, period: i64) -> Value4 {
        let k = &self.keyframes;
        if k.len() == 1 {
            return k[0].1;
        }
        let t = ticks.rem_euclid(period);
        let (first, last) = (k[0], k[k.len() - 1]);
        let wrap_in = (last.1, last.0 - period, first.1, first.0);
        let wrap_out = (last.1, last.0, first.1, first.0 + period);
        let mut segments = std::iter::once(wrap_in).chain(k.windows(2).map(|p| (p[0].1, p[0].0, p[1].1, p[1].0))).chain(std::iter::once(wrap_out));
        let (from, from_t, to, to_t) = segments.find(|s| t < s.3).unwrap_or(wrap_out);
        if t <= from_t {
            return from;
        }
        if t >= to_t {
            return to;
        }
        let alpha = self.easing.apply((t - from_t) as f32 / (to_t - from_t) as f32);
        std::array::from_fn(|i| from[i] + alpha * (to[i] - from[i]))
    }
}

/// One timeline's tracks by attribute name.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Timeline {
    pub period: i64,
    pub tracks: Vec<(String, Track)>,
}

impl Timeline {
    pub fn parse(v: &Value) -> Timeline {
        let tracks = v["tracks"].as_object().into_iter().flatten().filter_map(|(name, t)| Some((name.clone(), Track::parse(t)?))).collect();
        Timeline { period: v["period_ticks"].as_i64().filter(|p| *p > 0).unwrap_or(24000), tracks }
    }

    pub fn track(&self, attribute: &str) -> Option<&Track> {
        self.tracks.iter().find_map(|(name, t)| (name == attribute).then_some(t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn values_parse_as_numbers_colours_and_phases() {
        assert_eq!(parse_value(&json!(0.5)), Some([0.5, 0.0, 0.0, 0.0]));
        assert_eq!(parse_value(&json!("#ff0000")), Some([1.0, 0.0, 0.0, 1.0]));
        assert_eq!(parse_value(&json!("#00ff0000")), Some([1.0, 0.0, 0.0, 0.0]));
        assert_eq!(parse_value(&json!("new_moon")), Some([4.0, 0.0, 0.0, 0.0]));
        assert_eq!(parse_value(&json!({"a": 1})), None);
    }

    #[test]
    fn a_track_wraps_over_the_period_and_holds_at_flat_keys() {
        let track = Track::parse(&json!({"keyframes": [{"ticks": 1000, "value": 1.0}, {"ticks": 2000, "value": 3.0}], "modifier": "multiply"})).unwrap();
        assert_eq!(track.modifier, Modifier::Multiply);
        assert_eq!(track.sample(1500, 24000), [2.0, 0.0, 0.0, 0.0]);
        // From 2000 back round to 1000 + 24000: a quarter of the way at 7750.
        assert_eq!(track.sample(7750, 24000)[0], 2.5);
        assert_eq!(track.sample(500 + 24000 * 3, 24000)[0], track.sample(500, 24000)[0]);
    }

    #[test]
    fn two_keys_on_one_tick_make_a_full_turn() {
        // The sun's track: 360 then 0 at 6000, eased round the whole day.
        let track = Track::parse(&json!({"ease": {"cubic_bezier": [0.362, 0.241, 0.638, 0.759]}, "keyframes": [{"ticks": 6000, "value": 360.0}, {"ticks": 6000, "value": 0.0}]})).unwrap();
        assert_eq!(track.sample(6000, 24000)[0], 0.0);
        assert!((track.sample(18000, 24000)[0] - 180.0).abs() < 1e-3);
        let dawn = track.sample(0, 24000)[0];
        assert!(dawn > 180.0 && dawn < 360.0, "{dawn}");
    }

    #[test]
    fn moon_phases_step() {
        let track = Track::parse(&json!({"keyframes": [{"ticks": 0, "value": "full_moon"}, {"ticks": 24000, "value": "waning_gibbous"}]})).unwrap();
        assert_eq!(track.easing, Easing::Constant);
        assert_eq!((track.sample(23999, 192000)[0], track.sample(24000, 192000)[0], track.sample(100000, 192000)[0]), (0.0, 1.0, 1.0));
    }

    #[test]
    fn bezier_ends_and_middle() {
        let ease = Easing::Bezier([0.362, 0.241, 0.638, 0.759]);
        assert!(ease.apply(0.0).abs() < 1e-4 && (ease.apply(1.0) - 1.0).abs() < 1e-4);
        assert!((ease.apply(0.5) - 0.5).abs() < 1e-4, "symmetric control points");
        assert_eq!(Modifier::Maximum.apply([0.2, 0.0, 0.0, 0.0], [0.5, 0.0, 0.0, 0.0])[0], 0.5);
    }
}
