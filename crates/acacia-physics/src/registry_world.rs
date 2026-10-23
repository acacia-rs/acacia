//! [`WorldView`] over blocks named by an acacia-world [`BlockRegistry`]: a client's chunk view or a
//! server's level (feature `acacia-world`).

use acacia_world::{BlockAccess, BlockFlags, BlockRegistry, BlockState, ChunkView};

use crate::constants::SOUL_SAND_ACCELERATION_FRICTION_MULTIPLIER as SOUL_SAND_ACCELERATION;
use crate::{Aabb, BlockPhysics, BlockPos, Bounce, BubbleColumn, InsideMovement, Liquid, LiquidKind, Traversal, WorldView};

/// Blocks by runtime id, and which columns are there at all.
pub trait Terrain: BlockAccess {
    /// Whether the 16x16 column at chunk `(cx, cz)` is loaded; a body in one that is not stays put.
    fn is_column_loaded(&self, cx: i32, cz: i32) -> bool;
}

impl Terrain for ChunkView {
    fn is_column_loaded(&self, cx: i32, cz: i32) -> bool {
        self.chunk(cx, cz).is_some()
    }
}

/// The terrain as the physics engine sees it.
pub struct RegistryWorld<'a, T: ?Sized> {
    pub view: &'a T,
    pub registry: &'a BlockRegistry,
}

fn liquid_of(state: &BlockState) -> Option<Liquid> {
    let kind = if state.is_water() {
        LiquidKind::Water
    } else if state.is_lava() {
        LiquidKind::Lava
    } else {
        return None;
    };
    let d = state.liquid_depth;
    Some(Liquid { kind, depth: 8 - (d & 7), falling: d >= 8 })
}

/// Blocks whose raised collision (1.5 tall) the support lookup must see.
fn is_fence_or_wall(name: &str) -> bool {
    let base = name.trim_start_matches("minecraft:");
    base.ends_with("_fence") || base.ends_with("_fence_gate") || (base.ends_with("_wall") && !base.contains("sign") && !base.contains("banner") && !base.contains("torch") && !base.contains("head") && !base.contains("skull") && !base.contains("fan"))
}

/// A cell's physics from its layer-0 runtime id and, read only when needed, its layer-1 id.
pub fn block_physics(registry: &BlockRegistry, block: u32, liquid_layer: impl FnOnce() -> u32) -> BlockPhysics {
    let Some(s) = registry.get(block) else { return BlockPhysics::AIR };
    // BDS ignores flowing water in the second layer (spread into a fence or onto soul sand) for movement
    // (fuzz 161505-1); whether it ignores still water there too is untested. It ignores any layer-2 water
    // in a full cube (stone filled over a pool) or under air (left by a removed block), which otherwise bends
    // a neighbour's current (strict BDS fuzz).
    // A bubble column is water (its layer-2 water is flowing_water, which the rule above would drop).
    let column_water = s.flags.contains(BlockFlags::BUBBLE_COLUMN).then_some(Liquid { kind: LiquidKind::Water, depth: 8, falling: false });
    let liquid = liquid_of(s).or(column_water).or_else(|| {
        (!s.is_full_cube() && !s.is_air()).then(|| registry.get(liquid_layer())).flatten().filter(|l| !l.name.ends_with("flowing_water")).and_then(liquid_of)
    });
    if s.is_air() && liquid.is_none() {
        return BlockPhysics::AIR;
    }
    let f = s.flags;
    let has = |flag: BlockFlags| f.contains(flag);
    let soul_sand = has(BlockFlags::SOUL_SAND);
    BlockPhysics {
        air: false,
        friction: if s.friction > 0.0 { s.friction } else { 0.6 },
        acceleration_friction_multiplier: if soul_sand { SOUL_SAND_ACCELERATION } else { 1.0 },
        soul_speed_neutralizes: soul_sand,
        climbable: has(BlockFlags::CLIMBABLE),
        cobweb: has(BlockFlags::COBWEB),
        honey: has(BlockFlags::HONEY),
        bounce: if has(BlockFlags::SLIME) { Bounce::Slime } else if has(BlockFlags::BED) { Bounce::Bed } else { Bounce::None },
        inside: if has(BlockFlags::SWEET_BERRY) {
            InsideMovement::SweetBerryBush
        } else if has(BlockFlags::POWDER_SNOW) {
            InsideMovement::PowderSnow
        } else {
            InsideMovement::None
        },
        traversal: if has(BlockFlags::SCAFFOLDING) {
            Traversal::Scaffolding
        } else if has(BlockFlags::POWDER_SNOW) {
            Traversal::PowderSnow
        } else {
            Traversal::None
        },
        fence_or_wall: is_fence_or_wall(s.name),
        liquid,
        // Approximation for liquid flow: only full cubes block it on every face.
        solid_faces: if s.is_full_cube() { 0x3f } else { 0 },
        bubble_column: has(BlockFlags::BUBBLE_COLUMN).then(|| BubbleColumn { downward: has(BlockFlags::BUBBLE_DRAG), surface: None }),
    }
}

impl<T: Terrain + ?Sized> WorldView for RegistryWorld<'_, T> {
    fn block_collisions(&self, [x, y, z]: BlockPos, out: &mut Vec<Aabb>) {
        if let Some(s) = self.registry.get(self.view.block(x, y, z)) {
            out.extend(s.boxes.iter().map(|b| Aabb { min: b.min, max: b.max }));
        }
    }

    fn block(&self, [x, y, z]: BlockPos) -> BlockPhysics {
        block_physics(self.registry, self.view.block(x, y, z), || self.view.liquid(x, y, z))
    }

    fn is_area_loaded(&self, area: &Aabb) -> bool {
        let (min, max) = (crate::math::block_pos(area.min), crate::math::block_pos(area.max));
        let (x0, x1, z0, z1) = (min[0] >> 4, max[0] >> 4, min[2] >> 4, max[2] >> 4);
        (x0..=x1).all(|cx| (z0..=z1).all(|cz| self.view.is_column_loaded(cx, cz)))
    }
}
