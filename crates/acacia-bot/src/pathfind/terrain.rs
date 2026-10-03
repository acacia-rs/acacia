//! What the search knows about a block cell: collision boxes and hazards, and where a player can be.
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
use acacia_world::{Aabb, BlockAccess, BlockFlags, BlockRegistry, BlockState, ChunkView};

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

pub(crate) const UNLOADED: u8 = 1;
pub(crate) const WATER: u8 = 1 << 1;
/// Never enter: lava, fire, cobweb, sweet berry bush, powder snow, campfire, wither rose.
pub(crate) const AVOID: u8 = 1 << 2;
pub(crate) const CLIMB: u8 = 1 << 3;
/// Hurts to stand on (magma).
pub(crate) const HURT_FLOOR: u8 = 1 << 4;
/// Hurts to brush against (cactus, fire, lava).
pub(crate) const DANGER_NEAR: u8 = 1 << 5;

#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub boxes: &'static [Aabb],
    pub flags: u8,
}

impl Cell {
    pub fn has(&self, flags: u8) -> bool {
        self.flags & flags != 0
    }

    pub fn is_water(&self) -> bool {
        self.has(WATER)
    }
}

/// Block-local xz rectangle the player's box covers (or sweeps) inside a cell.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Rect {
    x: (f32, f32),
    z: (f32, f32),
}

impl Rect {
    pub const FOOT: Rect = Rect { x: (0.2, 0.8), z: (0.2, 0.8) };

    /// The footprint stretched to the cell edge on the side of `(dx, dz)`.
    pub fn toward(dx: i32, dz: i32) -> Rect {
        let span = |d: i32| match d {
            1 => (0.2, 1.0),
            -1 => (0.0, 0.8),
            _ => (0.2, 0.8),
        };
        Rect { x: span(dx), z: span(dz) }
    }

    /// The whole cell along a cardinal direction (a lane crossed at full length).
    pub fn lane(dx: i32, dz: i32) -> Rect {
        let span = |d: i32| if d != 0 { (0.0, 1.0) } else { (0.2, 0.8) };
        Rect { x: span(dx), z: span(dz) }
    }

    /// The quarter of a cell on sides `(sx, sz)`, which a diagonal move cuts through.
    pub fn corner(sx: i32, sz: i32) -> Rect {
        let half = |s: i32| if s > 0 { (0.5, 1.0) } else { (0.0, 0.5) };
        Rect { x: half(sx), z: half(sz) }
    }

    fn overlaps(&self, b: &Aabb) -> bool {
        b.min[0] < self.x.1 - EPS && b.max[0] > self.x.0 + EPS && b.min[2] < self.z.1 - EPS && b.max[2] > self.z.0 + EPS
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpotKind {
    Stand,
    Swim,
    Climb,
}

/// A place the player can be, with its feet height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spot {
    pub feet: f32,
    pub kind: SpotKind,
    /// The feet cell holds water.
    pub wet: bool,
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

/// Terrain as the search sees it, memoized per position (one instance per search or tick).
pub struct Terrain<'r, B> {
    blocks: B,
    registry: &'r BlockRegistry,
    cells: RefCell<PosMap<Cell>>,
    spots: RefCell<PosMap<Option<Spot>>>,
}

impl<'r, B: Blocks> Terrain<'r, B> {
    pub fn new(blocks: B, registry: &'r BlockRegistry) -> Self {
        Self { blocks, registry, cells: RefCell::default(), spots: RefCell::default() }
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
        let Some((block, liquid)) = self.blocks.ids(p) else { return Cell { boxes: SOLID, flags: UNLOADED } };
        let Some(state) = self.registry.get(block) else { return Cell { boxes: SOLID, flags: 0 } };
        let liquid = self.registry.get(liquid).map_or(0, |s| state_flags(s) & (WATER | AVOID | DANGER_NEAR));
        Cell { boxes: state.boxes, flags: state_flags(state) | liquid }
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
            return ok.then_some(Spot { feet, kind: SpotKind::Stand, wet });
        }
        let kind = if wet {
            SpotKind::Swim
        } else if here.has(CLIMB) {
            SpotKind::Climb
        } else {
            return None;
        };
        let feet = p[1] as f32;
        self.body_clear(p, Rect::FOOT, feet).then_some(Spot { feet, kind, wet })
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
    pub(crate) fn body_clear(&self, [x, _, z]: BlockPos, rect: Rect, feet: f32) -> bool {
        let top = feet + DEFAULT_PLAYER_HEIGHT;
        // One cell below the feet too: fences and walls reach 1.5 up.
        for y in feet.floor() as i32 - 1..=(top - EPS).floor() as i32 {
            let c = self.cell([x, y, z]);
            let (lo, hi) = (feet - y as f32, top - y as f32);
            if c.has(UNLOADED) || (hi > EPS && lo < 1.0 - EPS && c.has(AVOID)) {
                return false;
            }
            if c.boxes.iter().any(|b| rect.overlaps(b) && b.min[1] < hi - EPS && b.max[1] > lo + EPS) {
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

fn state_flags(s: &BlockState) -> u8 {
    let mut f = 0;
    if s.is_water() {
        f |= WATER;
    }
    if s.is_lava() {
        f |= AVOID | DANGER_NEAR;
    }
    if s.is_climbable() {
        f |= CLIMB;
    }
    if s.flags.intersects(BlockFlags(BlockFlags::COBWEB.0 | BlockFlags::SWEET_BERRY.0 | BlockFlags::POWDER_SNOW.0)) {
        f |= AVOID;
    }
    match s.name.trim_start_matches("minecraft:") {
        "fire" | "soul_fire" => f |= AVOID | DANGER_NEAR,
        "campfire" | "soul_campfire" | "wither_rose" => f |= AVOID,
        "cactus" => f |= DANGER_NEAR,
        "magma" => f |= HURT_FLOOR,
        _ => {}
    }
    f
}
