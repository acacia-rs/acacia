//! `Mth.sin` / `Mth.cos`: Java's 65536-entry float sine table, indexed through a double-to-long
//! cast, as every model's `setupAnim` calls them (after Horizon's `geometry/trig.rs`, MIT).

use std::sync::OnceLock;

const SCALE: f64 = 10430.378350470453;

fn table() -> &'static [f32] {
    static SINE: OnceLock<Box<[f32]>> = OnceLock::new();
    SINE.get_or_init(|| (0..65536).map(|i| (f64::from(i) / SCALE).sin() as f32).collect())
}

pub fn sin(angle: f32) -> f32 {
    table()[((f64::from(angle) * SCALE) as i64 & 65535) as usize]
}

pub fn cos(angle: f32) -> f32 {
    table()[((f64::from(angle) * SCALE + 16384.0) as i64 & 65535) as usize]
}

/// `Mth.rotLerpRad`.
pub fn rot_lerp_rad(amount: f32, from: f32, to: f32) -> f32 {
    use std::f32::consts::PI;
    let mut diff = to - from;
    while diff < -PI {
        diff += PI * 2.0;
    }
    while diff >= PI {
        diff -= PI * 2.0;
    }
    from + amount * diff
}

/// Java's `(float) (Math.PI / d)`: divided in double, then rounded once.
pub fn pi_div(d: f64) -> f32 {
    (std::f64::consts::PI / d) as f32
}

pub fn lerp(amount: f32, from: f32, to: f32) -> f32 {
    from + amount * (to - from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn table_lookup_quantises_and_wraps() {
        assert_eq!(sin(0.0), 0.0);
        assert_eq!(cos(0.0), 1.0);
        assert!((sin(PI / 2.0) - 1.0).abs() < 1e-6);
        // Negative angles wrap through the mask, not a sign flip of the lookup.
        assert_eq!(sin(-PI / 2.0), table()[49152]);
    }

    #[test]
    fn rot_lerp_takes_the_short_way() {
        assert!((rot_lerp_rad(0.5, 3.0, -3.0) - (3.0 + (2.0 * PI - 6.0) / 2.0)).abs() < 1e-5);
    }
}
