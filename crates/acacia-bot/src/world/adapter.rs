use acacia_physics::constants::SOUL_SAND_ACCELERATION_FRICTION_MULTIPLIER as SOUL_SAND_ACCELERATION;
use acacia_physics::{Aabb, BlockPhysics, BlockPos, Bounce, BubbleColumn, InsideMovement, Liquid, LiquidKind, Traversal, WorldView};
use acacia_world::{BlockAccess, BlockFlags, BlockRegistry, BlockState, ChunkView};

/// The tracked terrain as the physics engine sees it.
pub struct PhysicsWorld<'a> {
    pub view: &'a ChunkView,
    pub registry: &'a BlockRegistry,
}

impl PhysicsWorld<'_> {
    fn state(&self, runtime_id: u32) -> Option<&BlockState> {
        self.registry.get(runtime_id)
    }
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

impl WorldView for PhysicsWorld<'_> {
    fn block_collisions(&self, [x, y, z]: BlockPos, out: &mut Vec<Aabb>) {
        if let Some(s) = self.state(self.view.block(x, y, z)) {
            out.extend(s.boxes.iter().map(|b| Aabb { min: b.min, max: b.max }));
        }
    }

    fn block(&self, [x, y, z]: BlockPos) -> BlockPhysics {
        let Some(s) = self.state(self.view.block(x, y, z)) else { return BlockPhysics::AIR };
        // BDS ignores flowing water in the second layer (spread into a fence or onto soul sand) for movement
        // (fuzz 161505-1); whether it ignores still water there too is untested.
        // A bubble column is water (its layer-2 water is flowing_water, which the rule above would drop).
        let column_water = s.flags.contains(BlockFlags::BUBBLE_COLUMN).then_some(Liquid { kind: LiquidKind::Water, depth: 8, falling: false });
        let liquid = liquid_of(s).or(column_water).or_else(|| {
            self.state(self.view.liquid(x, y, z)).filter(|l| !l.name.ends_with("flowing_water")).and_then(liquid_of)
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

    fn is_area_loaded(&self, area: &Aabb) -> bool {
        let (x0, x1) = ((area.min[0].floor() as i32) >> 4, (area.max[0].floor() as i32) >> 4);
        let (z0, z1) = ((area.min[2].floor() as i32) >> 4, (area.max[2].floor() as i32) >> 4);
        (x0..=x1).all(|cx| (z0..=z1).all(|cz| self.view.chunk(cx, cz).is_some()))
    }
}
