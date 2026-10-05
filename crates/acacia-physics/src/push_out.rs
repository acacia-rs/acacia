//! BDS `MoveTowardsClosestSpaceSystem`: a player overlapping collision boxes is pushed out sideways
//! through its velocity instead of being moved out (see README).

use crate::aabb::Aabb;
use crate::sim::Sim;
use crate::state::PlayerState;
use crate::world::WorldView;

const PUSH_SPEED: f32 = 0.1;

fn overlaps(a: &Aabb, b: &Aabb) -> bool {
    (0..3).all(|i| a.min[i] < b.max[i] && b.min[i] < a.max[i])
}

impl<W: WorldView + ?Sized> Sim<'_, W> {
    /// After a tick whose sweep found the box inside a collider: pins the x/z velocity at 0.1 away from the
    /// overlapped boxes' mean centre (+x+z when centred), each axis turned or dropped where the next block is taken.
    pub(crate) fn push_towards_closest_space(&self, st: &mut PlayerState) {
        if !st.penetrated_last_frame {
            return;
        }
        let bb = st.bounding_box();
        let (inside, around): (Vec<Aabb>, Vec<Aabb>) =
            self.nearby_bboxes(st,&bb.grow_vec([1.0, 0.0, 1.0])).into_iter().partition(|b| overlaps(b, &bb));
        if inside.is_empty() {
            return;
        }
        let mean = |axis: usize| inside.iter().map(|b| (b.min[axis] + b.max[axis]) * 0.5).sum::<f32>() / inside.len() as f32;
        let centre = [mean(0), mean(2)];
        let half = [(bb.max[0] - bb.min[0]) * 0.5, (bb.max[2] - bb.min[2]) * 0.5];
        let mut d = [(bb.min[0] + bb.max[0]) * 0.5 - centre[0], (bb.min[2] + bb.max[2]) * 0.5 - centre[1]];
        if d[0].abs() < f32::EPSILON && d[1].abs() < f32::EPSILON {
            d = [1.0, 1.0];
        }
        let probe = Aabb::new(centre[0] - half[0], bb.min[1], centre[1] - half[1], centre[0] + half[0], bb.max[1], centre[1] + half[1]);
        for (i, axis) in [(0, 0), (1, 2)] {
            let taken = |sign: f32| {
                let mut step = [0.0; 3];
                step[axis] = sign;
                let probe = probe.extend(step);
                around.iter().any(|b| overlaps(b, &probe))
            };
            let sign = d[i].signum();
            if taken(sign) {
                d[i] = if taken(-sign) { 0.0 } else { -d[i] };
            }
        }
        let len = (d[0] * d[0] + d[1] * d[1]).sqrt();
        if len < 1e-4 {
            return;
        }
        let mut v = st.vel;
        for (i, axis) in [(0, 0), (1, 2)] {
            let push = d[i] / len * PUSH_SPEED;
            if v[axis].abs() < push.abs() {
                v[axis] = (v[axis] + push).clamp(-push.abs(), push.abs());
            }
        }
        st.set_vel(v);
    }
}
