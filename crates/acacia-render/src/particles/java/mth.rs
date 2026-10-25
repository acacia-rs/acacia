//! `Mth`'s table sine and cosine, as the client's particle code reads them.

use std::sync::OnceLock;

const SCALE: f64 = 10430.378350470453;

/// The 65536-entry float table at `index`, wrapped as Java casts it to long.
fn table(index: f64) -> f32 {
    static TABLE: OnceLock<Box<[f32]>> = OnceLock::new();
    let table = TABLE.get_or_init(|| (0..65536).map(|i| (f64::from(i) / SCALE).sin() as f32).collect());
    table[(index as i64 & 65535) as usize]
}

/// `Mth.sin`.
pub fn mth_sin(angle: f64) -> f32 {
    table(angle * SCALE)
}

/// `Mth.cos`.
pub fn mth_cos(angle: f64) -> f32 {
    table(angle * SCALE + 16384.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sine_table_wraps_like_javas() {
        assert_eq!(mth_sin(0.0), 0.0);
        assert_eq!(mth_cos(0.0), 1.0);
        assert!((mth_sin(std::f64::consts::FRAC_PI_2) - 1.0).abs() < 1e-6);
        assert_eq!(mth_sin(std::f64::consts::TAU + 0.5), mth_sin(0.5));
    }
}
