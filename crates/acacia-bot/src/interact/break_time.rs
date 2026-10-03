//! Vanilla block-breaking time.
//!
//! Bedrock semantics (Dragonfly `server/block/break_info.go` `BreakDuration`, PocketMine
//! `BlockBreakInfo::getBreakTime`): the effective tool's tier speed (1 by hand, for the wrong tool,
//! or for a tool below the block's harvest tier), + level² + 1 for Efficiency, × (1 + 0.2 × Haste),
//! × Mining Fatigue (0.3 / 0.09 / 0.0027 / 0.00081, Java and Geyser), ÷ 5 under water without Aqua
//! Affinity, ÷ 5 off the ground. Damage per tick = speed / hardness / (30 if harvestable else 100);
//! the block breaks after ceil(1 / damage) ticks. Sword and shears speeds follow Java 1.21
//! (cobweb 15, leaves/wool/vines 15/5/2 for shears, 1.5 for swords on plants), which is never
//! faster than Bedrock. Geyser checks progress with Java semantics (≥ 0.65 at `PredictBreak`),
//! which is equal to or faster than this, so these times pass there too.

use acacia_world::{Material, Mining, Tool, ToolKind, ToolTier};

/// Everything about the player that changes how fast it breaks blocks.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BreakConditions {
    /// Held tool; `None` for the hand or any item that is not a tool.
    pub tool: Option<Tool>,
    /// Efficiency level of the held tool.
    pub efficiency: u8,
    /// Haste (or Conduit Power) level, 0 when absent.
    pub haste: u8,
    pub mining_fatigue: u8,
    /// Eyes in water without Aqua Affinity.
    pub underwater: bool,
    pub on_ground: bool,
}

/// Ticks from `StartBreak` until the block breaks (1 = instantly); `None` if it cannot be broken
/// or its hardness is unknown (custom blocks).
pub fn break_ticks(mining: &Mining, c: &BreakConditions) -> Option<u32> {
    let hardness = mining.hardness;
    if hardness.is_nan() || mining.is_unbreakable() {
        return None;
    }
    let harvestable = mining.can_harvest(c.tool);
    let Some(mut speed) = tool_speed(mining.material, c.tool) else { return Some(1) };
    if !harvestable {
        speed = 1.0;
    }
    if speed > 1.0 && c.efficiency > 0 {
        speed += f32::from(c.efficiency).powi(2) + 1.0;
    }
    speed *= 1.0 + 0.2 * f32::from(c.haste);
    speed *= match c.mining_fatigue {
        0 => 1.0,
        1 => 0.3,
        2 => 0.09,
        3 => 0.0027,
        _ => 0.00081,
    };
    if c.underwater {
        speed /= 5.0;
    }
    if !c.on_ground {
        speed /= 5.0;
    }
    if hardness == 0.0 {
        return Some(1);
    }
    let ticks = f64::from(hardness) * if harvestable { 30.0 } else { 100.0 } / f64::from(speed);
    // Hardnesses like 0.6 are not exact in f32; the epsilon keeps 18.0000007 from rounding up to 19.
    Some((ticks - 1e-4).ceil().max(1.0) as u32)
}

/// Mining speed of `tool` on `material`, `None` when it breaks the block instantly.
fn tool_speed(material: Material, tool: Option<Tool>) -> Option<f32> {
    use Material as M;
    use ToolKind as K;
    let Some(tool) = tool else { return Some(1.0) };
    let tier = tool.tier.map_or(1.0, ToolTier::speed);
    Some(match (material, tool.kind) {
        (M::SwordInstant, K::Sword) => return None,
        (M::Cobweb, K::Sword | K::Shears) | (M::Leaves, K::Shears) => 15.0,
        (M::Wool, K::Shears) => 5.0,
        (M::Vine, K::Shears) => 2.0,
        (M::Leaves | M::Plant | M::Gourd | M::Vine, K::Sword) => 1.5,
        (M::Pickaxe, K::Pickaxe)
        | (M::Shovel, K::Shovel)
        | (M::Axe | M::Plant | M::Gourd | M::Vine, K::Axe)
        | (M::Hoe | M::Leaves, K::Hoe) => tier,
        _ => 1.0,
    })
}

#[cfg(test)]
mod tests {
    use acacia_world::BlockRegistry;

    use super::*;

    fn mining(name: &str) -> Mining {
        BlockRegistry::vanilla().states_of(name).next().unwrap().1.mining
    }

    fn ticks(block: &str, tool: &str, tweak: impl FnOnce(&mut BreakConditions)) -> Option<u32> {
        let mut c = BreakConditions { tool: Tool::from_identifier(tool), on_ground: true, ..BreakConditions::default() };
        tweak(&mut c);
        break_ticks(&mining(block), &c)
    }

    #[test]
    fn vanilla_times() {
        // Bedrock wiki / in-game: stone by hand 7.5 s, wooden pickaxe 1.15 s; dirt by hand 0.75 s;
        // obsidian with a diamond pickaxe 9.4 s.
        assert_eq!(ticks("minecraft:stone", "", |_| {}), Some(150));
        assert_eq!(ticks("minecraft:stone", "minecraft:wooden_pickaxe", |_| {}), Some(23));
        assert_eq!(ticks("minecraft:dirt", "", |_| {}), Some(15));
        assert_eq!(ticks("minecraft:obsidian", "minecraft:diamond_pickaxe", |_| {}), Some(188));
        assert_eq!(ticks("minecraft:oak_log", "minecraft:stone_axe", |_| {}), Some(15));
        assert_eq!(ticks("minecraft:dirt", "minecraft:iron_shovel", |_| {}), Some(3));
    }

    #[test]
    fn harvest_tier_and_wrong_tool() {
        // Wooden pickaxe cannot harvest iron ore: no tier bonus and the ×100 divisor (Bedrock).
        assert_eq!(ticks("minecraft:iron_ore", "minecraft:wooden_pickaxe", |_| {}), Some(300));
        assert_eq!(ticks("minecraft:iron_ore", "minecraft:stone_pickaxe", |_| {}), Some(23));
        assert_eq!(ticks("minecraft:stone", "minecraft:diamond_shovel", |_| {}), Some(150));
        assert_eq!(ticks("minecraft:stone", "minecraft:apple", |_| {}), Some(150));
    }

    #[test]
    fn modifiers() {
        assert_eq!(ticks("minecraft:stone", "minecraft:diamond_pickaxe", |c| c.efficiency = 5), Some(2));
        assert_eq!(ticks("minecraft:stone", "minecraft:wooden_pickaxe", |c| c.on_ground = false), Some(113));
        assert_eq!(ticks("minecraft:stone", "minecraft:wooden_pickaxe", |c| c.underwater = true), Some(113));
        assert_eq!(ticks("minecraft:stone", "minecraft:wooden_pickaxe", |c| c.haste = 2), Some(17));
        assert_eq!(ticks("minecraft:stone", "minecraft:wooden_pickaxe", |c| c.mining_fatigue = 1), Some(75));
        // Efficiency does nothing for a tool that is not effective on the block.
        assert_eq!(ticks("minecraft:stone", "minecraft:diamond_axe", |c| c.efficiency = 5), Some(150));
    }

    #[test]
    fn special_tools_and_blocks() {
        assert_eq!(ticks("minecraft:web", "minecraft:iron_sword", |_| {}), Some(8));
        assert_eq!(ticks("minecraft:web", "", |_| {}), Some(400));
        assert_eq!(ticks("minecraft:white_wool", "minecraft:shears", |_| {}), Some(5));
        assert_eq!(ticks("minecraft:oak_leaves", "minecraft:shears", |_| {}), Some(1));
        assert_eq!(ticks("minecraft:bamboo", "minecraft:wooden_sword", |_| {}), Some(1));
        assert_eq!(ticks("minecraft:short_grass", "", |_| {}), Some(1));
        assert_eq!(ticks("minecraft:bedrock", "minecraft:netherite_pickaxe", |_| {}), None);
    }
}
