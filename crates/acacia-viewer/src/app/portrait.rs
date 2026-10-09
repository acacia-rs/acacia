//! The player's figure in the inventory screen, as Java's `InventoryScreen`
//! (`renderEntityInInventoryFollowsMouse`) places it: centred in the panel's box, 30 GUI pixels a
//! block, turning its body and head after the mouse. Drawn over the UI by the renderer.

use acacia_render::entity::EntityInstance;
use acacia_ui::inventory::{self, Layout};
use glam::{Mat3, Mat4, Vec3};

use super::App;

/// The figure's box in the panel (GUI pixels from its corner), and its size on screen.
const BOX: [f32; 4] = [26.0, 8.0, 75.0, 78.0];
const PIXELS_PER_BLOCK: f32 = 30.0;
/// Half the player's height plus Java's offset: the box's centre is this far above the feet.
const CENTRE_HEIGHT: f32 = 0.9 + 0.0625;

impl App {
    pub(super) fn portrait(&self) -> Vec<EntityInstance> {
        if !self.screen_open || self.menu.is_some() || self.layout() != Layout::Player {
            return Vec::new();
        }
        let ([w, h], scale) = self.gui();
        let (w, h, scale) = (w as f32, h as f32, scale as f32);
        let [ox, oy] = inventory::origin(Layout::Player, [w / scale, h / scale]);
        let centre = [ox + (BOX[0] + BOX[2]) / 2.0, oy + (BOX[1] + BOX[3]) / 2.0];
        let mouse = self.gui_mouse();
        let (turn, tilt) = (((centre[0] - mouse[0]) / 40.0).atan(), ((centre[1] - mouse[1]) / 40.0).atan());

        // The depth at which a block spans the wanted pixels, and the box's centre there.
        let per_block = PIXELS_PER_BLOCK * scale;
        let depth = h / 2.0 / (self.camera.fov_y / 2.0).tan() / per_block;
        let at = Vec3::new((centre[0] * scale - w / 2.0) / per_block, -(centre[1] * scale - h / 2.0) / per_block, -depth);
        // Java's GUI space has y down and z towards the viewer; its pose is a half turn about z,
        // the tilt about x, then the body's turn. `mirror_x` is Java's model flip in Bedrock's space.
        let (flip_y, mirror_x) = (Mat4::from_scale(Vec3::new(1.0, -1.0, -1.0)), Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0)));
        let pose = Mat4::from_translation(Vec3::Y * CENTRE_HEIGHT)
            * Mat4::from_rotation_z(std::f32::consts::PI)
            * Mat4::from_rotation_x(tilt * 20f32.to_radians())
            * Mat4::from_rotation_y(-turn * 20f32.to_radians());
        let (right, forward) = (self.camera.right(), self.camera.forward());
        let view = Mat4::from_mat3(Mat3::from_cols(right, right.cross(forward), -forward));
        let place = view * self.camera.bob.inverse() * Mat4::from_translation(at) * flip_y * pose * mirror_x;
        // The head turns as far again as the body (Java: 40 against 20 times the atan, in degrees).
        self.entities.portrait(place, turn * 20.0, -tilt * 20.0, self.camera.position)
    }
}
