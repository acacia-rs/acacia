//! `math.*`. Angles are degrees; every result is what BDS answers (`tests/oracle/math.bds`).

use std::f32::consts::PI;

mod ease;

/// BDS's radians-to-degrees factor, one step below `f32::to_degrees`'.
const RAD_TO_DEG: f32 = f32::from_bits(0x4265_2ee0);

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
    pub(crate) fn is_random(self) -> bool {
        matches!(self, MathFn::Random | MathFn::RandomInteger | MathFn::DieRoll | MathFn::DieRollInteger)
    }

    /// `random` yields [0, 1) and is only called by the random functions.
    pub(crate) fn call(self, [a, b, c]: [f32; 3], random: &dyn Fn() -> f32) -> f32 {
        use MathFn::*;
        match self {
            Abs => a.abs(),
            Acos => degrees(inverse_trig_argument(a).acos()),
            Asin => degrees(inverse_trig_argument(a).asin()),
            Atan => degrees(f64::from(a).atan()),
            Atan2 => degrees(f64::from(a).atan2(f64::from(b))),
            Ceil => a.ceil(),
            // In BDS's order, which shows when the bounds are crossed: `clamp(1, 2.5, 1.5)` is 2.5.
            Clamp => if a > c { c } else if a < b { b } else { a },
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
            // Not `f32::max`: BDS answers the second argument when either is not a number.
            Max => if a > b { a } else { b },
            Min => if a < b { a } else { b },
            MinAngle => min_angle(a),
            // A divisor that is a constant 0 never gets here (`Parser::math`).
            Mod => if b == 0.0 { 0.0 } else { a % b },
            Pi => PI,
            Pow => a.powf(b),
            Random => a + (b - a) * random(),
            RandomInteger => random_integer(a, b, random),
            Round => a.round(),
            Sign => if a < 0.0 { -1.0 } else { 1.0 },
            Sin => a.to_radians().sin(),
            Sqrt => a.sqrt(),
            Trunc => a.trunc(),
            _ => ease::ease(self, a, b, c),
        }
    }
}

/// Rounded to `f32` before the conversion.
fn degrees(radians: f64) -> f32 {
    radians as f32 * RAD_TO_DEG
}

/// BDS clamps an argument up to 1.0005 out into [-1, 1]; only beyond that is it NaN.
fn inverse_trig_argument(x: f32) -> f64 {
    const TOLERANCE: f32 = f32::from_bits(0x3f80_1062);
    f64::from(if x.abs() > TOLERANCE { x } else { x.clamp(-1.0, 1.0) })
}

fn random_integer(low: f32, high: f32, random: &dyn Fn() -> f32) -> f32 {
    (low + (high - low + 1.0) * random()).floor().min(high.max(low))
}
