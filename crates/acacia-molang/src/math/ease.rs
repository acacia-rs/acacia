//! `math.ease_*(b, end, t)` as BDS computes them: Penner's `(t, b, c)` formulas with `c = end - b`, every
//! operation rounded in the order written. Ported from molangx (Apache-2.0, see THIRD-PARTY.md).

use std::f32::consts::{FRAC_PI_2, PI};

use super::MathFn;

const BACK_C1: f32 = f32::from_bits(0x3fd9_cd60);
const BACK_C3: f32 = f32::from_bits(0x402c_e6b0);
const BACK_C2: f32 = f32::from_bits(0x4026_12ff);
const BACK_C2_PLUS_1: f32 = f32::from_bits(0x4066_12ff);
/// `1/2.75`, `2/2.75`, `2.5/2.75`, then the centres `1.5/2.75`, `2.25/2.75`, `2.625/2.75`.
const BOUNCE_T: [f32; 3] = [f32::from_bits(0x3eba_2e8c), f32::from_bits(0x3f3a_2e8c), f32::from_bits(0x3f68_ba2f)];
const BOUNCE_C: [f32; 3] = [f32::from_bits(0x3f0b_a2e9), f32::from_bits(0x3f51_745d), f32::from_bits(0x3f74_5d17)];
/// 65536/2π.
const SIN_INDEX_SCALE: f32 = f32::from_bits(0x4622_f983);

pub(super) fn ease(function: MathFn, b: f32, end: f32, t: f32) -> f32 {
    use MathFn::*;
    let c = end - b;
    let u = t + t;
    match function {
        EaseInQuad => c * t * t + b,
        EaseOutQuad => b - (t - 2.0) * (c * t),
        EaseInOutQuad if 1.0 > u => c * 0.5 * u * u + b,
        EaseInOutQuad => {
            let v = u - 1.0;
            ((v - 2.0) * v - 1.0) * (c * -0.5) + b
        }
        EaseInCubic => c * t * t * t + b,
        EaseOutCubic => {
            let v = t - 1.0;
            (v * v * v + 1.0) * c + b
        }
        EaseInOutCubic if 1.0 > u => c * 0.5 * u * u * u + b,
        EaseInOutCubic => {
            let w = u - 2.0;
            (w * w * w + 2.0) * (c * 0.5) + b
        }
        EaseInQuart => c * t * t * t * t + b,
        EaseOutQuart => {
            let v = t - 1.0;
            b - (v * v * v * v - 1.0) * c
        }
        EaseInOutQuart if 1.0 > u => c * 0.5 * u * u * u * u + b,
        EaseInOutQuart => {
            let w = u - 2.0;
            (w * w * w * w - 2.0) * (c * -0.5) + b
        }
        EaseInQuint => c * t * t * t * t * t + b,
        EaseOutQuint => {
            let v = t - 1.0;
            (v * v * v * v * v + 1.0) * c + b
        }
        EaseInOutQuint if 1.0 > u => c * 0.5 * u * u * u * u * u + b,
        EaseInOutQuint => {
            let w = u - 2.0;
            (w * w * w * w * w + 2.0) * (c * 0.5) + b
        }
        EaseInSine => c - table_cos(t * FRAC_PI_2) * c + b,
        EaseOutSine => c * table_sin(t * FRAC_PI_2) + b,
        EaseInOutSine => (table_cos(t * PI) - 1.0) * (c * -0.5) + b,
        EaseInExpo => exp2((t - 1.0) * 10.0) * c + b,
        EaseOutExpo => (1.0 - exp2(t * -10.0)) * c + b,
        EaseInOutExpo if 1.0 > u => exp2((u - 1.0) * 10.0) * (c * 0.5) + b,
        EaseInOutExpo => (2.0 - exp2((u - 1.0) * -10.0)) * (c * 0.5) + b,
        EaseInCirc => ((1.0 - t * t).sqrt() - 1.0) * -c + b,
        EaseOutCirc => {
            let v = t - 1.0;
            c * (1.0 - v * v).sqrt() + b
        }
        EaseInOutCirc if 1.0 > u => c * -0.5 * ((1.0 - u * u).sqrt() - 1.0) + b,
        EaseInOutCirc => {
            let w = u - 2.0;
            c * 0.5 * ((1.0 - w * w).sqrt() + 1.0) + b
        }
        // `+ 0.0` turns a -0 product into +0, as BDS's code does.
        EaseInBounce => c - (bounce(1.0 - t) * c + 0.0) + b,
        EaseOutBounce => c * bounce(t) + b,
        EaseInOutBounce if 0.5 > t => (c - (bounce(1.0 - u) * c + 0.0) + 0.0) * 0.5 + b,
        EaseInOutBounce => b + ((bounce(u - 1.0) * c + 0.0) * 0.5 + c * 0.5),
        EaseInBack => (t * BACK_C3 - BACK_C1) * (c * t * t) + b,
        EaseOutBack => {
            let v = t - 1.0;
            (v * v * (v * BACK_C3 + BACK_C1) + 1.0) * c + b
        }
        EaseInOutBack if 1.0 > u => (u * BACK_C2_PLUS_1 - BACK_C2) * (u * u) * (c * 0.5) + b,
        EaseInOutBack => {
            let w = u - 2.0;
            ((w * BACK_C2_PLUS_1 + BACK_C2) * (w * w) + 2.0) * (c * 0.5) + b
        }
        // The elastic easings special-case their ends; the far end is `b + c`, not `end`.
        EaseInElastic | EaseOutElastic | EaseInOutElastic if t == 0.0 => b,
        EaseInElastic | EaseOutElastic if t == 1.0 => b + c,
        EaseInOutElastic if u == 2.0 => b + c,
        EaseInElastic => {
            let v = t - 1.0;
            b - c * exp2(10.0 * v) * table_sin(elastic_angle(v))
        }
        EaseOutElastic => b + (exp2(-10.0 * t) * c * table_sin(elastic_angle(t)) + c),
        EaseInOutElastic => {
            let v = u - 1.0;
            let s = table_sin(elastic_angle(v));
            if 1.0 > u { b + exp2(v * 10.0) * c * s * -0.5 } else { b + (exp2(v * -10.0) * c * s * 0.5 + c) }
        }
        _ => unreachable!("{function:?} is not an easing"),
    }
}

/// `n1·y·y + k` with `y` the distance from the piece's centre; a NaN takes the last piece.
fn bounce(x: f32) -> f32 {
    const N1: f32 = 7.5625;
    let piece = |centre: f32, k: f32| {
        let y = x - centre;
        N1 * y * y + k
    };
    if BOUNCE_T[0] > x {
        N1 * x * x
    } else if BOUNCE_T[1] > x {
        piece(BOUNCE_C[0], 0.75)
    } else if BOUNCE_T[2] > x {
        piece(BOUNCE_C[1], 0.9375)
    } else {
        piece(BOUNCE_C[2], 0.984375)
    }
}

/// `((x - 0.075)·2π)/0.3`, the period 0.3 kept as a division.
fn elastic_angle(x: f32) -> f32 {
    (x - 0.075) * (2.0 * PI) / 0.3
}

fn exp2(x: f32) -> f32 {
    f64::from(x).exp2() as f32
}

/// The game's 65536-entry sine table: entry `i` is `sin(i / SIN_INDEX_SCALE)`.
fn table_entry(index: i32) -> f32 {
    (f64::from((index & 0xffff) as f32 / SIN_INDEX_SCALE)).sin() as f32
}

/// x86's float-to-int: NaN and anything out of range give `i32::MIN`, not Rust's saturation.
fn to_int(x: f32) -> i32 {
    if (-2_147_483_648.0..2_147_483_648.0).contains(&x) { x as i32 } else { i32::MIN }
}

fn table_sin(radians: f32) -> f32 {
    table_entry(to_int(radians * SIN_INDEX_SCALE))
}

/// A quarter turn on: the scaled angle plus 16384, rounded before truncation.
fn table_cos(radians: f32) -> f32 {
    table_entry(to_int(radians * SIN_INDEX_SCALE + 16384.0))
}
