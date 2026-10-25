//! Java's `Mth.sin` and `Mth.cos`: a 65536-entry table filled in double precision (Bedrock's,
//! in acacia-physics, fills it with a float sine and differs by an ulp).

use std::sync::OnceLock;

const STEPS_PER_RADIAN: f64 = 10430.378350470453;

fn table() -> &'static [f32] {
    static SIN: OnceLock<Box<[f32]>> = OnceLock::new();
    SIN.get_or_init(|| (0..65536).map(|i| (f64::from(i) / STEPS_PER_RADIAN).sin() as f32).collect())
}

pub fn sin(v: f64) -> f32 {
    table()[((v * STEPS_PER_RADIAN) as i64 & 65535) as usize]
}

pub fn cos(v: f64) -> f32 {
    table()[((v * STEPS_PER_RADIAN + 16384.0) as i64 & 65535) as usize]
}

#[test]
fn quarter_turns_are_exact() {
    assert_eq!((sin(0.0), cos(0.0), sin(std::f64::consts::FRAC_PI_2)), (0.0, 1.0, 1.0));
    assert_eq!(sin(f64::from(std::f32::consts::TAU)), sin(0.0));
}
