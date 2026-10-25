//! `InventoryScreen.extractEntityInInventoryFollowsMouse`: an entity turned towards the mouse, handed
//! to the entity renderer as a picture-in-picture ([`Picture::Entity`]).

use crate::java::gui::{Gui, GuiDraw};
use crate::java::picture::Picture;

/// What the entity's render state holds that the call reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Figure {
    /// `minecraft:horse`.
    pub entity: String,
    /// `boundingBoxHeight` before the scale is divided out.
    pub bb_height: f32,
    /// `LivingEntityRenderState.scale`; `None` for an entity that is not living.
    pub scale: Option<f32>,
    /// `pose == FALL_FLYING`.
    pub fall_flying: bool,
}

type Quat = [f32; 4];

/// JOML `rotateX` of the identity.
fn rotate_x(angle: f32) -> Quat {
    let (s, c) = ((angle * 0.5).sin(), (angle * 0.5).cos());
    [s, 0.0, 0.0, c]
}

/// JOML `mul`.
fn mul(a: Quat, b: Quat) -> Quat {
    let [x, y, z, w] = a;
    [
        w * b[0] + x * b[3] + y * b[2] - z * b[1],
        w * b[1] - x * b[2] + y * b[3] + z * b[0],
        w * b[2] + x * b[1] - y * b[0] + z * b[3],
        w * b[3] - x * b[0] - y * b[1] - z * b[2],
    ]
}

/// The figure in `(x0, y0)-(x1, y1)` at `size`, its feet `offset_y` up, facing `mouse`.
#[allow(clippy::too_many_arguments)]
pub fn follows_mouse(gui: &mut Gui, rect: [i32; 4], size: i32, offset_y: f32, mouse: [f32; 2], figure: &Figure) {
    let [x0, y0, x1, y1] = rect;
    let cx = (x0 + x1) as f32 / 2.0;
    let cy = (y0 + y1) as f32 / 2.0;
    let x_angle = (((cx - mouse[0]) / 40.0) as f64).atan() as f32;
    let y_angle = (((cy - mouse[1]) / 40.0) as f64).atan() as f32;
    let pi = std::f32::consts::PI;
    let z = [0.0, 0.0, (pi * 0.5).sin(), (pi * 0.5).cos()];
    let x_rotation = rotate_x(y_angle * 20.0 * (std::f64::consts::PI / 180.0) as f32);
    let rotation = mul(z, x_rotation);
    let mut height = figure.bb_height;
    let angles = figure.scale.map(|scale| {
        height /= scale;
        let x_rot = if figure.fall_flying { 0.0 } else { -y_angle * 20.0 };
        [180.0 + x_angle * 20.0, x_angle * 20.0, x_rot]
    });
    gui.draws.push(GuiDraw::Picture(Picture::Entity {
        entity: figure.entity.clone(),
        scale: size as f32,
        translation: [0.0, height / 2.0 + offset_y, 0.0],
        rotation,
        camera: Some(x_rotation),
        angles,
        rect,
    }));
}
