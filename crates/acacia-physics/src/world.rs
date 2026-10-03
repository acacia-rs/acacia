//! The world interface the simulator needs. See README.md for the exact contract.

use crate::aabb::Aabb;
use crate::constants::SOUL_SAND_ACCELERATION_FRICTION_MULTIPLIER;
use crate::math::{BlockPos, Vec3};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Bounce {
    #[default]
    None,
    Slime,
    Bed,
}

/// Velocity change applied while the player's box overlaps the block volume.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InsideMovement {
    #[default]
    None,
    SweetBerryBush,
    PowderSnow,
}

/// Input-driven vertical traversal supported by a block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Traversal {
    #[default]
    None,
    Scaffolding,
    PowderSnow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiquidKind {
    Water,
    Lava,
}

/// A liquid in either block layer (waterlogged blocks included).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Liquid {
    pub kind: LiquidKind,
    /// 1..=8, 8 = source. From Bedrock `liquid_depth`: `8 - (liquid_depth & 7)`.
    pub depth: u8,
    /// Bedrock `liquid_depth >= 8`.
    pub falling: bool,
}

impl Liquid {
    pub fn source(kind: LiquidKind) -> Self {
        Self { kind, depth: 8, falling: false }
    }

    /// Surface height inside the cell.
    pub fn height(&self) -> f32 {
        if self.falling { 1.0 } else { (self.depth as f32 + 1.0) / 9.0 }
    }

    pub(crate) fn decay(&self) -> i32 {
        if self.falling { 0 } else { 8 - self.depth as i32 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BubbleColumn {
    pub downward: bool,
    /// Exact surface variant if known; `None` falls back to "air (no liquid) above".
    pub surface: Option<bool>,
}

/// Movement-relevant properties of one block cell (bedsim `block.MovementSemantics` + lookups).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockPhysics {
    /// Plain air (not a liquid, not any other block).
    pub air: bool,
    /// Ground friction: 0.6 default, ice/packed/frosted 0.98, blue ice 0.989, slime/honey 0.8.
    pub friction: f32,
    /// Acceleration-only friction factor (soul sand 1.225000023841858, else 1).
    pub acceleration_friction_multiplier: f32,
    /// Soul Speed cancels `acceleration_friction_multiplier` (soul sand).
    pub soul_speed_neutralizes: bool,
    /// Ladder, vines, cave/twisting/weeping vines.
    pub climbable: bool,
    pub cobweb: bool,
    pub honey: bool,
    pub bounce: Bounce,
    pub inside: InsideMovement,
    pub traversal: Traversal,
    /// Fences, fence gates and walls (support lookup through their raised boxes).
    pub fence_or_wall: bool,
    /// Liquid in this cell from either layer.
    pub liquid: Option<Liquid>,
    /// Bit `1 << face` set when that face is solid for liquid flow (face order: down, up, north(-z), south(+z), west(-x), east(+x)).
    pub solid_faces: u8,
    pub bubble_column: Option<BubbleColumn>,
}

impl BlockPhysics {
    pub const AIR: Self = Self {
        air: true,
        friction: 0.6,
        acceleration_friction_multiplier: 1.0,
        soul_speed_neutralizes: false,
        climbable: false,
        cobweb: false,
        honey: false,
        bounce: Bounce::None,
        inside: InsideMovement::None,
        traversal: Traversal::None,
        fence_or_wall: false,
        liquid: None,
        solid_faces: 0,
        bubble_column: None,
    };

    /// A generic non-air block with default friction and no faces marked solid.
    pub const fn block() -> Self {
        Self { air: false, ..Self::AIR }
    }

    /// A full solid cube (all faces solid) with the given friction.
    pub const fn solid(friction: f32) -> Self {
        Self { friction, solid_faces: 0x3f, ..Self::block() }
    }

    pub const fn soul_sand() -> Self {
        Self {
            acceleration_friction_multiplier: SOUL_SAND_ACCELERATION_FRICTION_MULTIPLIER,
            soul_speed_neutralizes: true,
            ..Self::block()
        }
    }

    pub const fn liquid(liquid: Liquid) -> Self {
        Self { liquid: Some(liquid), ..Self::block() }
    }
}

pub trait WorldView {
    /// Block-local collision boxes of the block at `pos` (block layer 0), appended to `out`.
    fn block_collisions(&self, pos: BlockPos, out: &mut Vec<Aabb>);

    fn block(&self, pos: BlockPos) -> BlockPhysics;

    /// Every collision box (world space) that `Aabb::intersects` `area`, appended to `out`.
    /// The default scans cells x, y, z (y starting one below `area` for 1.5-tall blocks).
    fn collisions(&self, area: &Aabb, out: &mut Vec<Aabb>) {
        let mut local = Vec::new();
        for x in area.min[0].floor() as i32..=area.max[0].floor() as i32 {
            for y in area.min[1].floor() as i32 - 1..=area.max[1].floor() as i32 {
                for z in area.min[2].floor() as i32..=area.max[2].floor() as i32 {
                    local.clear();
                    self.block_collisions([x, y, z], &mut local);
                    let offset: Vec3 = [x as f32, y as f32, z as f32];
                    out.extend(local.iter().map(|b| b.translate(offset)).filter(|b| b.intersects(area)));
                }
            }
        }
    }

    /// Whether every block in `area` is known. Unknown areas freeze movement for the tick.
    fn is_area_loaded(&self, _area: &Aabb) -> bool {
        true
    }
}
