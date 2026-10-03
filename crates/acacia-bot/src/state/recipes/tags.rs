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
        _ => false,
    }
}

fn is_log(item: &str) -> bool {
    let item = item.strip_prefix("stripped_").unwrap_or(item);
    ["_log", "_wood", "_stem", "_hyphae"].iter().any(|s| item.ends_with(s)) || item == "bamboo_block"
}

/// Tags named by a MoLang ingredient such as `query.any_tag('minecraft:planks', 'minecraft:logs')`.
pub(super) fn molang_tags(expression: &str) -> impl Iterator<Item = &str> {
    let args = expression.split_once("any_tag(").map_or("", |(_, rest)| rest.split(')').next().unwrap_or(""));
    args.split(',').map(|a| a.trim().trim_matches(|c| c == '\'' || c == '"')).filter(|a| !a.is_empty())
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
    fn molang_any_tag() {
        let tags: Vec<_> = molang_tags("query.any_tag('minecraft:planks', \"minecraft:logs\")").collect();
        assert_eq!(tags, ["minecraft:planks", "minecraft:logs"]);
        assert_eq!(molang_tags("query.is_item_name_any('x')").count(), 0);
    }
}
