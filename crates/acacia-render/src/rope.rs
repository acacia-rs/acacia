//! Fishing lines and leads: a sagging run of thin boxes between two points, drawn through the
//! entity pass. The curves are Java's (`FishingHookRenderer`, `LeashFeatureRenderer`); Java draws
//! the line a pixel wide and the lead in two alternating browns, here each is one colour.

use std::sync::{Arc, LazyLock};

use glam::{DVec3, Mat4, Quat, Vec3};

use crate::entity::bake::{Joint, Mesh, Vertex};
use crate::entity::{EntityInstance, Layer, NO_MODEL, NO_TEXTURE, Pose, Skin};

const LINE_SEGMENTS: usize = 16;
const LEAD_SEGMENTS: usize = 24;
/// Blocks across.
const LINE_WIDTH: f32 = 0.0125;
const LEAD_WIDTH: f32 = 0.05;
/// A bobber's line leaves it this far above its feet.
const BOBBER_LIFT: f64 = 0.25;

/// What a rope is: how it hangs and what it looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rope {
    FishingLine,
    Lead,
}

struct Looks {
    line: Arc<Skin>,
    lead: Arc<Skin>,
    layers: Arc<[Layer]>,
}

/// Shared: the entity pass keeps one texture per skin.
static LOOKS: LazyLock<Looks> = LazyLock::new(|| {
    let skin = |rgb: [u8; 3]| Arc::new(Skin { width: 1, height: 1, rgba: vec![rgb[0], rgb[1], rgb[2], 255], mesh: Some(unit_box()) });
    Looks { line: skin([0, 0, 0]), lead: skin([0x6F, 0x52, 0x3A]), layers: [Layer::plain(NO_MODEL, NO_TEXTURE)].into() }
});

/// A box one block each way around the origin, in one texel.
fn unit_box() -> Mesh {
    let mut vertices = Vec::with_capacity(36);
    for axis in 0..3 {
        for side in [-0.5f32, 0.5] {
            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            let corner = |a: f32, b: f32| {
                let mut p = [0.0; 3];
                (p[axis], p[u], p[v]) = (side, a, b);
                let mut normal = [0.0; 3];
                normal[axis] = side * 2.0;
                Vertex { position: p, bone: 0, normal, uv: [0.5, 0.5] }
            };
            let c = [corner(-0.5, -0.5), corner(0.5, -0.5), corner(0.5, 0.5), corner(-0.5, 0.5)];
            vertices.extend([c[0], c[1], c[2], c[0], c[2], c[3]]);
        }
    }
    let joint = Joint { parent: None, pivot: Vec3::ZERO, rotation: [0.0; 3], unbind: Mat4::IDENTITY };
    Mesh { vertices, bones: vec!["root".into()], joints: vec![joint] }
}

/// The points a rope hangs through, from `end` (the bobber's feet, or where the lead ties to the
/// mob) to `holder` (the rod's tip, or the hand or knot holding the lead).
pub fn curve(rope: Rope, end: DVec3, holder: DVec3) -> Vec<DVec3> {
    let d = holder - end;
    let (segments, height): (usize, fn(f64, f64) -> f64) = match rope {
        // Slack at the bobber, taut at the rod.
        Rope::FishingLine => (LINE_SEGMENTS, |dy, t| dy * (t * t + t) * 0.5 + BOBBER_LIFT * (1.0 - t)),
        // Hangs below the straight line whichever end is higher.
        Rope::Lead => (LEAD_SEGMENTS, |dy, t| if dy > 0.0 { dy * t * t } else { dy - dy * (1.0 - t) * (1.0 - t) }),
    };
    (0..=segments)
        .map(|i| {
            let t = i as f64 / segments as f64;
            end + DVec3::new(d.x * t, height(d.y, t), d.z * t)
        })
        .collect()
}

/// A rope's boxes for a frame, one a segment of its [`curve`].
pub fn instances(rope: Rope, end: DVec3, holder: DVec3, camera: DVec3) -> Vec<EntityInstance> {
    let (skin, width) = match rope {
        Rope::FishingLine => (&LOOKS.line, LINE_WIDTH),
        Rope::Lead => (&LOOKS.lead, LEAD_WIDTH),
    };
    curve(rope, end, holder)
        .windows(2)
        .map(|ends| {
            let (middle, along) = ((ends[0] + ends[1]) / 2.0, (ends[1] - ends[0]).as_vec3());
            let turn = Quat::from_rotation_arc(Vec3::X, along.try_normalize().unwrap_or(Vec3::X));
            let frame = Mat4::from_translation((middle - camera).as_vec3()) * Mat4::from_quat(turn) * Mat4::from_scale(Vec3::new(along.length(), width, width));
            EntityInstance { layers: LOOKS.layers.clone(), skin: Some(skin.clone()), position: middle, yaw: 0.0, scale: 1.0, pose: Pose::default(), frame: Some(frame), hurt: false, glint: None }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ropes_run_end_to_end_and_sag_below_the_straight_line() {
        let (end, holder) = (DVec3::new(0.0, 60.0, 0.0), DVec3::new(8.0, 63.0, 4.0));
        for (rope, lift) in [(Rope::FishingLine, BOBBER_LIFT), (Rope::Lead, 0.0)] {
            let points = curve(rope, end, holder);
            assert!(points[0].distance(end + DVec3::Y * lift) < 1e-9 && points.last().unwrap().distance(holder) < 1e-9, "{rope:?}");
            let middle = points[points.len() / 2];
            assert!(middle.y < (end.y + holder.y) / 2.0 + lift / 2.0 && (middle.x - 4.0).abs() < 1e-9, "{rope:?} {middle}");
        }
        // A lead sags the same way when the mob is the higher end.
        let down = curve(Rope::Lead, holder, end);
        assert!(down[LEAD_SEGMENTS / 2].y < (end.y + holder.y) / 2.0);
    }

    #[test]
    fn a_segment_s_box_spans_its_two_points() {
        let (end, holder, camera) = (DVec3::new(10.0, 60.0, 0.0), DVec3::new(10.0, 60.0, 16.0), DVec3::new(10.0, 60.0, -5.0));
        let boxes = instances(Rope::Lead, end, holder, camera);
        assert_eq!(boxes.len(), LEAD_SEGMENTS);
        let frame = boxes[0].frame.unwrap();
        let (from, to) = (frame.transform_point3(Vec3::new(-0.5, 0.0, 0.0)), frame.transform_point3(Vec3::new(0.5, 0.0, 0.0)));
        assert!(from.abs_diff_eq(Vec3::new(0.0, 0.0, 5.0), 1e-4) && to.abs_diff_eq(Vec3::new(0.0, 0.0, 5.0 + 16.0 / 24.0), 1e-4), "{from} {to}");
    }
}
