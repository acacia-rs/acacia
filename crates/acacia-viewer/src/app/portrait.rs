//! The entities of the GUI's pictures-in-picture (the player in the inventory, a horse in its
//! screen), placed in window pixels as Java's `PictureInPictureRenderer` and `GuiEntityRenderer`
//! pose them; the renderer draws them into its pictures texture (`acacia-render` `gpu/over.rs`).

use acacia_render::entity::EntityInstance;
use acacia_ui::java::picture::Picture;
use glam::{DVec3, Mat4, Quat, Vec3};

use crate::smooth::Smoother;
use crate::ui::PictureRequest;

pub(super) fn instances(entities: &Smoother, requests: &[PictureRequest], camera: DVec3) -> Vec<EntityInstance> {
    requests.iter().flat_map(|r| instance(entities, r, camera)).collect()
}

fn instance(entities: &Smoother, r: &PictureRequest, camera: DVec3) -> Vec<EntityInstance> {
    let Picture::Entity { entity, scale, translation, rotation, angles, .. } = &r.picture else { return Vec::new() };
    let [x0, y0, x1, y1] = r.window;
    // prepare: the texture's centre (an entity's getTranslateY is half the height), then
    // scale(s, s, -s) with s the GUI scale times the state's; renderToTexture: the translation
    // and the rotation.
    let k = r.gui_scale * scale;
    let pip = Mat4::from_translation(Vec3::new((x0 + x1) / 2.0, (y0 + y1) / 2.0, 0.0))
        * Mat4::from_scale(Vec3::new(k, k, -k))
        * Mat4::from_translation(Vec3::from(*translation))
        * Mat4::from_quat(Quat::from_array(*rotation));
    // A living entity's render state turns the body by 180 - bodyRot and the head by yRot, xRot.
    let [body, head, pitch] = angles.unwrap_or([180.0, 0.0, 0.0]);
    // See acacia-ui README "Java GUI", pictures: Java's model space against the mesh's.
    let place = pip * Mat4::from_rotation_y((180.0 - body).to_radians()) * Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
    entities.figure(entity != "minecraft:player", place, head, pitch, camera)
}
