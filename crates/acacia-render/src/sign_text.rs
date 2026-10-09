//! The text on signs: what the caller lays out per sign (`acacia_ui::signs`) and where each sign
//! model carries it. See README "Sign text".

use std::collections::HashMap;
use std::sync::Arc;

use acacia_ui::signs::Laid;
use glam::{IVec3, Mat4, Vec3};

use crate::blocks::model::{BlockModel, HANGING_SIGNS, SIGN, WALL_SIGN};

/// A sign's front and back text.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SignText {
    pub front: Laid,
    pub back: Laid,
}

/// Sign text by block position.
pub type SignTextMap = HashMap<[i32; 3], Arc<SignText>>;

/// Java's `BlockEntityRenderer.getViewDistance`: no block entity is drawn farther away.
pub const REACH: f64 = 64.0;
/// Java's `OUTLINE_RENDER_DISTANCE`: a glowing text's outline shows only this near, unless the
/// text is black.
pub const OUTLINE_REACH: f64 = 16.0;

/// A sign's text in the world.
#[derive(Debug, Clone)]
pub struct Placed {
    pub block: IVec3,
    /// Front and back: font pixels (y down) to blocks from the block's corner.
    pub faces: [Mat4; 2],
    pub text: Arc<SignText>,
}

/// Java's `StandingSignRenderer` and `HangingSignRenderer.textTransformation`; `None` for a model
/// that is no sign.
pub fn transforms(model: &BlockModel) -> Option<[Mat4; 2]> {
    // Pivot, the shift of a sign on a wall or under its chains, the text's offset and its scale.
    let (pivot, shift, offset, scale) = match model.geometry {
        SIGN => (0.5, Vec3::ZERO, Vec3::new(0.0, 0.333_333_34, 0.046_666_667), 0.010_416_667),
        WALL_SIGN => (0.5, Vec3::new(0.0, -0.3125, -0.4375), Vec3::new(0.0, 0.333_333_34, 0.046_666_667), 0.010_416_667),
        g if HANGING_SIGNS.contains(&g) => (0.9375, Vec3::new(0.0, -0.3125, 0.0), Vec3::new(0.0, -0.32, 0.073), 0.014_062_5),
        _ => return None,
    };
    let sign = Mat4::from_translation(Vec3::new(0.5, pivot, 0.5)) * Mat4::from_rotation_y(-model.yaw.to_radians()) * Mat4::from_translation(shift);
    let text = Mat4::from_translation(offset) * Mat4::from_scale(Vec3::new(scale, -scale, scale));
    Some([sign * text, sign * Mat4::from_rotation_y(std::f32::consts::PI) * text])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::model::Kind;

    fn sign(geometry: &'static str, yaw: f32) -> BlockModel {
        BlockModel { kind: Kind::Sign, geometry, texture: String::new(), yaw }
    }

    fn at(m: Mat4, x: f32, y: f32) -> Vec3 {
        m.transform_point3(Vec3::new(x, y, 0.0))
    }

    #[test]
    fn text_sits_just_off_the_board_on_both_sides() {
        let [front, back] = transforms(&sign(SIGN, 0.0)).unwrap();
        // A standing sign's board is 1/12 block thick around the block's middle, 7/12 to 13/12 up.
        assert!((at(front, 0.0, 0.0) - Vec3::new(0.5, 0.8333, 0.5467)).abs().max_element() < 1e-3);
        assert!((at(back, 0.0, 0.0) - Vec3::new(0.5, 0.8333, 0.4533)).abs().max_element() < 1e-3);
        // Text runs to the reader's right and down: seen from the south, +x.
        assert!(at(front, 48.0, 0.0).x > 0.99 && at(back, 48.0, 0.0).x < 0.01);
        assert!((at(front, 0.0, 10.0).y - (0.8333 - 10.0 / 96.0)).abs() < 1e-3);
    }

    #[test]
    fn turned_wall_and_hanging_signs() {
        // Yaw 90 faces west.
        let [west, _] = transforms(&sign(SIGN, 90.0)).unwrap();
        assert!((at(west, 0.0, 0.0) - Vec3::new(0.4533, 0.8333, 0.5)).abs().max_element() < 1e-3);
        // A wall sign facing south hangs on the block to the north.
        let [wall, _] = transforms(&sign(WALL_SIGN, 0.0)).unwrap();
        assert!((at(wall, 0.0, 0.0) - Vec3::new(0.5, 0.5208, 0.1092)).abs().max_element() < 1e-3);
        let [hanging, _] = transforms(&sign(HANGING_SIGNS[0], 0.0)).unwrap();
        assert!((at(hanging, 0.0, 0.0) - Vec3::new(0.5, 0.305, 0.573)).abs().max_element() < 1e-3);
        assert!(transforms(&BlockModel { kind: Kind::Chest, geometry: crate::blocks::model::CHEST, texture: String::new(), yaw: 0.0 }).is_none());
    }
}
