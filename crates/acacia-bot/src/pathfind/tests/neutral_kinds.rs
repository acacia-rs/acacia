//! The name rules `state_flags` used before acacia-registry, kept to pin what the move changed.
use std::collections::BTreeSet;

use acacia_world::{BlockFlags, BlockRegistry};

use crate::pathfind::cell::{AVOID, DANGER_NEAR, FALLS, HURT_FLOOR, OPENABLE, state_flags};

fn by_name(name: &str) -> u16 {
    let name = name.trim_start_matches("minecraft:");
    match name {
        "fire" | "soul_fire" => AVOID | DANGER_NEAR,
        "campfire" | "soul_campfire" | "wither_rose" => AVOID,
        "cactus" => DANGER_NEAR,
        "magma" => HURT_FLOOR,
        "sand" | "red_sand" | "gravel" | "suspicious_sand" | "suspicious_gravel" => FALLS,
        _ if name.ends_with("concrete_powder") => FALLS,
        _ if (name.ends_with("door") || name.ends_with("fence_gate")) && !name.starts_with("iron_") => OPENABLE,
        _ => 0,
    }
}

#[test]
fn interactive_matches_the_name_fragments_but_for_their_misfires() {
    const FRAGMENTS: &[&str] = &[
        "chest", "barrel", "shulker_box", "door", "gate", "button", "lever", "crafting_table", "furnace", "smoker",
        "anvil", "enchanting_table", "brewing_stand", "beacon", "hopper", "dispenser", "dropper", "loom", "stonecutter",
        "grindstone", "cartography_table", "smithing_table", "bed", "bell", "note_block", "repeater", "comparator",
        "daylight_detector", "cake", "campfire", "lectern", "composter", "jukebox", "respawn_anchor", "crafter",
    ];
    let by_name = |name: &str| {
        let name = name.trim_start_matches("minecraft:");
        FRAGMENTS.iter().any(|part| name.contains(part)) || name.ends_with("_sign")
    };
    let r = BlockRegistry::vanilla();
    let differ: BTreeSet<_> = (0..r.len() as u32)
        .map(|id| r.get(id).unwrap())
        .filter(|s| crate::interact::is_interactive(s) != by_name(s.name))
        .map(|s| s.name)
        .collect();
    // `bed` hit bedrock, `gate` the end gateway, `door` the iron ones; Bedrock spells it `noteblock`.
    let fixed = [
        "minecraft:bedrock", "minecraft:end_gateway", "minecraft:invisible_bedrock", "minecraft:iron_door",
        "minecraft:iron_trapdoor", "minecraft:noteblock",
    ];
    assert_eq!(differ, BTreeSet::from(fixed));
}

#[test]
fn half_slabs_exclude_double_slabs() {
    use acacia_world::neutral::value::SlabType;

    use crate::pathfind::process::BlockView;
    let view = |name, props| {
        let r = BlockRegistry::vanilla();
        BlockView::of(r.get(r.find(name, props).unwrap()).unwrap())
    };
    assert_eq!(view("minecraft:oak_slab", "minecraft:vertical_half=top").half_slab(), Some(SlabType::Top));
    assert_eq!(view("minecraft:oak_slab", "minecraft:vertical_half=bottom").half_slab(), Some(SlabType::Bottom));
    assert_eq!(view("minecraft:oak_double_slab", "minecraft:vertical_half=bottom").half_slab(), None);
    assert_eq!(view("minecraft:stone", "").half_slab(), None);
}

#[test]
fn toggling_open_lands_on_the_bedrock_open_bit_twin() {
    use acacia_world::neutral::prop;
    let r = BlockRegistry::vanilla();
    let mut toggled = 0;
    for id in 0..r.len() as u32 {
        let s = r.get(id).unwrap();
        let Some(open) = s.property("open_bit") else { continue };
        let flipped = s.properties.replace(&format!("open_bit={open}"), if open == "1" { "open_bit=0" } else { "open_bit=1" });
        let by_string = r.find(s.name, &flipped);
        let n = s.neutral.unwrap();
        // A lever's `open_bit` is Java's `powered`; only openables toggle.
        let Some(is_open) = n.get(prop::Open) else { continue };
        let by_neutral = r.runtime_id_of(n.with(prop::Open, !is_open).unwrap());
        assert_eq!(by_neutral, by_string, "{} [{}]", s.name, s.properties);
        toggled += 1;
    }
    assert!(toggled > 500, "{toggled}");
}

#[test]
fn state_flags_match_the_name_rules_but_for_anvils() {
    let r = BlockRegistry::vanilla();
    let named = AVOID | DANGER_NEAR | HURT_FLOOR | FALLS | OPENABLE;
    let physical = |s: &acacia_world::BlockState| {
        let hinders = BlockFlags(BlockFlags::COBWEB.0 | BlockFlags::SWEET_BERRY.0 | BlockFlags::POWDER_SNOW.0);
        (if s.is_lava() { AVOID | DANGER_NEAR } else { 0 }) | (if s.flags.intersects(hinders) { AVOID } else { 0 })
    };
    let differ: BTreeSet<_> = (0..r.len() as u32)
        .map(|id| r.get(id).unwrap())
        .filter(|s| state_flags(s) & named != (physical(s) | by_name(s.name)) & named)
        .map(|s| s.name)
        .collect();
    // Anvils fall like `rules::gravity` says; the name list left them out.
    assert_eq!(differ, BTreeSet::from(["minecraft:anvil", "minecraft:chipped_anvil", "minecraft:damaged_anvil"]));
}

#[test]
fn landing_hazards_differ_from_the_name_rule_only_where_intended() {
    use crate::pathfind::flight::HURTS;
    const FRAGMENTS: [&str; 7] = ["lava", "magma", "fire", "cactus", "sweet_berry", "powder_snow", "pointed_dripstone"];
    let r = BlockRegistry::vanilla();
    let differ: BTreeSet<_> = (0..r.len() as u32)
        .map(|id| r.get(id).unwrap())
        .filter(|s| s.kind().is_some_and(|k| HURTS.contains(&k)) != FRAGMENTS.iter().any(|f| s.name.contains(f)))
        .map(|s| s.name)
        .collect();
    // Substring accidents, plus the lava-filled states of Bedrock's one `cauldron`.
    let intended = |n: &str| n.contains("fire_coral") || matches!(n, "minecraft:firefly_bush" | "minecraft:cactus_flower" | "minecraft:cauldron");
    assert!(differ.iter().all(|n| intended(n)), "{differ:?}");
}

#[test]
fn backfill_substitutes_match_the_name_rule() {
    use acacia_world::neutral::{Block, Item};

    use crate::pathfind::process::substitutes;
    for block in Block::all() {
        let n = block.name();
        let by_name: &[Item] = match n {
            "minecraft:stone" => &[Item::Cobblestone],
            "minecraft:deepslate" => &[Item::CobbledDeepslate],
            "minecraft:grass_block" | "minecraft:podzol" | "minecraft:mycelium" | "minecraft:dirt_path" | "minecraft:farmland" => &[Item::Dirt],
            b if b.starts_with("minecraft:deepslate_") && b.ends_with("_ore") => &[Item::CobbledDeepslate],
            b if b.ends_with("_ore") => &[Item::Cobblestone],
            _ => &[],
        };
        assert_eq!(substitutes(block), by_name, "{n}");
    }
    assert_eq!(Block::from_name("grass_path").map(substitutes), Some(&[Item::Dirt][..]));
    assert_eq!(Block::from_name("lit_deepslate_redstone_ore").map(substitutes), Some(&[Item::CobbledDeepslate][..]));
}
