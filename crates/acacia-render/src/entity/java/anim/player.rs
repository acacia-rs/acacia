//! `PlayerModel.setupAnim` (the humanoid's, with the outer layers shown as the skin's parts say),
//! `PlayerCapeModel`'s cape and `ElytraModel`'s wings (26.3 client).

use std::f32::consts::PI;

use super::super::joml::{Quat, rotate_by};
use super::super::mth::pi_div;
use super::super::poses::Poses;
use super::super::state::RenderState;
use super::humanoid::{Humanoid, Model, humanoid};

const LAYERS: [(&str, &str); 6] = [
    ("hat", "showHat"),
    ("jacket", "showJacket"),
    ("left_pants", "showLeftPants"),
    ("right_pants", "showRightPants"),
    ("left_sleeve", "showLeftSleeve"),
    ("right_sleeve", "showRightSleeve"),
];

pub fn player(p: &mut Poses, s: &RenderState) {
    let body = !s.flag("isSpectator");
    for part in ["body", "right_arm", "left_arm", "right_leg", "left_leg"] {
        p.set_visible(part, body);
    }
    // AvatarRenderState shows every outer layer unless told otherwise.
    for (part, flag) in LAYERS {
        p.set_visible(part, !s.has(flag) || s.flag(flag));
    }
    humanoid(p, s, &Humanoid::of(s), Model::Humanoid);
}

/// `PlayerCapeModel.setupAnim`: the player's, then the cape turned by how it trails.
pub fn cape(p: &mut Poses, s: &RenderState) {
    player(p, s);
    let (lean, lean2) = (s.num("capeLean"), s.num("capeLean2"));
    let degrees = pi_div(180.0);
    let q = Quat::default()
        .rotate_y(-PI)
        .rotate_x((6.0 + lean / 2.0 + s.num("capeFlap")) * degrees)
        .rotate_z(lean2 / 2.0 * degrees)
        .rotate_y((180.0 - lean2 / 2.0) * degrees);
    rotate_by(p.get("cape"), q);
}

/// `ElytraModel.setupAnim`.
pub fn elytra(p: &mut Poses, s: &RenderState) {
    let y = if s.flag("isCrouching") { 3.0 } else { 0.0 };
    let (x_rot, y_rot, z_rot) = (s.num("elytraRotX"), s.num("elytraRotY"), s.num("elytraRotZ"));
    let left = p.get("left_wing");
    (left.y, left.x_rot, left.y_rot, left.z_rot) = (y, x_rot, y_rot, z_rot);
    let right = p.get("right_wing");
    (right.y, right.x_rot, right.y_rot, right.z_rot) = (y, x_rot, -y_rot, -z_rot);
}
