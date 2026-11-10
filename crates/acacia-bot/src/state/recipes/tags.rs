//! Vanilla item tags used by recipe ingredients. The client has them built in and no packet
//! carries them, so this covers the tags common crafting recipes use; an unknown tag matches
//! nothing.

const WOODS: &[&str] =
    &["oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "pale_oak", "bamboo", "crimson", "warped"];

/// Whether item `name` (`minecraft:oak_planks`) carries `tag` (`minecraft:planks`).
pub(super) fn has_tag(name: &str, tag: &str) -> bool {
    let Some(item) = name.strip_prefix("minecraft:") else { return false };
    let wooden = |suffix: &str| item.strip_suffix(suffix).is_some_and(|wood| WOODS.contains(&wood));
    match tag {
        "minecraft:planks" => item.ends_with("_planks"),
        "minecraft:wooden_slabs" => wooden("_slab"),
        "minecraft:logs" => is_log(item),
        "minecraft:logs_that_burn" => is_log(item) && !item.contains("crimson") && !item.contains("warped"),
        "minecraft:coals" => matches!(item, "coal" | "charcoal"),
        "minecraft:stone_tool_materials" | "minecraft:stone_crafting_materials" => {
            matches!(item, "cobblestone" | "blackstone" | "cobbled_deepslate")
        }
        "minecraft:wool" => item.ends_with("_wool"),
        "minecraft:soul_fire_base_blocks" => matches!(item, "soul_sand" | "soul_soil"),
        "minecraft:sand" => matches!(item, "sand" | "red_sand"),
        "minecraft:trim_templates" => item.ends_with("_armor_trim_smithing_template"),
        "minecraft:trim_materials" => TRIM_MATERIALS.contains(&item),
        "minecraft:trimmable_armors" => item == "turtle_helmet" || ARMOR_METALS.iter().any(|m| ARMOR_PIECES.iter().any(|p| item.strip_prefix(m).and_then(|r| r.strip_prefix('_')) == Some(p))),
        _ => false,
    }
}

const TRIM_MATERIALS: &[&str] =
    &["quartz", "iron_ingot", "netherite_ingot", "redstone", "copper_ingot", "gold_ingot", "emerald", "diamond", "lapis_lazuli", "amethyst_shard", "resin_brick"];
const ARMOR_METALS: &[&str] = &["leather", "chainmail", "iron", "golden", "diamond", "netherite", "copper"];
const ARMOR_PIECES: &[&str] = &["helmet", "chestplate", "leggings", "boots"];

fn is_log(item: &str) -> bool {
    let item = item.strip_prefix("stripped_").unwrap_or(item);
    ["_log", "_wood", "_stem", "_hyphae"].iter().any(|s| item.ends_with(s)) || item == "bamboo_block"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_tags() {
        assert!(has_tag("minecraft:birch_planks", "minecraft:planks"));
        assert!(has_tag("minecraft:dark_oak_slab", "minecraft:wooden_slabs"));
        assert!(!has_tag("minecraft:stone_slab", "minecraft:wooden_slabs"));
        assert!(has_tag("minecraft:stripped_cherry_log", "minecraft:logs"));
        assert!(!has_tag("minecraft:crimson_stem", "minecraft:logs_that_burn"));
        assert!(has_tag("minecraft:charcoal", "minecraft:coals"));
        assert!(has_tag("minecraft:cobbled_deepslate", "minecraft:stone_tool_materials"));
        assert!(!has_tag("minecraft:stone", "minecraft:unknown_tag"));
    }

    #[test]
    fn trim_tags() {
        assert!(has_tag("minecraft:sentry_armor_trim_smithing_template", "minecraft:trim_templates"));
        assert!(!has_tag("minecraft:netherite_upgrade_smithing_template", "minecraft:trim_templates"));
        assert!(has_tag("minecraft:copper_ingot", "minecraft:trim_materials"));
        assert!(!has_tag("minecraft:coal", "minecraft:trim_materials"));
        for armor in ["iron_chestplate", "golden_boots", "leather_helmet", "chainmail_leggings", "turtle_helmet", "copper_helmet"] {
            assert!(has_tag(&format!("minecraft:{armor}"), "minecraft:trimmable_armors"), "{armor}");
        }
        assert!(!has_tag("minecraft:elytra", "minecraft:trimmable_armors"));
        assert!(!has_tag("minecraft:iron_horse_armor", "minecraft:trimmable_armors"));
    }
}
