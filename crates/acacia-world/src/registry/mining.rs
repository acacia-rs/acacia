//! Block-breaking data (minecraft-data bedrock `blocks.json`) and tool identification. The break-time
//! formula itself lives with the bot; this is only what it needs to know about blocks and tools.

/// Which tools a block responds to. Discriminants match `tools/mining.mjs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Material {
    /// No tool speeds it up (glass, sapling, bedrock...).
    #[default]
    Default = 0,
    Pickaxe = 1,
    Shovel = 2,
    Axe = 3,
    Hoe = 4,
    /// Hoe; shears ×15; sword ×1.5.
    Leaves = 5,
    /// Axe; sword ×1.5.
    Plant = 6,
    /// Axe (pumpkin, melon); sword ×1.5.
    Gourd = 7,
    /// Axe; shears ×2; sword ×1.5.
    Vine = 8,
    /// Sword and shears ×15.
    Cobweb = 9,
    /// Shears ×5.
    Wool = 10,
    /// Swords break it instantly (bamboo).
    SwordInstant = 11,
}

impl Material {
    pub(crate) fn from_u8(v: u8) -> Material {
        use Material::*;
        [Default, Pickaxe, Shovel, Axe, Hoe, Leaves, Plant, Gourd, Vine, Cobweb, Wool, SwordInstant]
            .get(usize::from(v))
            .copied()
            .unwrap_or(Default)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolKind {
    Sword = 0,
    Shovel = 1,
    Pickaxe = 2,
    Axe = 3,
    Hoe = 4,
    Shears = 5,
}

impl ToolKind {
    pub const fn bit(self) -> u8 {
        1 << self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolTier {
    Wood,
    Gold,
    Stone,
    Copper,
    Iron,
    Diamond,
    Netherite,
}

impl ToolTier {
    /// Which blocks it can harvest: wood/gold 0, stone/copper 1, iron 2, diamond 3, netherite 4.
    pub const fn level(self) -> u8 {
        match self {
            ToolTier::Wood | ToolTier::Gold => 0,
            ToolTier::Stone | ToolTier::Copper => 1,
            ToolTier::Iron => 2,
            ToolTier::Diamond => 3,
            ToolTier::Netherite => 4,
        }
    }

    /// Mining speed on blocks of the tool's material (vanilla tier speeds; copper per 1.21.9 / 26.x).
    pub const fn speed(self) -> f32 {
        match self {
            ToolTier::Wood => 2.0,
            ToolTier::Stone => 4.0,
            ToolTier::Copper => 5.0,
            ToolTier::Iron => 6.0,
            ToolTier::Diamond => 8.0,
            ToolTier::Netherite => 9.0,
            ToolTier::Gold => 12.0,
        }
    }

    /// Maximum `Damage` before the tool breaks (dragonfly `item/tool.go`).
    pub const fn durability(self) -> u16 {
        match self {
            ToolTier::Gold => 32,
            ToolTier::Wood => 59,
            ToolTier::Stone => 131,
            ToolTier::Copper => 190,
            ToolTier::Iron => 250,
            ToolTier::Diamond => 1561,
            ToolTier::Netherite => 2031,
        }
    }
}

/// A mining tool; shears have no tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tool {
    pub kind: ToolKind,
    pub tier: Option<ToolTier>,
}

impl Tool {
    /// Maximum `Damage` before the tool breaks; shears 238 (dragonfly `item/shears.go`).
    pub const fn max_durability(self) -> u16 {
        match self.tier {
            Some(tier) => tier.durability(),
            None => 238,
        }
    }

    /// `minecraft:diamond_pickaxe` → diamond pickaxe. `None` for anything that is not a tool.
    pub fn from_identifier(identifier: &str) -> Option<Tool> {
        let name = identifier.strip_prefix("minecraft:").unwrap_or(identifier);
        if name == "shears" {
            return Some(Tool { kind: ToolKind::Shears, tier: None });
        }
        let (tier, kind) = name.split_once('_')?;
        let tier = match tier {
            "wooden" => ToolTier::Wood,
            "golden" => ToolTier::Gold,
            "stone" => ToolTier::Stone,
            "copper" => ToolTier::Copper,
            "iron" => ToolTier::Iron,
            "diamond" => ToolTier::Diamond,
            "netherite" => ToolTier::Netherite,
            _ => return None,
        };
        let kind = match kind {
            "sword" => ToolKind::Sword,
            "shovel" => ToolKind::Shovel,
            "pickaxe" => ToolKind::Pickaxe,
            "axe" => ToolKind::Axe,
            "hoe" => ToolKind::Hoe,
            _ => return None,
        };
        Some(Tool { kind, tier: Some(tier) })
    }
}

/// How a block breaks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mining {
    /// Negative = unbreakable, NaN = unknown (custom blocks, blocks without a data source).
    pub hardness: f32,
    pub material: Material,
    /// [`ToolKind::bit`]s that harvest it (make it drop); 0 = anything, including the hand.
    pub harvest_tools: u8,
    /// Minimum [`ToolTier::level`] of a harvesting tool.
    pub min_level: u8,
}

impl Mining {
    pub const UNKNOWN: Mining = Mining { hardness: f32::NAN, material: Material::Default, harvest_tools: 0, min_level: 0 };

    pub fn is_unbreakable(&self) -> bool {
        self.hardness < 0.0
    }

    /// Whether breaking it with `tool` (`None` = hand or a non-tool item) yields drops, which in
    /// vanilla also picks the ×1.5 rather than ×5 hardness multiplier.
    pub fn can_harvest(&self, tool: Option<Tool>) -> bool {
        if self.harvest_tools == 0 {
            return true;
        }
        let Some(tool) = tool else { return false };
        self.harvest_tools & tool.kind.bit() != 0 && tool.tier.map_or(0, ToolTier::level) >= self.min_level
    }
}
