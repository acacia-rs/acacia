use std::sync::OnceLock;

const SIN_SCALE: f64 = 10430.378350470453;
pub const DEG_TO_RAD: f32 = std::f64::consts::PI as f32 / 180.0;

fn table() -> &'static [f32] {
    static TABLE: OnceLock<Vec<f32>> = OnceLock::new();
    TABLE.get_or_init(|| (0..65536).map(|i| (i as f64 / SIN_SCALE).sin() as f32).collect())
}

pub fn sin(radians: f64) -> f32 {
    table()[((radians * SIN_SCALE) as i64 & 65535) as usize]
}

pub fn cos(radians: f64) -> f32 {
    table()[((radians * SIN_SCALE + 16384.0) as i64 & 65535) as usize]
}

pub fn floor(v: f64) -> i32 {
    v.floor() as i32
}

pub fn sqrt(v: f32) -> f32 {
    (v as f64).sqrt() as f32
}

pub fn length_sqr(v: [f64; 3]) -> f64 {
    v[0] * v[0] + v[1] * v[1] + v[2] * v[2]
}

pub fn horizontal_sqr(v: [f64; 3]) -> f64 {
    v[0] * v[0] + v[2] * v[2]
}
