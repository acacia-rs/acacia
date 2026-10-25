//! What Java's particles ask the level, over acacia's Bedrock blocks: collision, the fluid in a
//! cell (`FluidState` amount, falling, height) and the solid faces block effects test.

use acacia_world::BlockState;
use glam::{DVec3, IVec3};

use crate::particles::collide::{Blocks, sweep};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fluid {
    None,
    Water,
    Lava,
}

/// A cell's `FluidState`: Java's amount (8 for sources and falling fluid) and falling flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FluidState {
    pub fluid: Fluid,
    pub amount: u8,
    pub falling: bool,
}

impl FluidState {
    pub const EMPTY: FluidState = FluidState { fluid: Fluid::None, amount: 0, falling: false };

    pub fn is_source(self) -> bool {
        self.fluid != Fluid::None && self.amount == 8 && !self.falling
    }
}

fn of_state(state: &BlockState) -> Option<FluidState> {
    let fluid = match () {
        _ if state.name == "minecraft:bubble_column" => return Some(FluidState { fluid: Fluid::Water, amount: 8, falling: false }),
        _ if state.is_water() => Fluid::Water,
        _ if state.is_lava() => Fluid::Lava,
        _ => return None,
    };
    // Bedrock's liquid_depth: 0 a source, 1..7 flowing (Java's amount 8 - depth), 8+ falling.
    let (amount, falling) = match state.liquid_depth {
        0 => (8, false),
        d @ 1..=7 => (8 - d, false),
        _ => (8, true),
    };
    Some(FluidState { fluid, amount, falling })
}

pub fn fluid(blocks: &impl Blocks, cell: IVec3) -> FluidState {
    let block = blocks.block(cell).and_then(of_state);
    block.or_else(|| blocks.liquid(cell).and_then(of_state)).unwrap_or(FluidState::EMPTY)
}

/// `FluidState.getHeight`: full under more of the same fluid, else `amount / 9`.
pub fn fluid_height(blocks: &impl Blocks, cell: IVec3) -> (Fluid, f64) {
    let here = fluid(blocks, cell);
    if here.fluid == Fluid::None {
        return (Fluid::None, 0.0);
    }
    let covered = fluid(blocks, cell + IVec3::Y).fluid == here.fluid;
    (here.fluid, if covered { 1.0 } else { f64::from(here.amount as f32 / 9.0) })
}

/// `Particle.move`'s `Entity.collideBoundingBox` for a box from `min` to `max`.
pub fn collide(blocks: &impl Blocks, min: DVec3, max: DVec3, delta: DVec3) -> DVec3 {
    sweep(blocks, min, max, delta)
}

/// `getCollisionShape().max(Y, x, z)` at the cell-local column (`x`, `z`), 0 where nothing is.
pub fn collision_top(blocks: &impl Blocks, cell: IVec3, x: f64, z: f64) -> f64 {
    let Some(state) = blocks.block(cell) else { return 0.0 };
    let inside = |lo: f32, hi: f32, v: f64| f64::from(lo) <= v && v <= f64::from(hi);
    state.boxes.iter().filter(|b| inside(b.min[0], b.max[0], x) && inside(b.min[2], b.max[2], z)).map(|b| f64::from(b.max[1])).fold(0.0, f64::max)
}

/// The vertical extent of a cell's collision shape and its x/z bounds, `None` where unloaded.
pub struct Extent {
    pub bottom: f64,
    pub top: f64,
    pub x: [f64; 2],
    pub z: [f64; 2],
}

pub fn extent(blocks: &impl Blocks, cell: IVec3) -> Option<Extent> {
    let state = blocks.block(cell)?;
    if state.boxes.is_empty() {
        return Some(Extent { bottom: 0.0, top: 0.0, x: [0.0; 2], z: [0.0; 2] });
    }
    let fold = |pick: &dyn Fn(&acacia_world::Aabb) -> f32, min: bool| {
        let values = state.boxes.iter().map(|b| f64::from(pick(b)));
        if min { values.fold(f64::INFINITY, f64::min) } else { values.fold(f64::NEG_INFINITY, f64::max) }
    };
    Some(Extent {
        bottom: fold(&|b| b.min[1], true),
        top: fold(&|b| b.max[1], false),
        x: [fold(&|b| b.min[0], true), fold(&|b| b.max[0], false)],
        z: [fold(&|b| b.min[2], true), fold(&|b| b.max[2], false)],
    })
}

/// `isFaceSturdy`, taken as a full collision cube.
pub fn sturdy(blocks: &impl Blocks, cell: IVec3) -> bool {
    blocks.block(cell).is_some_and(BlockState::is_full_cube)
}

/// `isCollisionShapeFullBlock`.
pub fn full_collision(blocks: &impl Blocks, cell: IVec3) -> bool {
    sturdy(blocks, cell)
}

/// `isSolidRender`: a full cube that is not see-through.
pub fn solid_render(blocks: &impl Blocks, cell: IVec3) -> bool {
    const CLEAR: [&str; 6] = ["glass", "leaves", "ice", "slime", "honey_block", "barrier"];
    blocks.block(cell).is_some_and(|b| b.is_full_cube() && !CLEAR.iter().any(|c| b.name.contains(c)))
}

pub fn is_air(blocks: &impl Blocks, cell: IVec3) -> bool {
    blocks.block(cell).is_some_and(BlockState::is_air)
}

fn name(blocks: &impl Blocks, cell: IVec3) -> &'static str {
    blocks.block(cell).map_or("", |b| b.name.strip_prefix("minecraft:").unwrap_or(b.name))
}

/// The `replaceable` block tag in Bedrock names.
fn replaceable(name: &str) -> bool {
    const NAMES: [&str; 22] = [
        "air", "water", "flowing_water", "lava", "flowing_lava", "short_grass", "fern", "deadbush", "bush", "short_dry_grass", "tall_dry_grass",
        "seagrass", "fire", "soul_fire", "snow_layer", "vine", "glow_lichen", "resin_clump", "tall_grass", "large_fern", "structure_void", "bubble_column",
    ];
    const MORE: [&str; 5] = ["warped_roots", "nether_sprouts", "crimson_roots", "leaf_litter", "hanging_roots"];
    NAMES.contains(&name) || MORE.contains(&name) || name.starts_with("light_block")
}

/// `FallingBlock.isFree`: air, fire, a liquid or a replaceable block.
pub fn free(blocks: &impl Blocks, cell: IVec3) -> bool {
    blocks.block(cell).is_some_and(|b| b.is_air() || b.is_liquid()) || fluid(blocks, cell).fluid != Fluid::None || replaceable(name(blocks, cell))
}

/// The `impermeable` block tag: glass of every colour and barriers.
pub fn impermeable(blocks: &impl Blocks, cell: IVec3) -> bool {
    let name = name(blocks, cell);
    (name.ends_with("glass") && !name.ends_with("pane")) || name == "barrier"
}

/// `EnchantingTableBlock.isValidBookShelf`: a bookshelf with nothing solid between.
pub fn bookshelf_reaches(blocks: &impl Blocks, table: IVec3, offset: IVec3) -> bool {
    let between = table + IVec3::new(offset.x / 2, offset.y, offset.z / 2);
    name(blocks, table + offset) == "bookshelf" && replaceable(name(blocks, between))
}

/// Whether `FireBlock` can burn a block (`igniteOdds > 0`), by name.
// TODO: FireBlock.bootStrap's exact list; this covers the common flammables.
pub fn flammable(blocks: &impl Blocks, cell: IVec3) -> bool {
    const PARTS: [&str; 14] =
        ["planks", "_log", "_wood", "_stem", "hyphae", "leaves", "wool", "carpet", "bookshelf", "tnt", "hay_block", "vine", "bamboo", "scaffolding"];
    let name = name(blocks, cell);
    let stone = name.starts_with("crimson") || name.starts_with("warped");
    (!stone && PARTS.iter().any(|p| name.contains(p))) || ["short_grass", "tall_grass", "fern", "deadbush", "dried_kelp_block", "coal_block"].contains(&name)
}

/// `Level.isRainingAt`'s sky check: nothing but air up to the build height.
pub fn sky_above(blocks: &impl Blocks, cell: IVec3) -> bool {
    (cell.y..320).all(|y| blocks.block(IVec3::new(cell.x, y, cell.z)).is_none_or(BlockState::is_air))
}

/// `Mth.lerp`.
pub fn lerp(t: f64, from: f64, to: f64) -> f64 {
    from + t * (to - from)
}

/// `Mth.getSeed`; the x product overflows as Java's `int` does.
pub fn seed(x: i32, y: i32, z: i32) -> i64 {
    let s = i64::from(x.wrapping_mul(3_129_871)) ^ i64::from(z).wrapping_mul(116_129_781) ^ i64::from(y);
    s.wrapping_mul(s).wrapping_mul(42_317_861).wrapping_add(s.wrapping_mul(11)) >> 16
}

/// `PointedDripstoneBlock.getFluidAboveStalactite`'s position: above the first block over the
/// hanging column, within 11 blocks of the tip.
pub fn above_stalactite(blocks: &impl Blocks, tip: IVec3) -> Option<IVec3> {
    for i in 1..11 {
        let at = tip + IVec3::Y * i;
        let state = blocks.block(at)?;
        if state.name != "minecraft:pointed_dripstone" {
            return Some(at + IVec3::Y);
        }
        if state.property("hanging") != Some("1") {
            return None;
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    None,
    Side,
    Up,
}

fn wire(blocks: &impl Blocks, cell: IVec3) -> bool {
    name(blocks, cell) == "redstone_wire"
}

/// What a wire connects to, by name (`RedstoneWireBlock.shouldConnectTo`, sides ignored).
fn connects(blocks: &impl Blocks, cell: IVec3) -> bool {
    const PARTS: [&str; 10] = ["redstone", "repeater", "comparator", "observer", "lever", "button", "pressure_plate", "target", "daylight_detector", "lectern"];
    let name = name(blocks, cell);
    PARTS.iter().any(|p| name.contains(p)) && !name.ends_with("_ore") && name != "redstone_lamp"
}

/// Bedrock keeps no wire connections; Java's, as `getConnectionState` derives them, north, east,
/// south, west. A wire joined on one side only reaches across to the other.
pub fn wire_sides(blocks: &impl Blocks, cell: IVec3) -> [Side; 4] {
    let roofed = sturdy(blocks, cell + IVec3::Y);
    let mut sides = [IVec3::NEG_Z, IVec3::X, IVec3::Z, IVec3::NEG_X].map(|h| {
        let next = cell + h;
        if !roofed && wire(blocks, next + IVec3::Y) && sturdy(blocks, next) {
            Side::Up
        } else if connects(blocks, next) || (!sturdy(blocks, next) && wire(blocks, next - IVec3::Y)) {
            Side::Side
        } else {
            Side::None
        }
    });
    if sides.iter().any(|&s| s != Side::None) {
        for i in 0..4 {
            if sides[i] == Side::None && sides[(i + 2) % 4] != Side::None && sides[(i + 1) % 4] == Side::None && sides[(i + 3) % 4] == Side::None {
                sides[i] = Side::Side;
            }
        }
    }
    sides
}
