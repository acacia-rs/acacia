//! `PandaRenderer.setupRotations` after the living turn: rolling over, sitting up (shaking when
//! scared), lying on its back.

use glam::Vec3;

use super::Stack;
use crate::entity::java::mth::lerp;
use crate::entity::java::state::RenderState;

pub(super) fn rotations(s: &RenderState, m: &mut Stack) {
    let (baby, x_rot) = (s.baby(), s.x_rot());
    let roll = s.num("rollTime");
    if roll > 0.0 {
        let frac = roll - roll.floor();
        let pos = roll.floor() as i32;
        let next = pos + 1;
        let y = if baby { 0.3 } else { 0.8 };
        // getAngle: between this step's angle and the next until the quarter's last step.
        let angle = |this: f32, then: f32, threshold: f32| if (next as f32) < threshold { lerp(frac, this, then) } else { this };
        let (p, n) = (pos as f32, next as f32);
        let rolled = if p < 8.0 {
            let angle = angle(90.0 * p / 7.0, 90.0 * n / 7.0, 8.0);
            Some((angle, (y + 0.2) * (angle / 90.0)))
        } else if p < 16.0 {
            let angle = angle(90.0 + 90.0 * ((p - 8.0) / 7.0), 90.0 + 90.0 * (n - 8.0) / 7.0, 16.0);
            Some((angle, y + 0.2 + (y - 0.2) * (angle - 90.0) / 90.0))
        } else if p < 24.0 {
            let angle = angle(180.0 + 90.0 * ((p - 16.0) / 7.0), 180.0 + 90.0 * (n - 16.0) / 7.0, 24.0);
            Some((angle, y + y * (270.0 - angle) / 90.0))
        } else if pos < 32 {
            let angle = angle(270.0 + 90.0 * ((p - 24.0) / 7.0), 270.0 + 90.0 * (n - 24.0) / 7.0, 32.0);
            Some((angle, y * ((360.0 - angle) / 90.0)))
        } else {
            None
        };
        if let Some((angle, lift)) = rolled {
            m.shift(0.0, lift, 0.0);
            m.turn(Vec3::X, -angle);
        }
    }
    let sit = s.num("sitAmount");
    if sit > 0.0 {
        m.shift(0.0, 0.8 * sit, 0.0);
        m.turn(Vec3::X, lerp(sit, x_rot, x_rot + 90.0));
        m.shift(0.0, -1.0 * sit, 0.0);
        if s.flag("isScared") {
            let shake = (f64::from(s.age() * 1.25).cos() * std::f64::consts::PI * f64::from(0.05f32)) as f32;
            m.turn(Vec3::Y, shake);
            if baby {
                m.shift(0.0, 0.8, 0.55);
            }
        }
    }
    let lie = s.num("lieOnBackAmount");
    if lie > 0.0 {
        m.shift(0.0, if baby { 0.5 } else { 1.3 } * lie, 0.0);
        m.turn(Vec3::X, lerp(lie, x_rot, x_rot + 180.0));
    }
}
