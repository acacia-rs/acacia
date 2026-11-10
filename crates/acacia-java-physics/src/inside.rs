//! `Entity.applyEffectsFromBlocks`, shared by the player and the boat it drives.

use std::collections::HashSet;

use acacia_registry::prop::Drag;
use acacia_registry::{Block, BlockState, BlockTag};

use crate::aabb::Aabb;
use crate::block_path::for_each_cell_between;
use crate::collide::Blocks;
use crate::fluid::fluid_of;
use crate::mth::{floor, length_sqr};
use crate::player::{Player, WIDTH};
use crate::support::LEGACY_ON_POS;

const TARGET_MARGIN: f64 = 1.0E-5_f32 as f64;

/// `Entity.Movement`: from, to, and the movement asked for; a zero ask visits the path in one step.
pub(crate) type Move = ([f64; 3], [f64; 3], [f64; 3]);

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

/// `BubbleColumnBlock.entityInside`'s test: nothing collides or flows above the column.
pub(crate) fn nothing_above(world: &impl Blocks, pos: [i32; 3]) -> bool {
    let above = world.block_state([pos[0], pos[1] + 1, pos[2]]);
    above.java_collision().is_some_and(<[_]>::is_empty) && fluid_of(above).is_none()
}

/// `Entity.handleOnInsideBubbleColumn`.
pub(crate) fn inside_bubble_column(delta: &mut [f64; 3], state: BlockState) {
    delta[1] = if state.get(Drag) == Some(true) { (-0.3_f64).max(delta[1] - 0.03) } else { 0.7_f64.min(delta[1] + 0.06) };
}

/// What `applyEffectsFromBlocks` reads and changes on a body.
pub(crate) trait InsideBody {
    fn feet(&self) -> [f64; 3];
    fn velocity(&mut self) -> &mut [f64; 3];
    /// Width and height.
    fn size(&self) -> (f32, f32);
    fn grounded(&self) -> bool;
    fn stepping_carefully(&self) -> bool;
    /// `getOnPosLegacy`.
    fn effect_pos(&self, world: &impl Blocks) -> [i32; 3];
    /// `Block.entityInside` for the blocks that move this body.
    fn entity_inside(&mut self, world: &impl Blocks, pos: [i32; 3], precise: bool);

    /// `HoneyBlock.isSlidingDown`, then `doSlideMovement`; true when it slid.
    fn slide_on_honey(&mut self, pos: [i32; 3]) -> bool {
        let (feet, width, grounded) = (self.feet(), self.size().0, self.grounded());
        let delta = self.velocity();
        let old = old_delta_y(delta[1]);
        if grounded || feet[1] > pos[1] as f64 + 0.9375 - 1.0E-7 || old >= -0.08 {
            return false;
        }
        let overlap = 0.4375 + (width / 2.0) as f64;
        let dx = (pos[0] as f64 + 0.5 - feet[0]).abs();
        let dz = (pos[2] as f64 + 0.5 - feet[2]).abs();
        if dx + 1.0E-7 <= overlap && dz + 1.0E-7 <= overlap {
            return false;
        }
        if old < -0.13 {
            let reduce = -0.05 / old;
            delta[0] *= reduce;
            delta[2] *= reduce;
        }
        delta[1] = new_delta_y(-0.05);
        true
    }

    /// `applyEffectsFromBlocks(movements)`: slime's `stepOn`, then every block the moves pass through.
    /// TODO: blocks with their own inside shape (cauldrons, tripwire, end portals, falling into powder snow) count
    /// as full cubes, and fluids' own `entityInside` isn't run.
    fn apply_effects_from_blocks(&mut self, world: &impl Blocks, moves: &[Move])
    where
        Self: Sized,
    {
        if self.grounded() && world.block_state(self.effect_pos(world)).block() == Block::SlimeBlock {
            let careful = self.stepping_carefully();
            let delta = self.velocity();
            let abs_y = delta[1].abs();
            if abs_y < 0.1 && !careful {
                let scale = 0.4 + abs_y * 0.2;
                delta[0] *= scale;
                delta[2] *= scale;
            }
        }
        let mut seen = HashSet::new();
        for &(from, to, original) in moves {
            let travel = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
            let mut budget = MAX_ITERATIONS;
            if original == [0.0; 3] || length_sqr(travel) == 0.0 {
                budget -= visit_step(self, world, from, to, &mut seen, MAX_ITERATIONS);
            } else {
                let mut at = from;
                let order = if original[0].abs() < original[2].abs() { [1, 2, 0] } else { [1, 0, 2] };
                for axis in order.into_iter().filter(|&a| travel[a] != 0.0) {
                    let mut next = at;
                    next[axis] = to[axis];
                    budget -= visit_step(self, world, at, next, &mut seen, budget);
                    at = next;
                }
            }
            if budget <= 0 {
                visit_step(self, world, to, to, &mut seen, 1);
            }
        }
    }
}

/// `checkInsideBlocks` for one move.
const MAX_ITERATIONS: i32 = 16;

/// `checkInsideBlocks(from, to, ..., budget)`: returns the iterations it used.
fn visit_step(body: &mut impl InsideBody, world: &impl Blocks, from: [f64; 3], to: [f64; 3], seen: &mut HashSet<[i32; 3]>, budget: i32) -> i32 {
    let (width, height) = body.size();
    let target = Aabb::of_entity(to, width, height).deflated(TARGET_MARGIN);
    let travel = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    let far = length_sqr(travel) > 0.9999900000002526_f64.powi(2);
    let mut last = 0;
    for_each_cell_between(from, to, &target, |pos, iteration| {
        if iteration as i32 >= budget {
            return false;
        }
        last = iteration as i32;
        if !world.block_state(pos).is_air() && seen.insert(pos) {
            let cell = Aabb { min: pos.map(f64::from), max: pos.map(|v| f64::from(v) + 1.0) };
            body.entity_inside(world, pos, far || target.intersects(&cell));
        }
        true
    });
    last + 1
}

impl Player {
    fn make_stuck(&mut self, speed: [f64; 3]) {
        self.fall_distance = 0.0;
        self.stuck = speed;
    }

    /// `Entity.applyEffectsFromBlocks()` after the tick's moves.
    pub(crate) fn apply_block_effects(&mut self, world: &impl Blocks) {
        let mut moves = std::mem::take(&mut self.moves);
        if moves.is_empty() {
            moves.push((self.pos, self.pos, [0.0; 3]));
        }
        self.apply_effects_from_blocks(world, &moves);
    }
}

impl InsideBody for Player {
    fn feet(&self) -> [f64; 3] {
        self.pos
    }

    fn velocity(&mut self) -> &mut [f64; 3] {
        &mut self.delta
    }

    fn size(&self) -> (f32, f32) {
        (WIDTH, self.pose.height())
    }

    fn grounded(&self) -> bool {
        self.on_ground
    }

    fn stepping_carefully(&self) -> bool {
        self.keys.shift
    }

    fn effect_pos(&self, world: &impl Blocks) -> [i32; 3] {
        self.on_pos(world, LEGACY_ON_POS)
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
            Block::HoneyBlock => {
                if self.slide_on_honey(pos) {
                    self.fall_distance = 0.0;
                }
            }
            Block::BubbleColumn if precise => {
                if nothing_above(world, pos) {
                    let y = self.delta[1];
                    self.delta[1] = if state.get(Drag) == Some(true) { (-0.9_f64).max(y - 0.03) } else { 1.8_f64.min(y + 0.1) };
                } else {
                    inside_bubble_column(&mut self.delta, state);
                }
                self.fall_distance = 0.0;
            }
            _ => {}
        }
    }
}
