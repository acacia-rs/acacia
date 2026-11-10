use std::collections::HashSet;

use acacia_registry::prop::Drag;
use acacia_registry::{Block, BlockState, BlockTag};

use crate::aabb::Aabb;
use crate::collide::Blocks;
use crate::fluid::fluid_of;
use crate::mth::{floor, length_sqr};
use crate::player::{Player, WIDTH};
use crate::support::LEGACY_ON_POS;

const TARGET_MARGIN: f64 = 1.0E-5_f32 as f64;

/// Close to `isSuffocating`: full-cube collision, except the see-through blocks vanilla exempts.
fn suffocating(state: BlockState) -> bool {
    let block = state.block();
    let full = state.java_collision().is_some_and(|b| b == [[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]]);
    full && !block.is(BlockTag::Leaves) && !block.is(BlockTag::Impermeable)
}

pub fn collides_with_suffocating(world: &impl Blocks, area: &Aabb) -> bool {
    let low = area.min.map(floor);
    let high = area.max.map(floor);
    (low[0]..=high[0]).any(|x| {
        (low[1]..=high[1]).any(|y| (low[2]..=high[2]).any(|z| suffocating(world.block_state([x, y, z]))))
    })
}

fn old_delta_y(y: f64) -> f64 {
    y / 0.98_f32 as f64 + 0.08
}

fn new_delta_y(y: f64) -> f64 {
    (y - 0.08) * 0.98_f32 as f64
}

impl Player {
    fn honey_sliding(&self, pos: [i32; 3]) -> bool {
        if self.on_ground || self.pos[1] > pos[1] as f64 + 0.9375 - 1.0E-7 || old_delta_y(self.delta[1]) >= -0.08 {
            return false;
        }
        let dx = (pos[0] as f64 + 0.5 - self.pos[0]).abs();
        let dz = (pos[2] as f64 + 0.5 - self.pos[2]).abs();
        let overlap = 0.4375 + (WIDTH / 2.0) as f64;
        dx + 1.0E-7 > overlap || dz + 1.0E-7 > overlap
    }

    fn slide_on_honey(&mut self) {
        let old = old_delta_y(self.delta[1]);
        if old < -0.13 {
            let reduce = -0.05 / old;
            self.delta[0] *= reduce;
            self.delta[2] *= reduce;
        }
        self.delta[1] = new_delta_y(-0.05);
        self.fall_distance = 0.0;
    }

    fn make_stuck(&mut self, speed: [f64; 3]) {
        self.fall_distance = 0.0;
        self.stuck = speed;
    }

    fn bubble_column(&mut self, world: &impl Blocks, state: BlockState, pos: [i32; 3]) {
        let down = state.get(Drag) == Some(true);
        let above = world.block_state([pos[0], pos[1] + 1, pos[2]]);
        let open = above.java_collision().is_some_and(<[_]>::is_empty) && fluid_of(above).is_none();
        self.delta[1] = match (open, down) {
            (true, true) => (-0.9_f64).max(self.delta[1] - 0.03),
            (true, false) => 1.8_f64.min(self.delta[1] + 0.1),
            (false, true) => (-0.3_f64).max(self.delta[1] - 0.03),
            (false, false) => 0.7_f64.min(self.delta[1] + 0.06),
        };
        self.fall_distance = 0.0;
    }

    fn entity_inside(&mut self, world: &impl Blocks, pos: [i32; 3], precise: bool) {
        let state = world.block_state(pos);
        match state.block() {
            Block::Cobweb => {
                let speed = if self.effects.weaving { [0.5, 0.25, 0.5] } else { [0.25, 0.05_f32 as f64, 0.25] };
                self.make_stuck(speed);
            }
            Block::SweetBerryBush => self.make_stuck([0.8_f32 as f64, 0.75, 0.8_f32 as f64]),
            Block::PowderSnow => {
                if world.block_state(self.block_pos()).block() == Block::PowderSnow {
                    self.make_stuck([0.9_f32 as f64, 1.5, 0.9_f32 as f64]);
                }
                self.in_powder_snow = true;
            }
            Block::HoneyBlock if self.honey_sliding(pos) => self.slide_on_honey(),
            Block::BubbleColumn if precise => self.bubble_column(world, state, pos),
            _ => {}
        }
    }

    fn visit_box(&mut self, world: &impl Blocks, area: Aabb, target: &Aabb, far: bool, seen: &mut HashSet<[i32; 3]>) {
        let low = area.min.map(floor);
        let high = area.max.map(floor);
        for x in low[0]..=high[0] {
            for y in low[1]..=high[1] {
                for z in low[2]..=high[2] {
                    let pos = [x, y, z];
                    if world.block_state(pos).is_air() || !seen.insert(pos) {
                        continue;
                    }
                    let cell = Aabb { min: pos.map(|v| v as f64), max: pos.map(|v| v as f64 + 1.0) };
                    self.entity_inside(world, pos, far || target.intersects(&cell));
                }
            }
        }
    }

    /// `Entity.applyEffectsFromBlocks`. TODO: paths over a block long skip the cells between both ends.
    pub(crate) fn apply_block_effects(&mut self, world: &impl Blocks) {
        if self.on_ground {
            let below = self.on_pos(world, LEGACY_ON_POS);
            if world.block_state(below).block() == Block::SlimeBlock {
                let abs_y = self.delta[1].abs();
                if abs_y < 0.1 && !self.keys.shift {
                    let scale = 0.4 + abs_y * 0.2;
                    self.delta[0] *= scale;
                    self.delta[2] *= scale;
                }
            }
        }
        let mut moves = std::mem::take(&mut self.moves);
        if moves.is_empty() {
            moves.push((self.pos, self.pos, [0.0; 3]));
        }
        let mut seen = HashSet::new();
        for (from, to, original) in moves {
            let mut at = from;
            let order = if original[0].abs() < original[2].abs() { [1, 2, 0] } else { [1, 0, 2] };
            for axis in order {
                if to[axis] != from[axis] {
                    let mut next = at;
                    next[axis] = to[axis];
                    self.visit_step(world, at, next, &mut seen);
                    at = next;
                }
            }
            if length_sqr([to[0] - from[0], to[1] - from[1], to[2] - from[2]]) == 0.0 {
                self.visit_step(world, to, to, &mut seen);
            }
        }
    }

    fn visit_step(&mut self, world: &impl Blocks, from: [f64; 3], to: [f64; 3], seen: &mut HashSet<[i32; 3]>) {
        let height = self.pose.height();
        let target = Aabb::of_entity(to, WIDTH, height).deflated(TARGET_MARGIN);
        let travel = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
        let far = length_sqr(travel) > 0.9999900000002526_f64.powi(2);
        self.visit_box(world, target.moved(travel.map(|v| -v)), &target, far, seen);
        self.visit_box(world, target, &target, far, seen);
    }
}
