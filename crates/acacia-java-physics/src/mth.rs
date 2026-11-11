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

pub fn ceil(v: f64) -> i32 {
    v.ceil() as i32
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

pub fn normalize(v: [f64; 3]) -> [f64; 3] {
    let length = length_sqr(v).sqrt();
    if length < 1.0E-5_f32 as f64 { [0.0; 3] } else { v.map(|c| c / length) }
}

/// `Mth.wrapDegrees`: into [-180, 180).
pub fn wrap_degrees(d: f32) -> f32 {
    let d = d % 360.0;
    if d >= 180.0 {
        d - 360.0
    } else if d < -180.0 {
        d + 360.0
    } else {
        d
    }
}

/// `Entity.calculateViewVector`.
pub fn look_vector(pitch: f32, yaw: f32) -> [f64; 3] {
    let x = pitch * DEG_TO_RAD;
    let y = -yaw * DEG_TO_RAD;
    let (y_cos, y_sin) = (cos(y as f64), sin(y as f64));
    let (x_cos, x_sin) = (cos(x as f64), sin(x as f64));
    [(y_sin * x_cos) as f64, -x_sin as f64, (y_cos * x_cos) as f64]
}
