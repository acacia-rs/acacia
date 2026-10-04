//! Where a player can be, read from block cells ([`Cell`]).
//!
//! A node is a feet block position `p`; the feet rest at `p.y + off`, `off` in `(-0.5, 0.5]`: tops of
//! boxes in the cell below that are above 0.5 (full blocks, soul sand, fences at 1.5) or low boxes
//! in the cell itself (slabs, carpets, snow layers). Boxes count only where they overlap the
//! player's 0.6-wide footprint centred in the cell, so ladders and doors at a cell edge are no floor.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

use acacia_physics::BlockPos;
use acacia_physics::constants::DEFAULT_PLAYER_HEIGHT;
use acacia_world::{Aabb, BlockAccess, BlockRegistry, BlockState, ChunkView};

pub use super::cell::Cell;
use super::cell::{AVOID, DANGER_NEAR, HURT_FLOOR, LIQUID, OPENABLE, Rect, UNLOADED, WATER, state_flags};

pub(crate) const EPS: f32 = 1e-3;

/// Block ids by position for the search.
pub trait Blocks {
    /// (layer 0, layer 1 liquid) runtime ids; `None` when the chunk is not loaded.
    fn ids(&self, pos: BlockPos) -> Option<(u32, u32)>;
}

impl Blocks for ChunkView {
    fn ids(&self, [x, y, z]: BlockPos) -> Option<(u32, u32)> {
        self.chunk(x >> 4, z >> 4)?;
        Some((self.block(x, y, z), self.liquid(x, y, z)))
    }
}

impl<B: Blocks + ?Sized> Blocks for &B {
    fn ids(&self, pos: BlockPos) -> Option<(u32, u32)> {
        (**self).ids(pos)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpotKind {
    Stand,
    Swim,
    Climb,
}

/// The node's own body cells were dug out on the way (the world still shows them).
pub(crate) const CARVED: u8 = 1;
/// The node stands on a block placed on the way (the world still shows air).
pub(crate) const PLACED_FLOOR: u8 = 1 << 1;
/// The door or gate in the node's own cells was toggled on the way.
pub(crate) const OPENED: u8 = 1 << 2;

/// A place the player can be, with its feet height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spot {
    pub feet: f32,
    pub kind: SpotKind,
    /// The feet cell holds water.
    pub wet: bool,
    /// [`CARVED`] / [`PLACED_FLOOR`]: what the path changed to make this spot.
    pub(crate) made: u8,
}

impl Spot {
    pub(crate) fn stand(feet: f32, made: u8) -> Spot {
        Spot { feet, kind: SpotKind::Stand, wet: false, made }
    }
}

#[derive(Default)]
pub(crate) struct PosHasher(u64);

impl Hasher for PosHasher {
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(4) {
            let mut word = [0u8; 4];
            word[..chunk.len()].copy_from_slice(chunk);
            self.0 = (self.0.rotate_left(5) ^ u64::from(u32::from_le_bytes(word))).wrapping_mul(0x517c_c1b7_2722_0a95);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

pub(crate) type PosMap<V> = HashMap<BlockPos, V, BuildHasherDefault<PosHasher>>;

/// The part of a cell a player box covers: `rect` horizontally, `lo..hi` (block-local) vertically.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Span {
    pub rect: Rect,
    pub lo: f32,
    pub hi: f32,
}

impl Span {
    pub fn hits(&self, boxes: &[Aabb]) -> bool {
        boxes.iter().any(|b| self.rect.overlaps(b) && b.min[1] < self.hi - EPS && b.max[1] > self.lo + EPS)
    }
}

/// Terrain as the search sees it, memoized per position (one instance per search or tick).
pub struct Terrain<'r, B> {
    blocks: B,
    registry: &'r BlockRegistry,
    cells: RefCell<PosMap<Cell>>,
    spots: RefCell<PosMap<Option<Spot>>>,
    toggled: RefCell<HashMap<u32, Option<u32>>>,
}

impl<'r, B: Blocks> Terrain<'r, B> {
    pub fn new(blocks: B, registry: &'r BlockRegistry) -> Self {
        Self { blocks, registry, cells: RefCell::default(), spots: RefCell::default(), toggled: RefCell::default() }
    }

    pub fn cell(&self, p: BlockPos) -> Cell {
        if let Some(c) = self.cells.borrow().get(&p) {
            return *c;
        }
        let c = self.classify(p);
        self.cells.borrow_mut().insert(p, c);
        c
    }

    fn classify(&self, p: BlockPos) -> Cell {
        const SOLID: &[Aabb] = &[Aabb::FULL];
        let Some((block, liquid)) = self.blocks.ids(p) else { return Cell { boxes: SOLID, flags: UNLOADED, block: u32::MAX } };
        let Some(state) = self.registry.get(block) else { return Cell { boxes: SOLID, flags: 0, block } };
        let liquid = self.registry.get(liquid).map_or(0, |s| state_flags(s) & (WATER | LIQUID | AVOID | DANGER_NEAR));
        Cell { boxes: state.boxes, flags: state_flags(state) | liquid, block }
    }

    pub(crate) fn state(&self, p: BlockPos) -> Option<&'r BlockState> {
        self.registry.get(self.cell(p).block)
    }

    /// Boxes of the cell's door, gate or trapdoor once toggled (`open_bit` flipped).
    pub(crate) fn toggled_boxes(&self, block: u32) -> Option<&'r [Aabb]> {
        let id = *self.toggled.borrow_mut().entry(block).or_insert_with(|| {
            let s = self.registry.get(block)?;
            let open = s.property("open_bit")?;
            let flipped = s.properties.replace(&format!("open_bit={open}"), if open == "1" { "open_bit=0" } else { "open_bit=1" });
            self.registry.find(s.name, &flipped)
        });
        Some(self.registry.get(id?)?.boxes)
    }

    /// Where the player can be with feet in cell `p`, if anywhere.
    pub fn spot(&self, p: BlockPos) -> Option<Spot> {
        if let Some(s) = self.spots.borrow().get(&p) {
            return *s;
        }
        let s = self.compute_spot(p);
        self.spots.borrow_mut().insert(p, s);
        s
    }

    fn compute_spot(&self, p: BlockPos) -> Option<Spot> {
        let here = self.cell(p);
        if here.has(UNLOADED) {
            return None;
        }
        let wet = here.is_water();
        if let Some((feet, floor)) = self.support(p) {
            let ok = !floor.has(AVOID | HURT_FLOOR) && self.body_clear(p, Rect::FOOT, feet);
            return ok.then_some(Spot { feet, kind: SpotKind::Stand, wet, made: 0 });
        }
        let kind = if wet {
            SpotKind::Swim
        } else if here.has(super::cell::CLIMB) {
            SpotKind::Climb
        } else {
            return None;
        };
        let feet = p[1] as f32;
        self.body_clear(p, Rect::FOOT, feet).then_some(Spot { feet, kind, wet, made: 0 })
    }

    /// Feet height on the highest floor box under the footprint, and the cell it belongs to.
    fn support(&self, [x, y, z]: BlockPos) -> Option<(f32, Cell)> {
        let mut best: Option<(f32, Cell)> = None;
        for (dy, lo, hi) in [(-1, 0.5, 1.5), (0, 0.0, 0.5)] {
            let c = self.cell([x, y + dy, z]);
            if c.has(UNLOADED) {
                return None;
            }
            for b in c.boxes.iter().filter(|b| Rect::FOOT.overlaps(b) && b.max[1] > lo + EPS && b.max[1] <= hi + EPS) {
                let feet = (y + dy) as f32 + b.max[1];
                if best.is_none_or(|(f, _)| feet > f) {
                    best = Some((feet, c));
                }
            }
        }
        best
    }

    /// Whether a standing player box over `rect` of cell `p`, feet at `feet`, touches nothing.
    pub(crate) fn body_clear(&self, p: BlockPos, rect: Rect, feet: f32) -> bool {
        self.obstruct(p, rect, feet, |_, _, _| false)
    }

    /// [`Terrain::body_clear`] for the column a node stands in: a carved node's own two cells are
    /// clear whatever the world still shows.
    pub(crate) fn body_clear_from(&self, p: BlockPos, from: Spot, rect: Rect, feet: f32) -> bool {
        self.obstruct_from(p, from, rect, feet, |_, _, _| false)
    }

    pub(crate) fn obstruct_from(&self, p: BlockPos, from: Spot, rect: Rect, feet: f32, mut ok: impl FnMut(BlockPos, Cell, Span) -> bool) -> bool {
        let own = |q: BlockPos| q[1] == p[1] || q[1] == p[1] + 1;
        let (carved, opened) = (from.made & CARVED != 0, from.made & OPENED != 0);
        self.obstruct(p, rect, feet, |q, c, s| {
            let toggled = opened && own(q) && c.has(OPENABLE) && self.toggled_boxes(c.block).is_some_and(|b| !s.hits(b));
            (carved && own(q)) || toggled || ok(q, c, s)
        })
    }

    /// Visits every cell a standing player box over `rect` of column `p` (feet at `feet`) collides
    /// with or must avoid; `ok` decides whether that cell can be dealt with. Unloaded cells fail.
    pub(crate) fn obstruct(&self, [x, _, z]: BlockPos, rect: Rect, feet: f32, mut ok: impl FnMut(BlockPos, Cell, Span) -> bool) -> bool {
        let top = feet + DEFAULT_PLAYER_HEIGHT;
        // One cell below the feet too: fences and walls reach 1.5 up.
        for y in feet.floor() as i32 - 1..=(top - EPS).floor() as i32 {
            let q = [x, y, z];
            let c = self.cell(q);
            let span = Span { rect, lo: feet - y as f32, hi: top - y as f32 };
            if c.has(UNLOADED) {
                return false;
            }
            let avoid = span.hi > EPS && span.lo < 1.0 - EPS && c.has(AVOID);
            if (avoid || span.hits(c.boxes)) && !ok(q, c, span) {
                return false;
            }
        }
        true
    }

    /// Free fall passes through the cell (no collision under the footprint, nothing harmful).
    pub(crate) fn passable(&self, p: BlockPos) -> bool {
        let c = self.cell(p);
        !c.has(UNLOADED | AVOID) && !c.boxes.iter().any(|b| Rect::FOOT.overlaps(b))
    }

    /// A cactus, fire or lava beside the body or the floor at `p`.
    pub(crate) fn danger_near(&self, [x, y, z]: BlockPos) -> bool {
        [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .any(|(dx, dz)| (-1..2).any(|dy| self.cell([x + dx, y + dy, z + dz]).has(DANGER_NEAR)))
    }
}
