//! Synthetic block grid for tests and examples. Must stay in sync with tools/diffharness/world.go.

use std::collections::HashMap;

use crate::aabb::Aabb;
use crate::math::BlockPos;
use crate::world::{BlockPhysics, Bounce, InsideMovement, Liquid, LiquidKind, Traversal, WorldView};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestBlock {
    Stone,
    /// Bottom slab.
    Slab,
    /// Bottom half plus the +Z half on top.
    Stair,
    /// Ladder hanging on the +Z face of its cell.
    Ladder,
    Ice,
    PackedIce,
    BlueIce,
    SoulSand,
    Slime,
    Honey,
    Web,
    PowderSnow,
    BerryBush,
    Vine,
    Scaffolding,
    Bed,
    Liquid(Liquid),
}

impl TestBlock {
    /// Parses a diff-harness kind: `stone`, `water`, `water@7`, `water_falling`, `lava`, ...
    pub fn from_kind(kind: &str) -> Option<Self> {
        let (base, depth) = match kind.split_once('@') {
            Some((b, d)) => (b, d.parse().ok()?),
            None => (kind, 8),
        };
        let (base, falling) = match base.strip_suffix("_falling") {
            Some(b) => (b, true),
            None => (base, false),
        };
        let liquid = |kind| Some(Self::Liquid(Liquid { kind, depth, falling }));
        Some(match base {
            "water" => return liquid(LiquidKind::Water),
            "lava" => return liquid(LiquidKind::Lava),
            "stone" => Self::Stone,
            "slab" => Self::Slab,
            "stair" => Self::Stair,
            "ladder" => Self::Ladder,
            "ice" => Self::Ice,
            "packed_ice" => Self::PackedIce,
            "blue_ice" => Self::BlueIce,
            "soul_sand" => Self::SoulSand,
            "slime" => Self::Slime,
            "honey" => Self::Honey,
            "web" => Self::Web,
            "powder_snow" => Self::PowderSnow,
            "berry_bush" => Self::BerryBush,
            "vine" => Self::Vine,
            "scaffolding" => Self::Scaffolding,
            "bed" => Self::Bed,
            _ => return None,
        })
    }

    pub fn boxes(&self) -> &'static [Aabb] {
        const fn b(x0: f32, y0: f32, z0: f32, x1: f32, y1: f32, z1: f32) -> Aabb {
            Aabb { min: [x0, y0, z0], max: [x1, y1, z1] }
        }
        const FULL: &[Aabb] = &[b(0.0, 0.0, 0.0, 1.0, 1.0, 1.0)];
        const SLAB: &[Aabb] = &[b(0.0, 0.0, 0.0, 1.0, 0.5, 1.0)];
        const STAIR: &[Aabb] = &[b(0.0, 0.0, 0.0, 1.0, 0.5, 1.0), b(0.0, 0.5, 0.5, 1.0, 1.0, 1.0)];
        const LADDER: &[Aabb] = &[b(0.0, 0.0, 0.8125, 1.0, 1.0, 1.0)];
        const SOUL_SAND: &[Aabb] = &[b(0.0, 0.0, 0.0, 1.0, 0.875, 1.0)];
        const HONEY: &[Aabb] = &[b(0.0625, 0.0, 0.0625, 0.9375, 0.9375, 0.9375)];
        const BED: &[Aabb] = &[b(0.0, 0.0, 0.0, 1.0, 0.5625, 1.0)];
        match self {
            Self::Stone | Self::Ice | Self::PackedIce | Self::BlueIce | Self::Slime => FULL,
            Self::Slab => SLAB,
            Self::Stair => STAIR,
            Self::Ladder => LADDER,
            Self::SoulSand => SOUL_SAND,
            Self::Honey => HONEY,
            Self::Bed => BED,
            Self::Web | Self::PowderSnow | Self::BerryBush | Self::Vine | Self::Scaffolding | Self::Liquid(_) => &[],
        }
    }

    pub fn physics(&self) -> BlockPhysics {
        let block = BlockPhysics::block();
        match *self {
            Self::Stone => BlockPhysics::solid(0.6),
            Self::Ice | Self::PackedIce => BlockPhysics::solid(0.98),
            Self::BlueIce => BlockPhysics::solid(0.989),
            Self::Slime => BlockPhysics { bounce: Bounce::Slime, ..BlockPhysics::solid(0.8) },
            Self::Slab | Self::Stair => block,
            Self::Ladder | Self::Vine => BlockPhysics { climbable: true, ..block },
            Self::SoulSand => BlockPhysics::soul_sand(),
            Self::Honey => BlockPhysics { honey: true, friction: 0.8, ..block },
            Self::Web => BlockPhysics { cobweb: true, ..block },
            Self::PowderSnow => {
                BlockPhysics { inside: InsideMovement::PowderSnow, traversal: Traversal::PowderSnow, ..block }
            }
            Self::BerryBush => BlockPhysics { inside: InsideMovement::SweetBerryBush, ..block },
            Self::Scaffolding => BlockPhysics { traversal: Traversal::Scaffolding, ..block },
            Self::Bed => BlockPhysics { bounce: Bounce::Bed, ..block },
            Self::Liquid(l) => BlockPhysics::liquid(l),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TestWorld {
    cells: HashMap<BlockPos, TestBlock>,
}

impl TestWorld {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fills the inclusive cuboid `min..=max`; `None` clears it to air.
    pub fn fill(&mut self, block: Option<TestBlock>, min: BlockPos, max: BlockPos) -> &mut Self {
        for x in min[0]..=max[0] {
            for y in min[1]..=max[1] {
                for z in min[2]..=max[2] {
                    match block {
                        Some(b) => self.cells.insert([x, y, z], b),
                        None => self.cells.remove(&[x, y, z]),
                    };
                }
            }
        }
        self
    }

    pub fn set(&mut self, pos: BlockPos, block: TestBlock) -> &mut Self {
        self.cells.insert(pos, block);
        self
    }
}

impl WorldView for TestWorld {
    fn block_collisions(&self, pos: BlockPos, out: &mut Vec<Aabb>) {
        if let Some(b) = self.cells.get(&pos) {
            out.extend_from_slice(b.boxes());
        }
    }

    fn block(&self, pos: BlockPos) -> BlockPhysics {
        self.cells.get(&pos).map_or(BlockPhysics::AIR, TestBlock::physics)
    }
}
