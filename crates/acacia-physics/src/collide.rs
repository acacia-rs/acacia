//! Collision sweep, auto-step, sneak edge avoidance and support lookup (bedsim `tryCollisions` & co).

use crate::aabb::Aabb;
use crate::clip::{auto_step, clip_all};
use crate::constants::STEP_HEIGHT;
use crate::math::{BlockPos, Vec3, add, block_pos, hz_dist_sqr, len_sqr, pos_vec, sub};
use crate::motion::sprint_movement_blocked;
use crate::sim::Sim;
use crate::state::PlayerState;
use crate::world::{Traversal, WorldView};

/// Cells from floor(min) to ceil(max) inclusive, y outermost (bedsim `nearbyBlocks`).
pub(crate) fn nearby_cells(bb: &Aabb) -> impl Iterator<Item = BlockPos> {
    let min = block_pos(bb.min);
    let max = [bb.max[0].ceil() as i32, bb.max[1].ceil() as i32, bb.max[2].ceil() as i32];
    (min[1]..=max[1]).flat_map(move |y| {
        (min[0]..=max[0]).flat_map(move |x| (min[2]..=max[2]).map(move |z| [x, y, z]))
    })
}

/// Cells overlapped by `bb` (floor(min) to ceil(max) exclusive), x outermost.
pub(crate) fn overlapped_cells(bb: &Aabb) -> impl Iterator<Item = BlockPos> {
    let min = block_pos(bb.min);
    let max = [bb.max[0].ceil() as i32, bb.max[1].ceil() as i32, bb.max[2].ceil() as i32];
    (min[0]..max[0]).flat_map(move |x| {
        (min[1]..max[1]).flat_map(move |y| (min[2]..max[2]).map(move |z| [x, y, z]))
    })
}

/// Cells BDS's entity-inside walk visits: floor(min + 0.001)..=floor(max - 0.001), so a grazed cell is skipped.
pub(crate) fn inside_cells(bb: &Aabb) -> impl Iterator<Item = BlockPos> {
    let (min, max) = (block_pos(bb.min.map(|v| v + 1e-3)), block_pos(bb.max.map(|v| v - 1e-3)));
    (min[0]..=max[0]).flat_map(move |x| (min[1]..=max[1]).flat_map(move |y| (min[2]..=max[2]).map(move |z| [x, y, z])))
}

fn centre(bb: &Aabb) -> Vec3 {
    [(bb.min[0] + bb.max[0]) * 0.5, bb.min[1], (bb.min[2] + bb.max[2]) * 0.5]
}

const EDGE_BOUNDARY: f32 = 0.025;
const EDGE_OFFSET: f32 = 0.05;
const EDGE_MAX_ITER: usize = 1000;

fn shrink_towards_zero(v: f32) -> f32 {
    if (-EDGE_OFFSET..EDGE_OFFSET).contains(&v) {
        0.0
    } else if v > 0.0 {
        v - EDGE_OFFSET
    } else {
        v + EDGE_OFFSET
    }
}

impl<W: WorldView + ?Sized> Sim<'_, W> {
    /// Boxes movement collides with: scaffolding is solid only from above its block and not while descending
    /// (as Java's `ScaffoldingBlock`; strict BDS fuzz walks through its side).
    pub(crate) fn movement_bboxes(&self, st: &PlayerState, area: &Aabb) -> Vec<Aabb> {
        let feet = st.bounding_box().min[1];
        let mut boxes = self.nearby_bboxes(area);
        boxes.retain(|b| {
            let cell = block_pos([(b.min[0] + b.max[0]) * 0.5, (b.min[1] + b.max[1]) * 0.5, (b.min[2] + b.max[2]) * 0.5]);
            self.w.block(cell).traversal != Traversal::Scaffolding || !st.pressing_descend && feet > cell[1] as f32 + 1.0 - 1e-5
        });
        boxes
    }

    /// Sweeps Y, X, Z against nearby boxes, tries an auto-step and commits position and flags.
    pub(crate) fn try_collisions(&self, st: &mut PlayerState) -> bool {
        self.prepare_collision_box(st);
        let start = st.bounding_box();
        let mut bb = start;
        let cur = st.vel;
        let boxes = self.movement_bboxes(st, &bb.extend(cur));
        // BDS never moves a player out of a box it overlaps (see `push_out`); bedsim only once stuck.
        let one_way = true;
        let mut pen = [0f32; 3];

        let y = clip_all(&boxes, &bb, [0.0, cur[1], 0.0], one_way, Some(&mut pen));
        bb = bb.translate(y);
        let x = clip_all(&boxes, &bb, [cur[0], 0.0, 0.0], one_way, Some(&mut pen));
        bb = bb.translate(x);
        let z = clip_all(&boxes, &bb, [0.0, 0.0, cur[2]], one_way, Some(&mut pen));
        bb = bb.translate(z);
        let mut coll = add(add(y, x), z);

        let has_pen = len_sqr(pen) >= 1e-11;
        st.stuck_in_collider = st.penetrated_last_frame && has_pen;
        st.penetrated_last_frame = has_pen;

        let (xc, yc, zc) = (cur[0] != coll[0], cur[1] != coll[1], cur[2] != coll[2]);
        if (st.on_ground || (yc && cur[1] < 0.0)) && (xc || zc) {
            if !self.loaded(&start.extend(cur).extend_up(STEP_HEIGHT)) {
                return false;
            }
            let step = auto_step(&start, cur, &boxes, one_way);
            if !self.has_nearby_bboxes(&step.bb) && hz_dist_sqr(coll) < hz_dist_sqr(step.velocity) {
                coll = step.velocity;
                bb = step.bb;
            }
        }

        let end = centre(&bb);
        st.sprint_movement_blocked = sprint_movement_blocked(cur, sub(end, st.pos));
        st.set_pos(end);
        st.remember_box(bb);

        let yc = (cur[1] - coll[1]).abs() >= 1e-5;
        st.collide_x = (cur[0] - coll[0]).abs() >= 1e-5;
        st.collide_y = yc;
        st.collide_z = (cur[2] - coll[2]).abs() >= 1e-5;
        // Ground contact comes from the requested Y movement, including after auto-step.
        st.on_ground = (yc && cur[1] < 0.0) || (st.on_ground && !yc && cur[1] == 0.0);
        if !self.check_supporting_block(st, cur) {
            return false;
        }
        st.set_vel(coll);
        true
    }

    /// Limits sneaking movement to supported ground.
    pub(crate) fn avoid_edge(&self, st: &mut PlayerState) -> bool {
        if !st.sneaking || !st.on_ground || st.vel[1] > 0.0 {
            return true;
        }
        const DROP: f32 = -STEP_HEIGHT * 1.01;
        let bb = st.bounding_box().grow_vec([-EDGE_BOUNDARY, 0.0, -EDGE_BOUNDARY]);
        let (mut xm, mut zm) = (st.vel[0], st.vel[2]);
        if !self.loaded(&bb.extend([xm, DROP, zm])) {
            return false;
        }
        let supported = |dx: f32, dz: f32| self.has_nearby_bboxes(&bb.translate([dx, DROP, dz]));

        let mut i = 0;
        while i < EDGE_MAX_ITER && xm != 0.0 && !supported(xm, 0.0) {
            xm = shrink_towards_zero(xm);
            i += 1;
        }
        if i == EDGE_MAX_ITER {
            xm = 0.0;
        }
        i = 0;
        while i < EDGE_MAX_ITER && zm != 0.0 && !supported(0.0, zm) {
            zm = shrink_towards_zero(zm);
            i += 1;
        }
        if i == EDGE_MAX_ITER {
            zm = 0.0;
        }
        i = 0;
        while i < EDGE_MAX_ITER && xm != 0.0 && zm != 0.0 && !supported(xm, zm) {
            xm = shrink_towards_zero(xm);
            zm = shrink_towards_zero(zm);
            i += 1;
        }
        if i == EDGE_MAX_ITER {
            xm = 0.0;
            zm = 0.0;
        }
        let v = [xm, st.vel[1], zm];
        st.set_vel(v);
        true
    }

    fn check_supporting_block(&self, st: &mut PlayerState, vel: Vec3) -> bool {
        if !st.on_ground {
            st.supporting_block = None;
            return true;
        }
        let mut dec = st.bounding_box().extend_down(1e-3);
        if !self.loaded(&dec) {
            st.supporting_block = None;
            return false;
        }
        st.supporting_block = self.find_supporting_block(st, &dec);
        if st.supporting_block.is_none() {
            dec = dec.translate([-vel[0], 0.0, -vel[2]]);
            if !self.loaded(&dec) {
                return false;
            }
            st.supporting_block = self.find_supporting_block(st, &dec);
        }
        true
    }

    /// The colliding block whose centre is closest to the feet (BDS, like Java; bedsim measures from the
    /// centre of the feet's cell).
    fn find_supporting_block(&self, st: &PlayerState, bb: &Aabb) -> Option<BlockPos> {
        let mut best = None;
        let mut min_dist = f32::MAX;
        let mut boxes = Vec::new();
        for pos in nearby_cells(bb) {
            boxes.clear();
            self.w.block_collisions(pos, &mut boxes);
            let origin = pos_vec(pos);
            if let Some(_hit) = boxes.iter().find(|b| !b.is_empty() && bb.intersects(&b.translate(origin))) {
                let dist = len_sqr(sub(add(origin, [0.5, 0.5, 0.5]), st.pos));
                if dist < min_dist {
                    min_dist = dist;
                    best = Some(pos);
                }
            }
        }
        best
    }

    /// BDS `CurrentlyStandingOnBlockSystem`: of the collision boxes crossing the plane 0.2 under the feet,
    /// the highest-topped (ties: centre closest to the feet); the feet's own cell if none.
    pub(crate) fn standing_on_block(&self, st: &PlayerState) -> BlockPos {
        let mut plane = st.bounding_box();
        plane.min[1] -= 0.2;
        plane.max[1] = plane.min[1];
        let mut best = None;
        let mut boxes = Vec::new();
        for pos in nearby_cells(&plane) {
            boxes.clear();
            self.w.block_collisions(pos, &mut boxes);
            let origin = pos_vec(pos);
            let crossing = boxes.iter().map(|b| b.translate(origin)).filter(|b| {
                b.min[1] <= plane.min[1] && b.max[1] >= plane.min[1]
                    && b.max[0] > plane.min[0] && b.min[0] < plane.max[0] && b.max[2] > plane.min[2] && b.min[2] < plane.max[2]
            });
            if let Some(top) = crossing.map(|b| b.max[1]).reduce(f32::max) {
                let dist = len_sqr(sub(add(origin, [0.5, 0.5, 0.5]), st.pos));
                if best.is_none_or(|(t, d, _)| top > t || top == t && dist < d) {
                    best = Some((top, dist, pos));
                }
            }
        }
        best.map_or_else(|| block_pos(st.pos), |(_, _, pos)| pos)
    }

    pub(crate) fn is_inside_cobweb(&self, st: &PlayerState) -> bool {
        // BDS ignores a web the box only grazes (fuzz: a 1.4e-5 overlap does not slow).
        let bb = st.bounding_box().grow(-0.001);
        nearby_cells(&bb.grow(1.0)).any(|pos| {
            let b = self.w.block(pos);
            !b.air && b.cobweb && bb.intersects(&Aabb::block(pos))
        })
    }
}
