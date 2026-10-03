//! What the bot carries that the search may use: tools to dig with, blocks to build with.

use acacia_world::{Mining, Tool};

use crate::interact::breaking::{enchantment_level, EFFICIENCY};
use crate::interact::{break_ticks, BreakConditions};
use crate::items::SlotRef;
use crate::Bot;

/// Throwaway blocks [`GotoOpts::scaffold`](super::GotoOpts::scaffold) starts with.
pub const DEFAULT_SCAFFOLD: &[&str] = &[
    "minecraft:dirt",
    "minecraft:cobblestone",
    "minecraft:cobbled_deepslate",
    "minecraft:netherrack",
    "minecraft:stone",
    "minecraft:andesite",
    "minecraft:diorite",
    "minecraft:granite",
    "minecraft:tuff",
    "minecraft:blackstone",
    "minecraft:end_stone",
];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Kit {
    /// Tools and their Efficiency level.
    pub tools: Vec<(Tool, u8)>,
    /// Scaffold blocks available for bridging and pillaring.
    pub blocks: u32,
}

impl Kit {
    /// Fewest ticks to break a block with the hand or any tool, standing on the ground.
    pub fn break_ticks(&self, mining: &Mining) -> Option<u32> {
        let hand = BreakConditions { on_ground: true, ..BreakConditions::default() };
        let tools = self.tools.iter().map(|&(tool, efficiency)| BreakConditions { tool: Some(tool), efficiency, ..hand });
        std::iter::once(hand).chain(tools).filter_map(|c| break_ticks(mining, &c)).min()
    }
}

impl Bot {
    /// The tools and the `scaffold` blocks (item identifiers) in the main inventory.
    pub fn path_kit(&self, scaffold: &[String]) -> Kit {
        let mut kit = Kit::default();
        for stack in self.state.inventory.main.iter().filter(|s| !s.is_empty()) {
            let Some(name) = self.state.items.name(stack.network_id) else { continue };
            if let Some(tool) = Tool::from_identifier(name) {
                let entry = (tool, enchantment_level(stack, EFFICIENCY));
                if !kit.tools.contains(&entry) {
                    kit.tools.push(entry);
                }
            } else if scaffold.iter().any(|s| s == name) {
                kit.blocks += u32::from(stack.count);
            }
        }
        kit
    }

    /// The first slot holding any of the `scaffold` blocks.
    pub(crate) fn scaffold_slot(&self, scaffold: &[String]) -> Option<SlotRef> {
        scaffold.iter().find_map(|name| self.find_item(name))
    }
}
