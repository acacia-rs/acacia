//! Cave culling (Java's `SectionOcclusionGraph`): a BFS from the camera's section that enters a
//! neighbour only through a face its entry face sees ([`crate::mesh::visibility`]), never turns
//! back towards the camera, and stays inside the frustum. Conservative: hides only sections no
//! line of sight can reach through open blocks.

use std::collections::VecDeque;

use glam::{IVec3, Vec3};
use rustc_hash::FxHashSet;

use crate::camera::Frustum;
use crate::mesh::quad::DIRS;
use crate::mesh::visibility::{ALL, connected};
use crate::workers::SectionKey;

const fn opposite(face: usize) -> usize {
    face ^ 1
}

/// Sections reachable from the camera. `visibility` gives a section's face pairs, `None` outside
/// the loaded world; `sections` is the world's section y range.
pub fn visible_sections(
    frustum: &Frustum,
    cam_block: IVec3,
    cam_frac: Vec3,
    sections: std::ops::Range<i32>,
    visibility: impl Fn(SectionKey) -> Option<u16>,
) -> FxHashSet<SectionKey> {
    let cam = cam_block >> IVec3::splat(4);
    let mut seen = FxHashSet::default();
    // (section, face it was entered through, faces travelled so far)
    let mut queue: VecDeque<(IVec3, Option<usize>, u8)> = VecDeque::new();
    let start = cam.with_y(cam.y.clamp(sections.start, sections.end - 1));
    let (entered, travelled) = match cam.y {
        y if y >= sections.end => (Some(2), 1 << 3),
        y if y < sections.start => (Some(3), 1 << 2),
        _ => (None, 0),
    };
    seen.insert(key(start));
    queue.push_back((start, entered, travelled));
    let in_view = |s: IVec3| {
        let min = (s * 16 - cam_block).as_vec3() - cam_frac;
        frustum.intersects_box(min, min + Vec3::splat(16.0))
    };
    while let Some((s, entered, travelled)) = queue.pop_front() {
        let vis = visibility(key(s)).unwrap_or(ALL);
        for (face, d) in DIRS.iter().enumerate() {
            if travelled & (1 << opposite(face)) != 0 {
                continue;
            }
            if entered.is_some_and(|e| !connected(vis, e, face)) {
                continue;
            }
            let n = s + IVec3::from_array(*d);
            if !sections.contains(&n.y) || seen.contains(&key(n)) || visibility(key(n)).is_none() || !in_view(n) {
                continue;
            }
            seen.insert(key(n));
            queue.push_back((n, Some(opposite(face)), travelled | 1 << face));
        }
    }
    seen
}

fn key(s: IVec3) -> SectionKey {
    (s.x, s.y, s.z)
}

#[cfg(test)]
mod tests {
    use glam::DVec3;

    use super::*;
    use crate::camera::Camera;
    use crate::mesh::visibility::connected;

    /// Camera in section (0, 0, 0) looking south (+Z) over a 9×9 column area.
    fn reach(visibility: impl Fn(SectionKey) -> u16) -> FxHashSet<SectionKey> {
        let cam = Camera::new(DVec3::new(8.0, 8.0, 8.0));
        let (block, frac) = cam.split_position();
        let loaded = |k: SectionKey| (k.0.abs() <= 4 && k.2.abs() <= 4).then(|| visibility(k));
        visible_sections(&Frustum::new(cam.view_proj()), block, frac, -4..20, loaded)
    }

    #[test]
    fn a_closed_wall_hides_what_is_behind_it_but_not_itself() {
        let open = reach(|_| ALL);
        assert!(open.contains(&(0, 0, 3)) && !open.contains(&(0, 0, -3)), "frustum still applies");
        let walled = reach(|k| if k.2 == 2 { 0 } else { ALL });
        assert!(walled.contains(&(0, 0, 2)) && !walled.contains(&(0, 0, 3)));
    }

    #[test]
    fn a_tunnel_lets_sight_through_only_along_its_axis() {
        // Faces 4 (+Z) and 5 (-Z) connect; nothing else does.
        let tunnel = 1 << 14;
        assert!(connected(tunnel, 4, 5) && !connected(tunnel, 5, 0));
        let r = reach(|k| if k.2 == 2 { if k == (0, 0, 2) { tunnel } else { 0 } } else { ALL });
        assert!(r.contains(&(0, 0, 3)), "straight through");
    }
}
