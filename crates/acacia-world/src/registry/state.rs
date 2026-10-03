use super::mining::Mining;

/// Block-local axis-aligned box, coordinates in `0.0..=1.0` (fences and walls reach `1.5` in y).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Aabb {
    pub const FULL: Aabb = Aabb::new([0.0; 3], [1.0; 3]);

    pub const fn new(min: [f32; 3], max: [f32; 3]) -> Self {
        Aabb { min, max }
    }
}

/// Physics-relevant block traits. Bit values match `tools/rules.mjs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlockFlags(pub u16);

impl BlockFlags {
    pub const AIR: Self = Self(1 << 0);
    /// Collision is exactly one unit cube.
    pub const FULL_CUBE: Self = Self(1 << 1);
    pub const WATER: Self = Self(1 << 2);
    pub const LAVA: Self = Self(1 << 3);
    /// Ladder, vines, cave/twisting/weeping vines (Boar's climbable list; scaffolding is separate).
    pub const CLIMBABLE: Self = Self(1 << 4);
    pub const SCAFFOLDING: Self = Self(1 << 5);
    pub const COBWEB: Self = Self(1 << 6);
    pub const POWDER_SNOW: Self = Self(1 << 7);
    pub const HONEY: Self = Self(1 << 8);
    pub const SLIME: Self = Self(1 << 9);
    /// Boar slows ground movement ×0.55 on soul sand without Soul Speed.
    pub const SOUL_SAND: Self = Self(1 << 10);
    pub const BED: Self = Self(1 << 11);
    pub const SWEET_BERRY: Self = Self(1 << 12);
    pub const BUBBLE_COLUMN: Self = Self(1 << 13);
    /// Bubble column with `drag_down` (magma below) rather than an upward push.
    pub const BUBBLE_DRAG: Self = Self(1 << 14);
    /// Collision depends on the player, so `boxes` is only the usual case: scaffolding (top only when
    /// above and not descending), powder snow (solid with leather boots), bamboo/pointed dripstone
    /// (Boar collides them only vertically).
    pub const DYNAMIC_SHAPE: Self = Self(1 << 15);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

#[derive(Debug, Clone)]
pub struct BlockState {
    /// Namespaced, e.g. `minecraft:stone`.
    pub name: &'static str,
    /// `key=value` pairs sorted by key, comma separated; booleans are `0`/`1`.
    pub properties: &'static str,
    /// Stairs, fences, panes and walls get their connected shape from the state (26.50+).
    pub boxes: &'static [Aabb],
    pub friction: f32,
    pub flags: BlockFlags,
    /// `liquid_depth` for water/lava (0 = source, 8+ = falling), 0 otherwise.
    pub liquid_depth: u8,
    /// Id used when `StartGame.block_network_ids_are_hashes` (BDS): FNV-1a 32 of the LE NBT
    /// `{name, states}`. 0 for custom blocks (not computed).
    pub network_hash: u32,
    pub mining: Mining,
    /// Block light level the state emits, 0..=15.
    pub light_emission: u8,
    /// Extra light lost passing through, on top of 1 per block step; 15 blocks all light, 0 is clear.
    pub light_filter: u8,
}

impl BlockState {
    pub fn is_air(&self) -> bool {
        self.flags.contains(BlockFlags::AIR)
    }

    /// Has any collision box in its usual state.
    pub fn is_solid(&self) -> bool {
        !self.boxes.is_empty()
    }

    pub fn is_full_cube(&self) -> bool {
        self.flags.contains(BlockFlags::FULL_CUBE)
    }

    pub fn is_liquid(&self) -> bool {
        self.flags.intersects(BlockFlags(BlockFlags::WATER.0 | BlockFlags::LAVA.0))
    }

    pub fn is_water(&self) -> bool {
        self.flags.contains(BlockFlags::WATER)
    }

    pub fn is_lava(&self) -> bool {
        self.flags.contains(BlockFlags::LAVA)
    }

    pub fn is_climbable(&self) -> bool {
        self.flags.contains(BlockFlags::CLIMBABLE)
    }

    /// Fluid surface height within the block, Boar's `(8 - level) / 9` (sources and falling = 8/9).
    pub fn fluid_height(&self) -> f32 {
        match self.liquid_depth {
            _ if !self.is_liquid() => 0.0,
            0 | 8.. => 8.0 / 9.0,
            d => (8 - d) as f32 / 9.0,
        }
    }

    /// Multiplier on jump power (Boar: honey 0.6).
    pub fn jump_factor(&self) -> f32 {
        if self.flags.contains(BlockFlags::HONEY) { 0.6 } else { 1.0 }
    }

    /// Stuck-speed multiplier applied to movement while inside the block (Boar `entityInside`).
    /// Powder snow applies only while the player is below its top face.
    pub fn stuck_multiplier(&self) -> Option<[f32; 3]> {
        let f = self.flags;
        if f.contains(BlockFlags::SWEET_BERRY) {
            Some([0.8, 0.75, 0.8])
        } else if f.contains(BlockFlags::POWDER_SNOW) {
            Some([0.9, 1.5, 0.9])
        } else if f.contains(BlockFlags::COBWEB) {
            Some([0.25, 0.05, 0.25])
        } else {
            None
        }
    }

    pub fn property(&self, key: &str) -> Option<&'static str> {
        self.properties
            .split(',')
            .filter_map(|kv| kv.split_once('='))
            .find_map(|(k, v)| (k == key).then_some(v))
    }
}
