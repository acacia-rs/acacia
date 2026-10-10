//! A player's cape: the game's `geometry.cape` (a 10×16×1 sheet hung from the neck, behind the
//! body) in the cape image the skin brings, following the body's bones. It hangs at Java's rest
//! angle (`CapeLayer`: 6°); the swing a moving wearer gives it is not modelled.

use std::sync::{Arc, LazyLock};

use super::{Layer, Mesh, NO_MODEL, NO_TEXTURE, Pose, Skin, bake, geometry};

const GEOMETRY: &str = r#"{"format_version": "1.12.0", "minecraft:geometry": [{
    "description": {"identifier": "geometry.cape", "texture_width": 64, "texture_height": 32},
    "bones": [
        {"name": "waist", "pivot": [0, 12, 0]},
        {"name": "body", "parent": "waist", "pivot": [0, 24, 0]},
        {"name": "cape", "parent": "body", "pivot": [0, 24, 3], "rotation": [0, 180, 0],
         "cubes": [{"origin": [-5, 8, 3], "size": [10, 16, 1], "uv": [0, 0]}]}]}]}"#;
const BONE: &str = "cape";
/// Degrees the sheet's foot stands off the back.
const REST: f32 = 6.0;

/// `None` for an image that is not the cape sheet's 2:1.
pub fn skin(width: u32, height: u32, rgba: Vec<u8>) -> Option<Skin> {
    (width == height * 2 && height > 0).then(|| Skin { width, height, rgba, mesh: Some(mesh()) })
}

/// What a cape's instance draws: its skin's own mesh and image.
pub fn layers() -> Arc<[Layer]> {
    static LAYERS: LazyLock<Arc<[Layer]>> = LazyLock::new(|| [Layer::plain(NO_MODEL, NO_TEXTURE)].into());
    LAYERS.clone()
}

fn mesh() -> Mesh {
    bake::bake(&geometry::parse(GEOMETRY)["geometry.cape"])
}

/// The cape's pose under a wearer posed as `wearer`.
pub fn pose(wearer: &Pose) -> Pose {
    let mut pose = Pose(["waist", "body"].into_iter().filter_map(|bone| wearer.get(bone).cloned()).collect());
    // The pack's sneaking pose turns both legs back under a leaning body: the sheet clears them.
    let leg = |bone| wearer.get(bone).map_or(0.0, |leg| leg.rotation[0]);
    let (left, right) = (leg("leftleg"), leg("rightleg"));
    let bent = if left * right > 0.0 { left.abs().min(right.abs()) } else { 0.0 };
    pose.turn(BONE, [-REST - bent, 0.0, 0.0]);
    pose
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::*;

    #[test]
    fn the_sheet_hangs_behind_the_back_and_stands_off_at_its_foot() {
        let mesh = mesh();
        let cape = mesh.bones.iter().position(|bone| bone == BONE).unwrap();
        let (lo, hi) = mesh.vertices.iter().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), v| (lo.min(v.position.into()), hi.max(v.position.into())));
        // The wearer looks down -z: its back is at z = 2 px.
        assert!((lo * 16.0 - Vec3::new(-5.0, 8.0, 2.0)).abs().max_element() < 1e-3 && (hi * 16.0 - Vec3::new(5.0, 24.0, 3.0)).abs().max_element() < 1e-3, "{lo} {hi}");
        let posed = mesh.skin(&pose(&Pose::default()))[cape];
        let (top, foot) = (posed.transform_point3(Vec3::new(0.0, 24.0, 2.5) / 16.0), posed.transform_point3(Vec3::new(0.0, 8.0, 2.5) / 16.0));
        assert!((top.z * 16.0 - 2.5).abs() < 0.2 && foot.z > top.z + 1.0 / 16.0, "{top} {foot}");
        assert!(skin(64, 32, Vec::new()).is_some() && skin(64, 64, Vec::new()).is_none());
    }
}
