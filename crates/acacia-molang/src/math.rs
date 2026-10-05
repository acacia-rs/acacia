//! `math.*`. Angles are degrees; every result is what BDS answers (`tests/oracle/math.bds`).

use std::f32::consts::PI;

macro_rules! math_functions {
    ($($name:literal $variant:ident $arity:literal,)*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum MathFn { $($variant,)* }

        impl MathFn {
            /// By the name after `math.`, with the argument count it takes.
            pub(crate) fn find(name: &str) -> Option<(MathFn, &'static str, u8)> {
                match name {
                    $($name => Some((MathFn::$variant, $name, $arity)),)*
                    _ => None,
                }
            }
        }
    };
}

math_functions! {
    "abs" Abs 1, "acos" Acos 1, "asin" Asin 1, "atan" Atan 1, "atan2" Atan2 2, "ceil" Ceil 1, "clamp" Clamp 3,
    "copy_sign" CopySign 2, "cos" Cos 1, "die_roll" DieRoll 3, "die_roll_integer" DieRollInteger 3, "exp" Exp 1,
    "floor" Floor 1, "hermite_blend" HermiteBlend 1, "inverse_lerp" InverseLerp 3, "lerp" Lerp 3,
    "lerprotate" LerpRotate 3, "ln" Ln 1, "max" Max 2, "min" Min 2, "min_angle" MinAngle 1, "mod" Mod 2, "pi" Pi 0,
    "pow" Pow 2, "random" Random 2, "random_integer" RandomInteger 2, "round" Round 1, "sign" Sign 1, "sin" Sin 1,
    "sqrt" Sqrt 1, "trunc" Trunc 1,
    "ease_in_back" EaseInBack 3, "ease_in_bounce" EaseInBounce 3, "ease_in_circ" EaseInCirc 3,
    "ease_in_cubic" EaseInCubic 3, "ease_in_elastic" EaseInElastic 3, "ease_in_expo" EaseInExpo 3,
    "ease_in_quad" EaseInQuad 3, "ease_in_quart" EaseInQuart 3, "ease_in_quint" EaseInQuint 3,
    "ease_in_sine" EaseInSine 3,
    "ease_out_back" EaseOutBack 3, "ease_out_bounce" EaseOutBounce 3, "ease_out_circ" EaseOutCirc 3,
    "ease_out_cubic" EaseOutCubic 3, "ease_out_elastic" EaseOutElastic 3, "ease_out_expo" EaseOutExpo 3,
    "ease_out_quad" EaseOutQuad 3, "ease_out_quart" EaseOutQuart 3, "ease_out_quint" EaseOutQuint 3,
    "ease_out_sine" EaseOutSine 3,
    "ease_in_out_back" EaseInOutBack 3, "ease_in_out_bounce" EaseInOutBounce 3, "ease_in_out_circ" EaseInOutCirc 3,
    "ease_in_out_cubic" EaseInOutCubic 3, "ease_in_out_elastic" EaseInOutElastic 3, "ease_in_out_expo" EaseInOutExpo 3,
    "ease_in_out_quad" EaseInOutQuad 3, "ease_in_out_quart" EaseInOutQuart 3, "ease_in_out_quint" EaseInOutQuint 3,
    "ease_in_out_sine" EaseInOutSine 3,
}

/// Into [-180, 180).
fn min_angle(degrees: f32) -> f32 {
    let wrapped = (degrees + 180.0).rem_euclid(360.0) - 180.0;
    if wrapped >= 180.0 { -180.0 } else { wrapped }
}

impl MathFn {
    /// `random` yields [0, 1) and is only called by the random functions.
    pub(crate) fn call(self, [a, b, c]: [f32; 3], random: &dyn Fn() -> f32) -> f32 {
        use MathFn::*;
        match self {
            Abs => a.abs(),
            Acos => a.acos().to_degrees(),
            Asin => a.asin().to_degrees(),
            Atan => a.atan().to_degrees(),
            Atan2 => a.atan2(b).to_degrees(),
            Ceil => a.ceil(),
            Clamp => a.max(b).min(c),
            CopySign => a.copysign(b),
            Cos => a.to_radians().cos(),
            DieRoll => (0..a.max(0.0) as u32).map(|_| b + (c - b) * random()).sum(),
            DieRollInteger => (0..a.max(0.0) as u32).map(|_| random_integer(b, c, random)).sum(),
            Exp => a.exp(),
            Floor => a.floor(),
            HermiteBlend => 3.0 * a * a - 2.0 * a * a * a,
            InverseLerp => (c - a) / (b - a),
            Lerp => a + (b - a) * c,
            LerpRotate => a + min_angle(b - a) * c,
            Ln => a.ln(),
            Max => a.max(b),
            Min => a.min(b),
            MinAngle => min_angle(a),
            Mod => a % b,
            Pi => PI,
            Pow => a.powf(b),
            Random => a + (b - a) * random(),
            RandomInteger => random_integer(a, b, random),
            Round => a.round(),
            Sign => if a < 0.0 { -1.0 } else { 1.0 },
            Sin => a.to_radians().sin(),
            Sqrt => a.sqrt(),
            Trunc => a.trunc(),
            _ => a + (b - a) * self.ease(c),
        }
    }

    /// The easing curve at `t`; formulas as on easings.net, without its special cases at 0 and 1.
    fn ease(self, t: f32) -> f32 {
        use MathFn::*;
        let out = |curve: fn(f32) -> f32, t: f32| 1.0 - curve(1.0 - t);
        let in_out = |curve: fn(f32) -> f32, t: f32| if t < 0.5 { curve(2.0 * t) / 2.0 } else { 1.0 - curve(2.0 - 2.0 * t) / 2.0 };
        let curve: fn(f32) -> f32 = match self {
            EaseInQuad | EaseOutQuad | EaseInOutQuad => |t| t * t,
            EaseInCubic | EaseOutCubic | EaseInOutCubic => |t| t * t * t,
            EaseInQuart | EaseOutQuart | EaseInOutQuart => |t| t * t * t * t,
            EaseInQuint | EaseOutQuint | EaseInOutQuint => |t| t * t * t * t * t,
            EaseInSine | EaseOutSine | EaseInOutSine => |t| 1.0 - (t * PI / 2.0).cos(),
            EaseInExpo | EaseOutExpo | EaseInOutExpo => |t| 2f32.powf(10.0 * t - 10.0),
            EaseInCirc | EaseOutCirc | EaseInOutCirc => |t| 1.0 - (1.0 - t * t).sqrt(),
            EaseInBack | EaseOutBack => |t| 2.70158 * t * t * t - 1.70158 * t * t,
            EaseInOutBack => |t| t * t * (3.5949095 * t - 2.5949095),
            EaseInElastic | EaseInOutElastic => |t| -(2f32.powf(10.0 * t - 10.0)) * table_sin((10.0 * t - 10.75) * (2.0 * PI / 3.0)),
            EaseOutElastic => |t| 2f32.powf(-10.0 * t) * table_sin((10.0 * t - 0.75) * (2.0 * PI / 3.0)) + 1.0,
            _ => |t| 1.0 - bounce(1.0 - t),
        };
        match self {
            EaseOutElastic => curve(t),
            EaseInQuad | EaseInCubic | EaseInQuart | EaseInQuint | EaseInSine | EaseInExpo | EaseInCirc | EaseInBack
            | EaseInElastic | EaseInBounce => curve(t),
            EaseOutQuad | EaseOutCubic | EaseOutQuart | EaseOutQuint | EaseOutSine | EaseOutExpo | EaseOutCirc
            | EaseOutBack | EaseOutBounce => out(curve, t),
            _ => in_out(curve, t),
        }
    }
}

/// The game's 65536-step sine table, which the elastic curves use in place of a real sine.
fn table_sin(radians: f32) -> f32 {
    let step = (radians * 10430.378) as i32 & 0xffff;
    (f64::from(step) * std::f64::consts::TAU / 65536.0).sin() as f32
}

fn random_integer(low: f32, high: f32, random: &dyn Fn() -> f32) -> f32 {
    (low + (high - low + 1.0) * random()).floor().min(high.max(low))
}

fn bounce(t: f32) -> f32 {
    const N: f32 = 7.5625;
    const D: f32 = 2.75;
    if t < 1.0 / D {
        N * t * t
    } else if t < 2.0 / D {
        let t = t - 1.5 / D;
        N * t * t + 0.75
    } else if t < 2.5 / D {
        let t = t - 2.25 / D;
        N * t * t + 0.9375
    } else {
        let t = t - 2.625 / D;
        N * t * t + 0.984375
    }
}
