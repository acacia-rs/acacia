use acacia_physics::BlockPos;
use acacia_world::neutral::{Block, Item};

use super::{Act, Ctx, Event, Process, Want};
use crate::pathfind::{DEFAULT_SCAFFOLD, Goal};
use crate::ActionError;

/// Eye to cell centre; placing reaches 4.5 to the block clicked, a neighbour of the cell.
const REACH: f32 = 4.0;
/// Holes this far behind are walked back to once nothing else wants control; further ones are given up.
const RETURN: f32 = 16.0;
/// Ticks without another process in control before walking back.
const QUIET_TICKS: u64 = 20;
const MAX_TRIES: u32 = 3;
const RETRY_TICKS: u64 = 10;
/// Player box half width, plus a margin: a block placed into the player is refused.
const BODY: f32 = 0.4;
const BODY_HEIGHT: f32 = 1.9;

struct Hole {
    pos: BlockPos,
    block: String,
    tries: u32,
    next_at: u64,
}

/// Fills the cells paths dig out once the bot is past them (Baritone's `BackfillProcess`): with the block
/// that was there if the bot has it, else a [`substitutes`] block, else a scaffold block. Takes control only
/// briefly, so give it a higher priority than the processes whose digs it fills.
pub struct Backfill {
    holes: Vec<Hole>,
    scaffold: Vec<Item>,
    filled: usize,
    abandoned: Vec<BlockPos>,
    tick: u64,
}

impl Default for Backfill {
    fn default() -> Self {
        Self::with_scaffold(DEFAULT_SCAFFOLD.to_vec())
    }
}

impl Backfill {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fills with any of `scaffold` (in preference order) when the original block is missing.
    pub fn with_scaffold(scaffold: Vec<Item>) -> Self {
        Self { holes: Vec::new(), scaffold, filled: 0, abandoned: Vec::new(), tick: 0 }
    }

    /// Dug cells not yet filled.
    pub fn pending(&self) -> impl Iterator<Item = BlockPos> + '_ {
        self.holes.iter().map(|h| h.pos)
    }

    /// Dug cells found solid again.
    pub fn filled(&self) -> usize {
        self.filled
    }

    /// Dug cells given up on: out of tries, or left too far behind.
    pub fn abandoned(&self) -> &[BlockPos] {
        &self.abandoned
    }

    fn blocks(&self, original: &str) -> Vec<Item> {
        let block = Block::from_name(original);
        let same = block.and_then(|b| Item::from_name(b.name()));
        let swaps = block.map_or(&[][..], substitutes);
        let mut out = Vec::new();
        same.iter().chain(swaps).chain(&self.scaffold).for_each(|&i| if !out.contains(&i) { out.push(i) });
        out
    }

    fn give_up(&mut self, at: usize) {
        let hole = self.holes.swap_remove(at);
        tracing::debug!(pos = ?hole.pos, tries = hole.tries, "backfill gives up a hole");
        self.abandoned.push(hole.pos);
    }
}

impl Process for Backfill {
    fn name(&self) -> &str {
        "backfill"
    }

    fn want(&mut self, cx: &Ctx) -> Want {
        self.tick = cx.tick;
        let before = self.holes.len();
        self.holes.retain(|h| cx.look.solid(h.pos) != Some(true));
        self.filled += before - self.holes.len();
        while let Some(at) = self.holes.iter().position(|h| dist(cx.pos, h.pos) > 2.0 * RETURN) {
            self.give_up(at);
        }
        if !cx.on_ground {
            return Want::Yield;
        }
        let eye = cx.eye();
        let open = |h: &&Hole| h.next_at <= cx.tick && cx.look.solid(h.pos) == Some(false) && !on_path(cx, h.pos) && !in_body(cx, h.pos);
        let nearest = |a: &&Hole, b: &&Hole| dist(eye, a.pos).total_cmp(&dist(eye, b.pos));
        if let Some(h) = self.holes.iter().filter(open).filter(|h| dist(eye, h.pos) <= REACH).min_by(nearest) {
            return Want::Act(Act::Fill { pos: h.pos, blocks: self.blocks(&h.block) });
        }
        match self.holes.iter().filter(open).filter(|h| dist(cx.pos, h.pos) <= RETURN).min_by(nearest) {
            Some(h) if cx.quiet >= QUIET_TICKS => Want::Go(Goal::Near(h.pos, 2.0)),
            _ => Want::Yield,
        }
    }

    fn acted(&mut self, act: &Act, result: &Result<(), ActionError>) {
        let Act::Fill { pos, .. } = act else { return };
        let Some(at) = self.holes.iter().position(|h| h.pos == *pos) else { return };
        let Err(e) = result else { return };
        tracing::debug!(%e, ?pos, "backfill failed; retrying later");
        let hole = &mut self.holes[at];
        (hole.tries, hole.next_at) = (hole.tries + 1, self.tick + RETRY_TICKS);
        if hole.tries >= MAX_TRIES {
            self.give_up(at);
        }
    }

    fn observe(&mut self, event: &Event) {
        let Event::Dug { pos, block } = event else { return };
        if !self.holes.iter().any(|h| h.pos == *pos) {
            self.holes.push(Hole { pos: *pos, block: block.clone(), tries: 0, next_at: 0 });
        }
    }
}

/// What to fill a dug block's cell with when the bot lacks the block (breaking it dropped another).
pub fn substitutes(block: Block) -> &'static [Item] {
    let name = block.name();
    match block {
        Block::Stone => &[Item::Cobblestone],
        Block::Deepslate => &[Item::CobbledDeepslate],
        Block::GrassBlock | Block::Podzol | Block::Mycelium | Block::DirtPath | Block::Farmland => &[Item::Dirt],
        _ if name.starts_with("minecraft:deepslate_") && name.ends_with("_ore") => &[Item::CobbledDeepslate],
        _ if name.ends_with("_ore") => &[Item::Cobblestone],
        _ => &[],
    }
}

/// A node still ahead stands in or passes through the cell.
fn on_path(cx: &Ctx, p: BlockPos) -> bool {
    cx.path.iter().any(|n| n.pos == p || n.pos == [p[0], p[1] - 1, p[2]])
}

fn in_body(cx: &Ctx, p: BlockPos) -> bool {
    let [x, y, z] = cx.pos;
    let overlap = |lo: f32, hi: f32, c: i32| lo < (c + 1) as f32 && hi > c as f32;
    overlap(x - BODY, x + BODY, p[0]) && overlap(y, y + BODY_HEIGHT, p[1]) && overlap(z - BODY, z + BODY, p[2])
}

fn dist(from: acacia_physics::Vec3, p: BlockPos) -> f32 {
    let d = [p[0] as f32 + 0.5 - from[0], p[1] as f32 + 0.5 - from[1], p[2] as f32 + 0.5 - from[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}
