//! The boat, which the game has in code and no pack file describes (`hardcoded.geo.json`, after
//! Java's `BoatModel`): its paddles row as `animatePaddle` turns them, and a patch over the hull
//! is drawn to the depth only, so the water behind it is not (Java's `water_patch`).

use super::bake::Mesh;
use super::controller::Definition;
use super::molang::Value;
use super::{Blend, BonePose, EntityModels, Layer, NO_TEXTURE};

/// Left, right.
const PADDLES: [&str; 2] = ["left_paddle", "right_paddle"];
/// The queries a caller answers with each paddle's rowing time: Java's `rowingTime`, 0 at rest
/// and π/8 more each tick it rows.
pub const ROW_TIME: [&str; 2] = ["row_time_left", "row_time_right"];
/// A geometry of this name after another's is that one's depth-only patch.
pub(super) const MASK: &str = ".mask";

/// A paddle's x and y turn in degrees at rowing `time`. The geometry rests at `time` 0.
fn turn(time: f32, side: usize) -> [f32; 2] {
    let between = |from: f32, to: f32, phase: f32| from + (to - from) * ((phase.sin() + 1.0) / 2.0);
    let y = between(-45.0, 45.0, 1.0 - time);
    [between(-60.0, -15.0, -time), if side == 1 { 180.0 - y } else { y }]
}

/// The paddles' poses, for a mesh that has them and a rower that rows.
pub(super) fn paddles<'a>(mesh: &Mesh, query: &'a dyn Fn(&str) -> Value) -> impl Iterator<Item = BonePose> + 'a {
    let rows = mesh.bones.iter().any(|b| b == PADDLES[0]);
    (0..2).filter(move |_| rows).filter_map(move |side| {
        let time = query(ROW_TIME[side]).num();
        let (rest, now) = (turn(0.0, side), turn(time, side));
        (time != 0.0).then(|| BonePose { bone: PADDLES[side].into(), rotation: [now[0] - rest[0], now[1] - rest[1], 0.0], position: [0.0; 3], scale: [1.0; 3] })
    })
}

impl EntityModels {
    /// The depth-only layer of a kind whose default geometry has a patch.
    pub(super) fn mask(&self, definition: &Definition) -> Option<Layer> {
        let model = *self.by_geometry.get(&format!("{}{MASK}", definition.geometry.get("default")?))?;
        Some(Layer { blend: Blend::Mask, ..Layer::plain(model, NO_TEXTURE) })
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::*;
    use crate::entity::{Pose, bake, geometry};

    fn boat() -> Mesh {
        bake::bake(&geometry::parse(include_str!("hardcoded.geo.json"))["geometry.boat"])
    }

    /// The far end of a paddle's blade, in 1/16 block.
    fn blade(mesh: &Mesh, pose: &Pose, side: usize) -> Vec3 {
        let bone = mesh.bones.iter().position(|b| b == PADDLES[side]).unwrap();
        let posed = mesh.skin(pose)[bone];
        // The blade is the bone's second box; its outer end is the box's far z before the turn.
        let corners = mesh.vertices.iter().filter(|v| v.bone as usize == bone).skip(36).map(|v| posed.transform_point3(Vec3::from(v.position)) * 16.0);
        corners.fold(Vec3::ZERO, |far, p| if p.z.abs() > far.z.abs() { p } else { far })
    }

    #[test]
    fn paddles_rest_at_java_s_idle_angles_with_their_blades_outboard() {
        let mesh = boat();
        for side in 0..2 {
            let bone = mesh.bones.iter().position(|b| b == PADDLES[side]).unwrap();
            let [x, y] = turn(0.0, side);
            let rest = mesh.joints[bone].rotation;
            assert!((rest[0] - x).abs() < 1e-3 && (rest[1] - y).abs() < 1e-3 && rest[2] == 11.25, "{rest:?} against {x} {y}");
            assert_eq!(mesh.vertices.iter().filter(|v| v.bone as usize == bone).count(), 72, "a shaft and a blade");
        }
        assert_eq!((turn(0.0, 0)[0], turn(0.0, 1)[0]), (-37.5, -37.5));
        // The hull's sides are 10 px out: the blades hang beyond them, below the rim, mirrored.
        let (left, right) = (blade(&mesh, &Pose::default(), 0), blade(&mesh, &Pose::default(), 1));
        assert!(left.z > 15.0 && left.y < 0.0 && left.x > 3.0, "{left}");
        assert!((right - left * Vec3::new(1.0, 1.0, -1.0)).abs().max_element() < 0.01, "{left} {right}");
    }

    #[test]
    fn a_rowing_paddle_sweeps_and_a_resting_one_stays() {
        let mesh = boat();
        let rowing = |time: f32| move |name: &str| Value::Num(if name == ROW_TIME[0] { time } else { 0.0 });
        assert_eq!(paddles(&mesh, &rowing(0.0)).count(), 0);
        // A quarter of a stroke on: the left blade has lifted and swung; the right one has not moved.
        let quarter = std::f32::consts::FRAC_PI_2;
        let pose = Pose(paddles(&mesh, &rowing(quarter)).collect());
        assert_eq!(pose.0.len(), 1);
        let [x, y] = turn(quarter, 0);
        assert!((x + 60.0).abs() < 1e-3 && (y + 45.0 - 90.0 * ((1.0 - quarter).sin() + 1.0) / 2.0).abs() < 1e-3, "{x} {y}");
        let (rest, swept) = (blade(&mesh, &Pose::default(), 0), blade(&mesh, &pose, 0));
        assert!(swept.distance(rest) > 4.0, "{rest} {swept}");
        assert_eq!(blade(&mesh, &pose, 1), blade(&mesh, &Pose::default(), 1));
        // A whole stroke later it is back where it was.
        let again = Pose(paddles(&mesh, &rowing(quarter + std::f32::consts::TAU)).collect());
        assert!(blade(&mesh, &again, 0).distance(swept) < 0.01);
    }
}
