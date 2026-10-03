//! Bounding boxes, retained collision endpoints and pose transitions (bedsim `bbox.go` + pose helpers).

use crate::aabb::Aabb;
use crate::math::Vec3;
use crate::sim::Sim;
use crate::state::{CollisionShape, PlayerState};
use crate::world::WorldView;

impl PlayerState {
    /// Scaled half-width and pose height.
    pub(crate) fn collision_dims(&self) -> [f32; 2] {
        let scale = self.size[2];
        let mut height = self.size[1] * scale;
        if self.swim_pose() || self.gliding {
            height = self.size[0] * scale;
        }
        [(self.size[0] * 0.5) * scale, height]
    }

    pub(crate) fn box_with_dims(&self, pos: Vec3, dims: [f32; 2]) -> Aabb {
        if let Some(s) = &self.shape
            && s.pos == pos
            && s.dims == dims
            && pos == self.pos
        {
            return s.bb;
        }
        let [w, h] = dims;
        Aabb::new(pos[0] - w, pos[1], pos[2] - w, pos[0] + w, pos[1] + h, pos[2] + w)
    }

    /// The player's collision box at the current position.
    pub fn bounding_box(&self) -> Aabb {
        self.box_with_dims(self.pos, self.collision_dims())
    }

    pub(crate) fn fresh_box(&self) -> Aabb {
        let [w, h] = self.collision_dims();
        let p = self.pos;
        Aabb::new(p[0] - w, p[1], p[2] - w, p[0] + w, p[1] + h, p[2] + w)
    }

    pub(crate) fn remember_box(&mut self, bb: Aabb) {
        self.shape = Some(CollisionShape { bb, pos: self.pos, dims: self.collision_dims() });
    }

    pub(crate) fn set_swimming_pose_flags(&mut self) {
        self.sneaking = false;
        self.crawling = false;
        self.size[1] = self.standing_height;
    }
}

/// Restores horizontal contact faces only when the box's f32 centre still equals `pos`.
fn recover_rounded_contacts(bb: Aabb, pos: Vec3, boxes: &[Aabb]) -> Aabb {
    let (omin, omax) = (bb.min, bb.max);
    let (mut low, mut high) = (omin, omax);
    for other in boxes {
        if !bb.intersects(other) {
            continue;
        }
        for axis in [0, 2] {
            let face = other.max[axis];
            if face > omin[axis] && face < omax[axis] && (face + omax[axis]) * 0.5 == pos[axis] {
                low[axis] = low[axis].max(face);
            }
            let face = other.min[axis];
            if face < omax[axis] && face > omin[axis] && (omin[axis] + face) * 0.5 == pos[axis] {
                high[axis] = high[axis].min(face);
            }
        }
    }
    for axis in [0, 2] {
        if low[axis] >= high[axis] || (low[axis] + high[axis]) * 0.5 != pos[axis] {
            low[axis] = omin[axis];
            high[axis] = omax[axis];
        }
    }
    Aabb::new(low[0], low[1], low[2], high[0], high[1], high[2])
}

impl<W: WorldView + ?Sized> Sim<'_, W> {
    /// Recovers unknown contact endpoints from a rounded position once the area is loaded.
    pub(crate) fn prepare_collision_box(&self, st: &mut PlayerState) {
        let dims = st.collision_dims();
        if let Some(s) = &st.shape {
            let same = s.dims == dims;
            if same && s.pos == st.pos {
                return;
            }
            if !same {
                let bb = st.fresh_box();
                st.remember_box(bb);
                return;
            }
        }
        let bb = st.fresh_box();
        if !self.loaded(&bb) {
            return;
        }
        let boxes = self.nearby_bboxes(&bb);
        let bb = recover_rounded_contacts(bb, st.pos, &boxes);
        st.remember_box(bb);
    }

    pub(crate) fn loaded(&self, area: &Aabb) -> bool {
        self.w.is_area_loaded(area)
    }

    pub(crate) fn nearby_bboxes(&self, area: &Aabb) -> Vec<Aabb> {
        let mut out = Vec::new();
        self.w.collisions(area, &mut out);
        out.retain(|b| !b.is_empty());
        out
    }

    pub(crate) fn has_nearby_bboxes(&self, area: &Aabb) -> bool {
        !self.nearby_bboxes(area).is_empty()
    }

    pub(crate) fn pose_collisions_available(&self, st: &PlayerState) -> bool {
        self.loaded(&st.bounding_box())
    }

    /// Whether a standing box of `height` is free, and whether that could be determined.
    pub(crate) fn can_fit_height_known(&self, st: &PlayerState, height: f32) -> (bool, bool) {
        let scale = st.size[2];
        let bb = st.box_with_dims(st.pos, [(st.size[0] * 0.5) * scale, height * scale]);
        if !self.loaded(&bb) {
            return (false, false);
        }
        (!self.has_nearby_bboxes(&bb), true)
    }

    /// Chooses standing, sneaking or crawling without entering a ceiling.
    pub(crate) fn restore_upright_pose(&self, st: &mut PlayerState, collisions_available: bool) -> bool {
        if !collisions_available {
            return false;
        }
        for (height, sneaking) in [(st.standing_height, false), (st.sneaking_height, true)] {
            match self.can_fit_height_known(st, height) {
                (_, false) => return false,
                (true, true) => {
                    st.sneaking = sneaking;
                    st.crawling = false;
                    st.size[1] = height;
                    return true;
                }
                _ => {}
            }
        }
        st.sneaking = false;
        st.crawling = true;
        st.size[1] = st.crawling_height;
        true
    }

    pub(crate) fn stop_gliding(&self, st: &mut PlayerState) -> bool {
        st.gliding = false;
        if st.swim_pose() {
            return true;
        }
        let available = self.pose_collisions_available(st);
        self.restore_upright_pose(st, available)
    }
}
