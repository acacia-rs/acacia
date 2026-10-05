// Ported from Pomme (https://github.com/PommeMC/Client), pomme-client/src/world/block/model.rs.
// Copyright (C) 2026 Purdze. GPL-3.0-or-later; see ../../LICENSE-pomme.

//! The two turns a face's corners take: its element's rotation, then the blockstate's.

use glam::{Mat3, Vec3};

use super::ElementRotation;

pub(super) fn element(mut positions: [[f32; 3]; 4], rotation: Option<&ElementRotation>) -> [[f32; 3]; 4] {
    let Some(rotation) = rotation else { return positions };
    let mut turn = match rotation.axis.as_deref() {
        Some(axis) => about(axis, rotation.angle),
        // CuboidRotation.EulerXYZRotation: about x first, then y, then z.
        None => about("z", rotation.z) * about("y", rotation.y) * about("x", rotation.x),
    };
    if rotation.rescale && turn != Mat3::IDENTITY {
        // CuboidRotation.computeRescale: each axis stretches until its turned unit reaches a face of the block.
        turn *= Mat3::from_diagonal(Vec3::from_array([turn.x_axis, turn.y_axis, turn.z_axis].map(|axis| 1.0 / axis.abs().max_element())));
    }
    let origin = Vec3::from_array(rotation.origin);
    for p in &mut positions {
        *p = (origin + turn * (Vec3::from_array(*p) - origin)).to_array();
    }
    positions
}

/// A turn of `degrees` about an axis, with the sine and cosine JOML's `Matrix4f.rotation` takes:
/// the cosine comes from the sine, so at 45 degrees the two differ by a bit. Which side a
/// diagonal face is shaded as hangs on that bit.
fn about(axis: &str, degrees: f32) -> Mat3 {
    if degrees == 0.0 {
        return Mat3::IDENTITY;
    }
    let angle = degrees * (std::f64::consts::PI / 180.0) as f32;
    let sin = f64::from(angle).sin() as f32;
    let turns = (angle + std::f32::consts::FRAC_PI_2).rem_euclid(std::f32::consts::TAU);
    let cos = (1.0 - sin * sin).sqrt() * if turns >= std::f32::consts::PI { -1.0 } else { 1.0 };
    match axis {
        "x" => Mat3::from_cols_array(&[1.0, 0.0, 0.0, 0.0, cos, sin, 0.0, -sin, cos]),
        "y" => Mat3::from_cols_array(&[cos, 0.0, -sin, 0.0, 1.0, 0.0, sin, 0.0, cos]),
        "z" => Mat3::from_cols_array(&[cos, sin, 0.0, -sin, cos, 0.0, 0.0, 0.0, 1.0]),
        _ => Mat3::IDENTITY,
    }
}

/// Exact for quarter turns, which is all a blockstate has.
fn quarter(degrees: u16) -> (f32, f32) {
    [(0.0, 1.0), (1.0, 0.0), (0.0, -1.0), (-1.0, 0.0)][usize::from(degrees / 90 % 4)]
}

/// A blockstate's `x` then `y` rotation about the block's centre.
pub(super) fn model(mut p: [f32; 3], x: u16, y: u16) -> [f32; 3] {
    const CENTRE: f32 = 8.0;
    let ((sin_x, cos_x), (sin_y, cos_y)) = (quarter(x), quarter(y));
    let (dy, dz) = (p[1] - CENTRE, p[2] - CENTRE);
    (p[1], p[2]) = (CENTRE + cos_x * dy + sin_x * dz, CENTRE - sin_x * dy + cos_x * dz);
    let (dx, dz) = (p[0] - CENTRE, p[2] - CENTRE);
    (p[0], p[2]) = (CENTRE + cos_y * dx - sin_y * dz, CENTRE + sin_y * dx + cos_y * dz);
    p
}
