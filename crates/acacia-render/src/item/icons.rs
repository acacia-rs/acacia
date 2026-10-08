//! Which `textures/items` image an item draws with. Vanilla's item-to-icon map is built into the
//! client (BDS doesn't know it), so it is rebuilt here: rename rules, then a table for the rest.
//! Block items draw their block instead; see README "Items".

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

use crate::assets::json;

/// Items no rule names, to an icon key or texture file stem.
const EXPLICIT: &[(&str, &str)] = &[
    ("golden_apple", "apple_golden"),
    ("enchanted_golden_apple", "apple_golden"),
    ("golden_carrot", "carrot_golden"),
    ("cod", "fish"),
    ("cooked_cod", "cooked_fish"),
    ("tropical_fish", "clownfish"),
    ("tropical_fish_bucket", "bucket_tropical"),
    ("melon_slice", "melon"),
    ("glistering_melon_slice", "melon_speckled"),
    ("baked_potato", "potato_baked"),
    ("poisonous_potato", "potato_poisonous"),
    ("popped_chorus_fruit", "chorus_fruit_popped"),
    ("fermented_spider_eye", "spider_eye_fermented"),
    ("bow", "bow_standby"),
    ("crossbow", "crossbow_standby"),
    ("oak_sign", "sign"),
    ("minecart", "minecart_normal"),
    ("redstone", "redstone_dust"),
    ("sugar_cane", "reeds"),
    ("slime_ball", "slimeball"),
    ("compass", "compass_item"),
    ("lodestone_compass", "lodestonecompass_item"),
    ("clock", "clock_item"),
    ("book", "book_normal"),
    ("writable_book", "book_writable"),
    ("written_book", "book_written"),
    ("enchanted_book", "book_enchanted"),
    ("empty_map", "map_empty"),
    ("filled_map", "map_filled"),
    ("potion", "potion_bottle_drinkable"),
    ("splash_potion", "potion_bottle_splash"),
    ("lingering_potion", "potion_bottle_lingering"),
    ("glass_bottle", "potion_bottle_empty"),
    ("bone_meal", "dye_powder_white"),
    ("cocoa_beans", "dye_powder_brown"),
    ("ink_sac", "dye_powder_black"),
    ("lapis_lazuli", "dye_powder_blue"),
    ("glow_ink_sac", "dye_powder_glow"),
    ("fire_charge", "fireball"),
    ("firework_rocket", "fireworks"),
    ("firework_star", "fireworks_charge"),
    ("totem_of_undying", "totem"),
    ("turtle_scute", "turtle_shell_piece"),
    ("zombie_pigman_spawn_egg", "spawn_egg_zombified_piglin"),
    ("tropical_fish_spawn_egg", "spawn_egg_tropicalfish"),
];

/// Wood names whose icons spell them differently.
fn wood(name: &str) -> &str {
    if name == "dark_oak" { "darkoak" } else { name }
}

fn tool_material(name: &str) -> &str {
    match name {
        "golden" => "gold",
        "wooden" => "wood",
        other => other,
    }
}

/// Icon keys or file stems to try for an item name without its namespace, in order.
fn candidates(name: &str) -> Vec<String> {
    let mut out = vec![name.to_owned()];
    let mut add = |c: String| out.push(c);
    if let Some(mob) = name.strip_suffix("_spawn_egg") {
        add(format!("spawn_egg_{mob}"));
        add(format!("spawn_egg_{}", mob.replace('_', "")));
    }
    if let Some(disc) = name.strip_prefix("music_disc_") {
        add(format!("record_{disc}"));
    }
    if let Some(food) = name.strip_prefix("cooked_") {
        add(format!("{food}_cooked"));
    }
    if matches!(name, "beef" | "chicken" | "mutton" | "porkchop" | "rabbit") {
        add(format!("{name}_raw"));
    }
    if let Some(crop) = name.strip_suffix("_seeds") {
        add(format!("seeds_{crop}"));
    }
    if let Some((colour, kind)) = name.rsplit_once('_').filter(|(_, k)| matches!(*k, "harness" | "bundle")) {
        add(format!("{kind}_{colour}"));
    }
    if let Some(w) = name.strip_suffix("_hanging_sign") {
        add(format!("sign_{}_hanging", wood(w)));
    } else if let Some(w) = name.strip_suffix("_sign") {
        add(format!("sign_{}", wood(w)));
        add(format!("{w}_sign_item"));
    }
    if let Some(kind) = name.strip_suffix("_minecart") {
        add(format!("minecart_{kind}"));
    }
    if let Some(colour) = name.strip_suffix("_dye") {
        let colour = if colour == "light_gray" { "silver" } else { colour };
        add(format!("dye_powder_{colour}_new"));
        add(format!("dye_powder_{colour}"));
    }
    if let Some(kind) = name.strip_suffix("_bucket") {
        add(format!("bucket_{kind}"));
    }
    if let Some(w) = name.strip_suffix("_chest_boat") {
        add(format!("chest_boat_{}", wood(w)));
    } else if let Some(w) = name.strip_suffix("_boat") {
        add(format!("boat_{}", wood(w)));
    }
    if let Some((material, kind)) = name.split_once('_') {
        add(format!("{}_{kind}", tool_material(material)));
    }
    out
}

pub struct ItemIcons {
    /// `item_texture.json`: icon key to its image paths; arrays are picked by the item's aux value.
    keys: HashMap<String, Vec<String>>,
    /// Every image path by file stem, from the keys and the `textures/items` folder.
    stems: HashMap<String, String>,
}

impl ItemIcons {
    pub fn load(root: &Path) -> ItemIcons {
        let mut keys: HashMap<String, Vec<String>> = HashMap::new();
        let data = json::read(&root.join("textures/item_texture.json")).ok();
        for (key, entry) in data.as_ref().and_then(|d| d["texture_data"].as_object()).into_iter().flatten() {
            let paths = match &entry["textures"] {
                Value::Array(list) => list.iter().filter_map(path).collect(),
                single => path(single).into_iter().collect(),
            };
            keys.insert(key.clone(), paths);
        }
        let mut stems: HashMap<String, String> = keys.values().flatten().map(|p: &String| (stem(p).to_owned(), p.clone())).collect();
        for entry in std::fs::read_dir(root.join("textures/items")).into_iter().flatten().flatten() {
            let file = entry.file_name().to_string_lossy().into_owned();
            if let Some(s) = file.strip_suffix(".png").or_else(|| file.strip_suffix(".tga")) {
                stems.entry(s.to_owned()).or_insert_with(|| format!("textures/items/{s}"));
            }
        }
        ItemIcons { keys, stems }
    }

    /// The image path (pack-relative, no extension) for `name` (`minecraft:apple`) with `aux`.
    pub fn path(&self, name: &str, aux: u32) -> Option<&str> {
        let bare = name.strip_prefix("minecraft:").unwrap_or(name);
        let explicit = EXPLICIT.iter().find(|(n, _)| *n == bare).map(|(_, icon)| icon.to_string());
        explicit.into_iter().chain(candidates(bare)).find_map(|c| self.lookup(&c, aux))
    }

    fn lookup(&self, candidate: &str, aux: u32) -> Option<&str> {
        if let Some(paths) = self.keys.get(candidate).filter(|p| !p.is_empty()) {
            return Some(&paths[(aux as usize).min(paths.len() - 1)]);
        }
        self.stems.get(candidate).map(String::as_str)
    }
}

fn path(texture: &Value) -> Option<String> {
    texture.as_str().or_else(|| texture["path"].as_str()).map(str::to_owned)
}

fn stem(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_cover_renamed_families() {
        let has = |name: &str, want: &str| assert!(candidates(name).iter().any(|c| c == want), "{name} -> {want}");
        has("cooked_beef", "beef_cooked");
        has("beef", "beef_raw");
        has("golden_sword", "gold_sword");
        has("wooden_pickaxe", "wood_pickaxe");
        has("cave_spider_spawn_egg", "spawn_egg_cave_spider");
        has("music_disc_cat", "record_cat");
        has("dark_oak_hanging_sign", "sign_darkoak_hanging");
        has("light_gray_dye", "dye_powder_silver");
        has("dark_oak_chest_boat", "chest_boat_darkoak");
        has("lime_harness", "harness_lime");
    }

    #[test]
    fn arrays_pick_by_aux_and_clamp() {
        let icons = ItemIcons {
            keys: HashMap::from([("bucket".into(), vec!["textures/items/bucket_empty".into(), "textures/items/bucket_milk".into()])]),
            stems: HashMap::new(),
        };
        assert_eq!(icons.path("minecraft:bucket", 1), Some("textures/items/bucket_milk"));
        assert_eq!(icons.path("minecraft:bucket", 9), Some("textures/items/bucket_milk"));
        assert_eq!(icons.path("minecraft:nothing", 0), None);
    }
}
