//! `PositionMoveRotation.calculateAbsolute`: where a teleport puts an entity.

use crate::mth::{cos, sin};

/// Position, motion and rotation, as a teleport packet carries them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Placement {
    pub pos: [f64; 3],
    pub delta: [f64; 3],
    pub y_rot: f32,
    pub x_rot: f32,
}

/// `Relative` bits: X, Y, Z, Y_ROT, X_ROT, DELTA_X, DELTA_Y, DELTA_Z, ROTATE_DELTA.
const Y_ROT: u32 = 3;
const X_ROT: u32 = 4;
const DELTA: u32 = 5;
const ROTATE_DELTA: u32 = 8;

impl Placement {
    /// `change` applied to `self` with the packet's `relatives` bits.
    pub fn teleported(&self, change: &Placement, relatives: i32) -> Placement {
        let is = |bit: u32| relatives & (1 << bit) != 0;
        let pos = std::array::from_fn(|a| if is(a as u32) { self.pos[a] } else { 0.0 } + change.pos[a]);
        let y_rot = if is(Y_ROT) { self.y_rot } else { 0.0 } + change.y_rot;
        let x_rot = (if is(X_ROT) { self.x_rot } else { 0.0 } + change.x_rot).clamp(-90.0, 90.0);
        let mut current = self.delta;
        if is(ROTATE_DELTA) {
            current = x_rotated(current, f64::from(self.x_rot - x_rot).to_radians() as f32);
            current = y_rotated(current, f64::from(self.y_rot - y_rot).to_radians() as f32);
        }
        let delta = std::array::from_fn(|a| if is(DELTA + a as u32) { current[a] } else { 0.0 } + change.delta[a]);
        Placement { pos, delta, y_rot, x_rot }
    }
}

/// `Vec3.xRot`.
fn x_rotated(v: [f64; 3], radians: f32) -> [f64; 3] {
    let (c, s) = (cos(radians as f64) as f64, sin(radians as f64) as f64);
    [v[0], v[1] * c + v[2] * s, v[2] * c - v[1] * s]
}

/// `Vec3.yRot`.
fn y_rotated(v: [f64; 3], radians: f32) -> [f64; 3] {
    let (c, s) = (cos(radians as f64) as f64, sin(radians as f64) as f64);
    [v[0] * c + v[2] * s, v[1], v[2] * c - v[0] * s]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_bits_offset_from_the_source() {
        let start = Placement { pos: [10.0, 64.0, -3.0], y_rot: 90.0, x_rot: 80.0, ..Placement::default() };
        let change = Placement { pos: [1.0, 2.0, 3.0], y_rot: 5.0, x_rot: 20.0, ..Placement::default() };
        let to = |relatives| {
            let p = start.teleported(&change, relatives);
            (p.pos, p.y_rot, p.x_rot)
        };
        assert_eq!(to(0), ([1.0, 2.0, 3.0], 5.0, 20.0));
        assert_eq!(to(0b11001), ([11.0, 2.0, 3.0], 95.0, 90.0));
    }

    #[test]
    fn rotate_delta_turns_the_motion_with_the_look() {
        let source = Placement { delta: [0.0, 0.0, 1.0], y_rot: 0.0, ..Placement::default() };
        let change = Placement { y_rot: 90.0, ..Placement::default() };
        let all_delta = 1 << 5 | 1 << 6 | 1 << 7 | 1 << 8;
        let turned = source.teleported(&change, all_delta);
        assert_eq!(turned.y_rot, 90.0);
        assert!((turned.delta[0] + 1.0).abs() < 1.0E-6 && turned.delta[2].abs() < 1.0E-6, "{:?}", turned.delta);
        assert_eq!(source.teleported(&change, 0).delta, [0.0; 3]);
    }
}
