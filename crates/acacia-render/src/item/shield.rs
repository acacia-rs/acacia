//! The shield: a model, not a sprite. Java hard-codes it (`ShieldModel`: a 12×22×1 plate with a
//! 2×6×6 handle) and the Bedrock pack has the same boxes as `geometry.shield`; both looks draw the
//! pack's `textures/entity/shield`. Displays: Java's `item/shield.json` and `shield_blocking.json`
//! (26.3). See README "Items".

use std::path::Path;

use glam::{Mat4, Vec3};
use image::RgbaImage;

use super::hand::Display;
use crate::assets::{image_file, json};
use crate::banner::{self, Banner};
use crate::entity::bake::{self, Mesh};
use crate::entity::{Pose, Skin, geometry};

pub const ITEM: &str = "minecraft:shield";
const TEXTURE: &str = "textures/entity/shield";
const GEOMETRY: &str = "geometry.shield";
/// `ShieldModel` in the pack's layout, for packs without `models/entity/shield.geo.json`.
const BUILT_IN: &str = r#"{"format_version": "1.12.0", "minecraft:geometry": [{
    "description": {"identifier": "geometry.shield", "texture_width": 64, "texture_height": 64},
    "bones": [{"name": "shield", "cubes": [
        {"origin": [0, 25, 0], "size": [2, 6, 6], "uv": [26, 0]},
        {"origin": [-5, 17, -1], "size": [12, 22, 1], "uv": [0, 0]}]}]}]}"#;
/// Where the pack's model has the origin of `ShieldModel` (the plate's middle, a pixel behind
/// its back), in 1/16 block.
const PACK_ORIGIN: Vec3 = Vec3::new(1.0, 28.0, 1.0);
/// `ground`: scale, then translation in blocks.
pub(super) const GROUND: (f32, Vec3) = (0.25, Vec3::new(0.125, 0.25, 0.125));
/// The plate's bottom edge in item mesh space.
pub(super) const LOWEST: f32 = -0.5 - 11.0 / 16.0;

/// The model's texture, with the banner a shield was crafted with.
pub(super) fn sheet(root: &Path, cloth: Option<&Banner>) -> Option<RgbaImage> {
    let file = image_file(root, TEXTURE)?;
    let image = image::open(&file).inspect_err(|e| tracing::warn!(?file, %e, "shield texture")).ok()?.to_rgba8();
    Some(match cloth {
        Some(cloth) => banner::onto_shield(root, image, cloth),
        None => image,
    })
}

pub(super) fn skin(root: &Path, cloth: Option<&Banner>) -> Option<Skin> {
    let image = sheet(root, cloth)?;
    let pack = json::read(&root.join("models/entity/shield.geo.json")).ok().and_then(|file| geometry::parse(&file.to_string()).remove(GEOMETRY));
    let mesh = mesh(&pack.or_else(|| geometry::parse(BUILT_IN).remove(GEOMETRY))?);
    Some(Skin { width: image.width(), height: image.height(), rgba: image.into_raw(), mesh: Some(mesh) })
}

/// The model in item mesh space: Java draws it through `scale(1, -1, -1)` from the corner of the
/// item's unit box, and item meshes are that box centred with z mirrored. The pack's geometry is
/// Java's with y mirrored, so a shift does it.
fn mesh(geometry: &geometry::Geometry) -> Mesh {
    bake::bake(geometry).fixed(Mat4::from_translation(Vec3::new(-0.5, -0.5, 0.5) - PACK_ORIGIN / 16.0))
}

/// Raises an arm as Java's `HumanoidModel.poseBlockingArm` does (0.9424778 rad up, π/6 across):
/// the blocking displays are placed for that arm, not for the pack's blocking animation.
pub fn raise_arm(pose: &mut Pose, left: bool) {
    let (bone, across) = if left { ("leftarm", 30.0) } else { ("rightarm", -30.0) };
    pose.turn(bone, [-54.0, across, 0.0]);
}

/// The shield models' `display` for a hand, as the files list it (the left one not yet mirrored).
pub(super) fn display(first_person: bool, left: bool, blocking: bool) -> Display {
    match (first_person, left, blocking) {
        (false, false, false) => Display::px([0.0, 90.0, 0.0], [10.0, 6.0, -4.0], 1.0),
        (false, true, false) => Display::px([0.0, 90.0, 0.0], [10.0, 6.0, 12.0], 1.0),
        (false, false, true) => Display::px([45.0, 155.0, 0.0], [-3.49, 11.0, -2.0], 1.0),
        (false, true, true) => Display::px([45.0, 155.0, 0.0], [11.51, 7.0, 2.5], 1.0),
        (true, false, false) => Display::px([0.0, 180.0, 5.0], [-10.0, 1.75, -10.0], 1.25),
        (true, true, false) => Display::px([0.0, 180.0, 5.0], [10.0, 0.0, -10.0], 1.25),
        (true, false, true) => Display::px([0.0, 180.0, -5.0], [-15.0, 3.25, -11.0], 1.25),
        (true, true, true) => Display::px([0.0, 180.0, -5.0], [5.0, 5.0, -11.0], 1.25),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::Form;
    use crate::item::hand::held_frame;

    fn bounds(mesh: &Mesh, frame: Mat4) -> (Vec3, Vec3) {
        let points = mesh.vertices.iter().map(|v| frame.transform_point3(Vec3::from(v.position)));
        points.fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)))
    }

    #[test]
    fn the_built_in_model_is_java_s_shield_in_item_space() {
        let mesh = mesh(&geometry::parse(BUILT_IN)[GEOMETRY]);
        assert_eq!(mesh.vertices.len(), 72);
        // Java's item space, z mirrored: the plate 12×22×1 around (-0.5, -0.5), the handle 6 deep behind it.
        let (lo, hi) = bounds(&mesh, Mat4::IDENTITY);
        assert!((lo * 16.0 - Vec3::new(-14.0, -19.0, 6.0)).abs().max_element() < 1e-3, "{lo}");
        assert!((hi * 16.0 - Vec3::new(-2.0, 3.0, 13.0)).abs().max_element() < 1e-3, "{hi}");
        assert!((lo.y - LOWEST).abs() < 1e-6);
        // The plate's front (the pack's -z face) takes the texture from (1, 1): 12×22 of 64×64.
        let front: Vec<[f32; 2]> = mesh.vertices[36..].iter().filter(|v| v.normal[2] < -0.5).map(|v| v.uv).collect();
        assert_eq!((front[0], front[2]), ([1.0 / 64.0, 1.0 / 64.0], [13.0 / 64.0, 23.0 / 64.0]));
    }

    /// Needs assets/vanilla (tools/fetch-vanilla-pack.sh); skips when it is missing.
    #[test]
    fn the_pack_s_shield_is_the_built_in_one() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/vanilla");
        let Some(skin) = skin(&dir, None) else { return eprintln!("skipped: no shield in {}", dir.display()) };
        let built_in = mesh(&geometry::parse(BUILT_IN)[GEOMETRY]);
        assert_eq!((skin.width, skin.height), (64, 64));
        assert_eq!(bounds(&skin.mesh.unwrap(), Mat4::IDENTITY), bounds(&built_in, Mat4::IDENTITY));
    }

    #[test]
    fn a_resting_arm_carries_the_shield_on_its_outside() {
        let mesh = mesh(&geometry::parse(BUILT_IN)[GEOMETRY]);
        let plate = Mesh { vertices: mesh.vertices[36..].to_vec(), ..mesh.clone() };
        for (left, side) in [(false, -1.0), (true, 1.0)] {
            // The shoulder of a humanoid's arm, in model space (blocks): the entity's right is -x.
            let hand = Mat4::from_translation(Vec3::new(5.0 * side, 22.0, 0.0) / 16.0);
            // Raised, the plate turns across the body and tips: no longer flat to the side.
            let (lo, hi) = bounds(&plate, held_frame(Display::third_person(Form::Shield, left, true), hand, left));
            assert!((hi.x - lo.x) * 16.0 > 6.0, "{left}: {lo} {hi}");
            let frame = held_frame(Display::third_person(Form::Shield, left, false), hand, left);
            // Facing sideways with its long side level (as a sword points forward out of the
            // fist), 4.5 px out from the shoulder and centred 6 px below it.
            let (lo, hi) = bounds(&plate, frame);
            let (size, middle) = ((hi - lo) * 16.0, (lo + hi) * 8.0);
            assert!((size - Vec3::new(1.0, 12.0, 22.0)).abs().max_element() < 1e-3, "{left}: {size}");
            assert!((middle - Vec3::new(9.5 * side, 16.0, 0.0)).abs().max_element() < 1e-3, "{left}: {middle}");
            // The handle reaches from the plate back into the fist.
            let (lo, hi) = bounds(&mesh, frame);
            assert!(((lo.x + hi.x) * 8.0 - 6.5 * side).abs() < 1e-3, "{left}: {lo} {hi}");
        }
    }

    #[test]
    fn a_raised_shield_moves_towards_the_middle_of_the_view() {
        // The plate's middle in Java's item space, from the hand's resting place.
        let centre = |left, blocking| Display::first_person(ITEM, Form::Shield, left, blocking).frame().transform_point3(Vec3::new(-0.5, -0.5, -0.5 + 1.5 / 16.0));
        let (rest, raised) = (centre(false, false), centre(false, true));
        assert!(raised.x < rest.x - 0.15 && raised.y > rest.y + 0.15, "{rest} {raised}");
        // The left hand's entry, mirrored by `ItemTransform.apply`, lands as far on its other side.
        let left = centre(true, false);
        assert!((left.x + rest.x).abs() < 0.01 && (left.y - rest.y).abs() < 0.01 && (left.z - rest.z).abs() < 1e-4, "{rest} {left}");
    }
}
