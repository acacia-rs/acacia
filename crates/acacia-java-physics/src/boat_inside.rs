//! The driven boat's block effects (`AbstractBoat.tick` runs `applyEffectsFromBlocks` twice) and the server's
//! bubble-column events.

use acacia_registry::Block;

use crate::boat::{Boat, HEIGHT, WIDTH};
use crate::collide::Blocks;
use crate::inside::{inside_bubble_column, nothing_above, InsideBody};
use crate::support::LEGACY_ON_POS;

impl Boat {
    /// Both calls: the first follows this tick's moves (or old → new position), the second old → new in one step.
    pub(crate) fn apply_block_effects(&mut self, world: &impl Blocks) {
        let mut moves = std::mem::take(&mut self.moves);
        let direct = (self.old_pos, self.pos, [0.0; 3]);
        if moves.is_empty() {
            moves.push(direct);
        }
        self.apply_effects_from_blocks(world, &moves);
        self.apply_effects_from_blocks(world, &[direct]);
    }

    /// `handleBubbleColumnEffect` for the server's entity event 71 (dragged down; it also ejects the rider) or 72.
    pub fn bubble_column_effect(&mut self, down: bool) {
        if down {
            self.delta[1] += -0.7;
        } else {
            self.delta[1] = 0.6;
        }
    }
}

impl InsideBody for Boat {
    fn feet(&self) -> [f64; 3] {
        self.pos
    }

    fn velocity(&mut self) -> &mut [f64; 3] {
        &mut self.delta
    }

    fn size(&self) -> (f32, f32) {
        (WIDTH, HEIGHT)
    }

    fn grounded(&self) -> bool {
        self.on_ground
    }

    fn stepping_carefully(&self) -> bool {
        false
    }

    fn effect_pos(&self, world: &impl Blocks) -> [i32; 3] {
        self.support.on_pos(world, self.pos, LEGACY_ON_POS)
    }

    /// Webs and powder snow hold any non-living entity; above a bubble column only the server acts (events 71/72).
    fn entity_inside(&mut self, world: &impl Blocks, pos: [i32; 3], precise: bool) {
        let state = world.block_state(pos);
        match state.block() {
            Block::Cobweb => self.stuck = [0.25, 0.05_f32 as f64, 0.25],
            Block::PowderSnow => self.stuck = [0.9_f32 as f64, 1.5, 0.9_f32 as f64],
            Block::HoneyBlock => {
                self.slide_on_honey(pos);
            }
            Block::BubbleColumn if precise && !nothing_above(world, pos) => inside_bubble_column(&mut self.delta, state),
            _ => {}
        }
    }
}
