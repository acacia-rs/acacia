//! Water/lava travel, swimming and currents (bedsim `liquid.go`).

use crate::aabb::Aabb;
use crate::constants::*;
use crate::math::{BlockPos, PI32, Vec3, add, block_pos, len, mc_cos, mc_sin, scale};
use crate::sim::Sim;
use crate::state::PlayerState;
use crate::world::{BlockPhysics, Liquid, LiquidKind, WorldView};

/// (neighbour offset, flow direction, face of the source cell towards it).
const FACES: [([i32; 3], Vec3, u8); 4] = [
    ([-1, 0, 0], [-1.0, 0.0, 0.0], 4),
    ([1, 0, 0], [1.0, 0.0, 0.0], 5),
    ([0, 0, -1], [0.0, 0.0, -1.0], 2),
    ([0, 0, 1], [0.0, 0.0, 1.0], 3),
];

fn opposite(face: u8) -> u8 {
    face ^ 1
}

fn offset(p: BlockPos, d: [i32; 3]) -> BlockPos {
    [p[0] + d[0], p[1] + d[1], p[2] + d[2]]
}

/// Shrinks a box per axis, collapsing to the midpoint when it would invert.
fn shrink(bb: Aabb, o: Vec3) -> Aabb {
    let (mut min, mut max) = ([0f32; 3], [0f32; 3]);
    for a in 0..3 {
        min[a] = bb.min[a] + o[a];
        max[a] = bb.max[a] - o[a];
        if min[a] > max[a] {
            let mid = (bb.min[a] + bb.max[a]) * 0.5;
            (min[a], max[a]) = (mid, mid);
        }
    }
    Aabb::new(min[0], min[1], min[2], max[0], max[1], max[2])
}

impl<W: WorldView + ?Sized> Sim<'_, W> {
    fn liquid_at(&self, pos: BlockPos) -> Option<Liquid> {
        self.w.block(pos).liquid
    }

    /// Liquid cells the shrunk box overlaps, however shallow the liquid: BDS (and Boar) push a player
    /// standing in the thinnest puddle, where bedsim requires the box to reach below the surface.
    pub(crate) fn touching_liquid_blocks(&self, st: &PlayerState, kind: LiquidKind) -> Vec<BlockPos> {
        self.liquid_blocks_touching(st.liquid_box.unwrap_or_else(|| st.bounding_box()), kind)
    }

    /// [`Self::touching_liquid_blocks`] for an explicit player box.
    pub(crate) fn liquid_blocks_touching(&self, player_box: Aabb, kind: LiquidKind) -> Vec<BlockPos> {
        let o = if kind == LiquidKind::Lava { [0.1, 0.4, 0.1] } else { [0.001, 0.401, 0.001] };
        let bb = shrink(player_box, o);
        let min = block_pos(bb.min);
        let max = [(bb.max[0] + 1.0).floor() as i32, (bb.max[1] + 1.0).floor() as i32, (bb.max[2] + 1.0).floor() as i32];
        let mut out = Vec::new();
        for x in min[0]..max[0] {
            for y in min[1]..max[1] {
                for z in min[2]..max[2] {
                    let pos = [x, y, z];
                    if self.liquid_at(pos).is_some_and(|l| l.kind == kind) {
                        out.push(pos);
                    }
                }
            }
        }
        out
    }

    /// Whether `p` is under the surface: depth/9 high, or full with a falling cell or liquid above.
    pub(crate) fn point_under(&self, p: Vec3, kind: LiquidKind) -> bool {
        let pos = block_pos(p);
        self.liquid_at(pos).is_some_and(|l| {
            let above = self.liquid_at([pos[0], pos[1] + 1, pos[2]]).is_some_and(|a| a.kind == kind);
            let surface = if l.falling || above { 1.0 } else { l.depth as f32 / 9.0 };
            l.kind == kind && p[1] < pos[1] as f32 + surface
        })
    }

    /// Whether the point `height` above the feet of a standing player (scaled with the size, whatever the
    /// pose) is under the surface.
    fn standing_height_in(&self, st: &PlayerState, kind: LiquidKind, height: f32) -> bool {
        let size = if st.size[2] <= 0.0 { 1.0 } else { st.size[2] };
        self.point_under([st.pos[0], st.pos[1] + height * size, st.pos[2]], kind)
    }

    /// The top of the head: BDS's sneak-sink test without `WantDown`.
    pub(crate) fn standing_head_in(&self, st: &PlayerState, kind: LiquidKind) -> bool {
        self.standing_height_in(st, kind, DEFAULT_PLAYER_HEIGHT)
    }

    /// BDS `PlayerInputRequestComponent::mBreathingPoint` outside the swim pose: the eyes (a daylight-detector
    /// drill puts it within 0.005 of feet + 1.62).
    fn breathing_point(&self, st: &PlayerState) -> Vec3 {
        let eye = if st.sneaking { SNEAKING_PLAYER_HEIGHT_OFFSET } else { DEFAULT_PLAYER_HEIGHT_OFFSET };
        let size = if st.size[2] <= 0.0 { 1.0 } else { st.size[2] };
        add(st.pos, [0.0, eye * size, 0.0])
    }

    /// BDS `UnderWaterSensingSystem`: the breathing point under the surface of its cell, cell + 1 - d/9
    /// (a source or falling cell is full); the sprint swim start needs it.
    pub(crate) fn eyes_in(&self, st: &PlayerState, kind: LiquidKind) -> bool {
        let p = self.breathing_point(st);
        let pos = block_pos(p);
        self.liquid_at(pos).is_some_and(|l| {
            let surface = if l.falling { 1.0 } else { (1 + l.depth) as f32 / 9.0 };
            l.kind == kind && p[1] < pos[1] as f32 + surface
        })
    }

    /// BDS `SwimTriggerSystem`'s water test for a sprint swim start: breathing point under water, and looking up
    /// (look y >= 0.15) the breathing point's block and the block above the box centre's must not be air. The
    /// look is `ActorRotationComponent`'s previous rotation before this input applies: the pitch two inputs back.
    pub(crate) fn swim_start_submerged(&self, st: &PlayerState) -> bool {
        if !self.eyes_in(st, LiquidKind::Water) {
            return false;
        }
        if -mc_sin(st.prev_pitch * PI32 / 180.0) < 0.15 {
            return true;
        }
        let bb = st.bounding_box();
        let centre = block_pos([(bb.min[0] + bb.max[0]) * 0.5, (bb.min[1] + bb.max[1]) * 0.5, (bb.min[2] + bb.max[2]) * 0.5]);
        !self.w.block(block_pos(self.breathing_point(st))).air && !self.w.block([centre[0], centre[1] + 1, centre[2]]).air
    }

    /// BDS `SwimTriggerSystem`'s look-up stop: swimming with the breathing point (the swim eye) in an air block
    /// while looking up more than 45° by its measure, acos(cos²(pitch)) (pitch above ~32.8°).
    pub(crate) fn swim_surfacing(&self, st: &PlayerState) -> bool {
        let a = st.prev_pitch * -PI32 / 180.0;
        let (look_y, horizontal) = (mc_sin(a), mc_cos(a));
        look_y > 0.0
            && (horizontal * horizontal).acos() * 57.29578 > 45.0
            && self.w.block(block_pos(add(st.pos, [0.0, COMPACT_PLAYER_HEIGHT_OFFSET, 0.0]))).air
    }

    /// Whether the box overlaps any liquid cell by more than 1e-5, however shallow (BDS; bedsim tests the
    /// liquid's height and counts a box merely touching the cell).
    pub(crate) fn contains_any_liquid(&self, bb: &Aabb) -> bool {
        let bb = &shrink(*bb, [1e-5; 3]);
        crate::collide::overlapped_cells(bb)
            .any(|pos| self.liquid_at(pos).is_some() && bb.max[1] > pos[1] as f32 && bb.min[1] < pos[1] as f32 + 1.0)
    }

    pub(crate) fn apply_liquid_flow(&self, st: &mut PlayerState, positions: &[BlockPos], kind: LiquidKind) -> bool {
        if positions.is_empty() {
            return true;
        }
        if !self.loaded(&st.bounding_box().grow(1.0)) {
            return false;
        }
        let mut flow = [0f32; 3];
        for &pos in positions {
            if let Some(l) = self.liquid_at(pos)
                && l.kind == kind
            {
                flow = add(flow, self.liquid_flow(pos, &l));
            }
        }
        let length = len(flow);
        if length >= 1e-4 {
            let strength = if kind == LiquidKind::Lava { 0.0035 } else { 0.014 };
            st.set_vel(add(st.vel, scale(flow, strength / length)));
        }
        true
    }

    fn face_closed(&self, b: &BlockPhysics, face: u8) -> bool {
        b.solid_faces & (1 << face) != 0
    }

    fn liquid_flow(&self, pos: BlockPos, l: &Liquid) -> Vec3 {
        let decay = l.decay();
        let here = self.w.block(pos);
        let mut flow = [0f32; 3];
        for (d, dir, face) in FACES {
            let npos = offset(pos, d);
            let nb = self.w.block(npos);
            let closed = self.face_closed(&here, face) || self.face_closed(&nb, opposite(face));
            if let Some(n) = nb.liquid
                && n.kind == l.kind
            {
                if !closed {
                    flow = add(flow, scale(dir, (n.decay() - decay) as f32));
                }
                continue;
            }
            if closed {
                continue;
            }
            // A dry neighbour that blocks motion (a fence too, open faces or not) takes no flow (vanilla).
            if self.blocks_motion(npos) {
                continue;
            }
            if let Some(lower) = self.liquid_at(offset(npos, [0, -1, 0]))
                && lower.kind == l.kind
            {
                flow = add(flow, scale(dir, (lower.decay() - decay + 8) as f32));
            }
        }
        if l.falling {
            for (d, _, _) in FACES {
                let npos = offset(pos, d);
                if self.has_collision(npos) || self.has_collision(offset(npos, [0, 1, 0])) {
                    let length = len(flow);
                    if length > 1e-4 {
                        flow = scale(flow, 1.0 / length);
                    }
                    flow[1] -= 6.0;
                    break;
                }
            }
        }
        let length = len(flow);
        if length > 1e-4 { scale(flow, 1.0 / length) } else { [0.0; 3] }
    }

    /// Collision above carpet height: a carpet lets flow pass over it (fuzz).
    fn blocks_motion(&self, pos: BlockPos) -> bool {
        let mut boxes = Vec::new();
        self.w.block_collisions(pos, &mut boxes);
        boxes.iter().any(|b| b.max[1] > 0.0625 + 1e-4)
    }

    fn has_collision(&self, pos: BlockPos) -> bool {
        let mut boxes = Vec::new();
        self.w.block_collisions(pos, &mut boxes);
        !boxes.is_empty()
    }

    pub(crate) fn update_swim_travel(&self, st: &mut PlayerState) {
        if !st.swimming || st.effective_jumping {
            return;
        }
        let target = -mc_sin(st.pitch * PI32 / 180.0);
        let rate = if target < -0.2 { 0.085 } else { 0.06 };
        if target > 0.0 && !st.pressing_descend {
            let below = self.w.block(block_pos(add(st.pos, [0.0, DEFAULT_PLAYER_HEIGHT_OFFSET - 1.1, 0.0])));
            // Liquid at the swim pose's eye keeps steering (BDS brackets it to 0.346..0.412; bedsim 0.42).
            let liquid = block_pos(add(st.pos, [0.0, COMPACT_PLAYER_HEIGHT_OFFSET, 0.0]));
            if below.air && below.liquid.is_none() && self.liquid_at(liquid).is_none() {
                st.set_vel([st.vel[0], 0.0, st.vel[2]]);
                return;
            }
        }
        let mut v = st.vel;
        v[1] += (target - v[1]) * rate;
        st.set_vel(v);
    }
}
