//! What an item frame on a wall shows: its item at half size on the plate, or a map's picture
//! over the whole face of the block (Java's `ItemFrameRenderer`). `turn` is the block entity's
//! `ItemRotation` in degrees.

use std::sync::Arc;

use glam::{DVec3, IVec3, Mat4, Vec3};

use super::{Form, ItemModel};
use crate::entity::{EntityInstance, Layer, NO_MODEL, NO_TEXTURE, Pose, Skin};
use crate::glint::Glint;

/// Where the item's middle sits before the wall, in the frame's model space (the wall is at
/// +0.5): just off the plate.
const ITEM_DEPTH: f32 = 6.5 / 16.0;
/// A map lies over the border.
const MAP_DEPTH: f32 = 6.9 / 16.0;
/// Java halves the item; its `fixed` display halves a block again.
const ITEM_SCALE: f32 = 0.5;

/// The frame's model space (origin at the block's bottom centre, facing -z) to the camera's
/// world, as the entity pass places a block model.
fn model_space(block: IVec3, yaw: f32, camera: DVec3) -> Mat4 {
    let foot = block.as_dvec3() + DVec3::new(0.5, 0.0, 0.5) - camera;
    Mat4::from_translation(foot.as_vec3()) * Mat4::from_rotation_y(-yaw.to_radians()) * Mat4::from_scale(Vec3::new(1.0, 1.0, -1.0))
}

/// Tips a wall frame's model space back about the block's middle: 90° lays it on the floor
/// facing up, -90° hangs it under the ceiling.
pub fn tilted(degrees: f32) -> Mat4 {
    let middle = Vec3::new(0.0, 0.5, 0.0);
    Mat4::from_translation(middle) * Mat4::from_rotation_x(degrees.to_radians()) * Mat4::from_translation(-middle)
}

/// Where the frame hangs: its model's yaw and [`tilted`]'s degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hung {
    pub yaw: f32,
    pub tilt: f32,
}

fn placed(block: IVec3, hung: Hung, turn: f32, depth: f32, scale: f32, camera: DVec3) -> Mat4 {
    // TODO: which way `ItemRotation` turns, and a floor frame's upright, were not checked against the game.
    model_space(block, hung.yaw, camera) * tilted(hung.tilt) * Mat4::from_translation(Vec3::new(0.0, 0.5, depth)) * Mat4::from_rotation_z(turn.to_radians()) * Mat4::from_scale(Vec3::splat(scale))
}

fn instance(layers: Arc<[Layer]>, skin: Arc<Skin>, block: IVec3, frame: Mat4, glint: Option<Glint>) -> EntityInstance {
    // The entity pass reads the light half a block over this: the frame's own cell.
    EntityInstance { layers, skin: Some(skin), position: block.as_dvec3() + DVec3::new(0.5, 0.0, 0.5), yaw: 0.0, scale: 1.0, pose: Pose::default(), frame: Some(frame), hurt: false, glint }
}

pub fn item(model: &ItemModel, block: IVec3, hung: Hung, turn: f32, camera: DVec3) -> EntityInstance {
    let scale = if model.form == Form::Block { ITEM_SCALE * 0.5 } else { ITEM_SCALE };
    let frame = placed(block, hung, turn, ITEM_DEPTH, scale, camera);
    instance(model.layers.clone(), model.skin.clone(), block, frame, model.glint.then_some(Glint::Item))
}

/// `picture` is [`super::map::picture`]'s skin.
pub fn map_picture(picture: &Arc<Skin>, block: IVec3, hung: Hung, turn: f32, camera: DVec3) -> EntityInstance {
    instance([Layer::plain(NO_MODEL, NO_TEXTURE)].into(), picture.clone(), block, placed(block, hung, turn, MAP_DEPTH, 1.0, camera), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_facing_south_shows_its_item_upright_before_the_plate() {
        // A wall frame at yaw 0 hangs on the block to its north and faces south (+z).
        let frame = placed(IVec3::new(10, 64, -3), Hung { yaw: 0.0, tilt: 0.0 }, 0.0, ITEM_DEPTH, ITEM_SCALE, DVec3::ZERO);
        let at = |x: f32, y: f32| frame.transform_point3(Vec3::new(x, y, 0.0));
        let (middle, top_right) = (at(0.0, 0.0), at(0.5, 0.5));
        assert!((middle - Vec3::new(10.5, 64.5, -2.5 - ITEM_DEPTH)).abs().max_element() < 1e-5, "{middle}");
        // Seen from the south (looking north), east is to the right: the image is not mirrored.
        assert!((top_right - middle - Vec3::new(0.25, 0.25, 0.0)).abs().max_element() < 1e-5, "{top_right}");
        // On a floor the item lies just over the block below; under a ceiling, just under the one above.
        let lying = |tilt| placed(IVec3::ZERO, Hung { yaw: 0.0, tilt }, 0.0, ITEM_DEPTH, ITEM_SCALE, DVec3::ZERO).transform_point3(Vec3::ZERO);
        assert!((lying(90.0) - Vec3::new(0.5, 0.5 - ITEM_DEPTH, 0.5)).abs().max_element() < 1e-5, "{}", lying(90.0));
        assert!((lying(-90.0) - Vec3::new(0.5, 0.5 + ITEM_DEPTH, 0.5)).abs().max_element() < 1e-5, "{}", lying(-90.0));
    }
}
