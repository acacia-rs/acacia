//! One block cell as the search sees it: its collision boxes and traits, and the footprint
//! rectangles the player's box covers inside a cell.

use acacia_world::{Aabb, BlockFlags, BlockState};

use super::terrain::EPS;

pub(crate) const UNLOADED: u16 = 1;
pub(crate) const WATER: u16 = 1 << 1;
/// Never enter: lava, fire, cobweb, sweet berry bush, powder snow, campfire, wither rose.
pub(crate) const AVOID: u16 = 1 << 2;
pub(crate) const CLIMB: u16 = 1 << 3;
/// Hurts to stand on (magma).
pub(crate) const HURT_FLOOR: u16 = 1 << 4;
/// Hurts to brush against (cactus, fire, lava).
pub(crate) const DANGER_NEAR: u16 = 1 << 5;
/// Water or lava in either layer.
pub(crate) const LIQUID: u16 = 1 << 6;
/// Collision is exactly one unit cube.
pub(crate) const FULL: u16 = 1 << 7;
/// Falls when the block under it goes (sand, gravel, concrete powder).
pub(crate) const FALLS: u16 = 1 << 8;
/// A door, fence gate or trapdoor a hand opens (not iron).
pub(crate) const OPENABLE: u16 = 1 << 9;

#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub boxes: &'static [Aabb],
    pub flags: u16,
    /// Layer-0 runtime id (`u32::MAX` when not loaded).
    pub block: u32,
}

impl Cell {
    pub fn has(&self, flags: u16) -> bool {
        self.flags & flags != 0
    }

    pub fn is_water(&self) -> bool {
        self.has(WATER)
    }

    /// A full cube that is safe to stand on or to place against.
    pub(crate) fn is_floor(&self) -> bool {
        self.has(FULL) && !self.has(AVOID | HURT_FLOOR | UNLOADED)
    }

    /// Loaded, without collision, liquid or hazard: a block can be placed into it.
    pub(crate) fn is_open(&self) -> bool {
        self.boxes.is_empty() && !self.has(UNLOADED | LIQUID | AVOID)
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

    pub fn overlaps(&self, b: &Aabb) -> bool {
        b.min[0] < self.x.1 - EPS && b.max[0] > self.x.0 + EPS && b.min[2] < self.z.1 - EPS && b.max[2] > self.z.0 + EPS
    }
}

pub(crate) fn state_flags(s: &BlockState) -> u16 {
    let mut f = 0;
    if s.is_water() {
        f |= WATER;
    }
    if s.is_liquid() {
        f |= LIQUID;
    }
    if s.is_lava() {
        f |= AVOID | DANGER_NEAR;
    }
    if s.is_climbable() {
        f |= CLIMB;
    }
    if s.is_full_cube() {
        f |= FULL;
    }
    if s.flags.intersects(BlockFlags(BlockFlags::COBWEB.0 | BlockFlags::SWEET_BERRY.0 | BlockFlags::POWDER_SNOW.0)) {
        f |= AVOID;
    }
    let name = s.name.trim_start_matches("minecraft:");
    match name {
        "fire" | "soul_fire" => f |= AVOID | DANGER_NEAR,
        "campfire" | "soul_campfire" | "wither_rose" => f |= AVOID,
        "cactus" => f |= DANGER_NEAR,
        "magma" => f |= HURT_FLOOR,
        "sand" | "red_sand" | "gravel" | "suspicious_sand" | "suspicious_gravel" => f |= FALLS,
        _ if name.ends_with("concrete_powder") => f |= FALLS,
        _ if (name.ends_with("door") || name.ends_with("fence_gate")) && !name.starts_with("iron_") => f |= OPENABLE,
        _ => {}
    }
    f
}
