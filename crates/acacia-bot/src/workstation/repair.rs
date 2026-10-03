//! Anvil repair arithmetic: how much material a repair uses up and the damage left. The server
//! rejects a request whose material `Consume` differs from what it uses (status 5), so the bot
//! has to predict it. Values are vanilla's (Java and Bedrock agree).

/// Maximum durability of a repairable item, `None` for items without durability.
pub(crate) fn max_durability(name: &str) -> Option<u16> {
    let item = name.strip_prefix("minecraft:").unwrap_or(name);
    let fixed = match item {
        "elytra" => Some(432),
        "shield" => Some(336),
        "turtle_helmet" => Some(275),
        "mace" => Some(500),
        "trident" => Some(250),
        "crossbow" => Some(465),
        "bow" => Some(384),
        "fishing_rod" => Some(384),
        "shears" => Some(238),
        "flint_and_steel" => Some(64),
        _ => None,
    };
    if fixed.is_some() {
        return fixed;
    }
    let (material, piece) = item.rsplit_once('_')?;
    let tool = match material {
        "wooden" => Some(59),
        "stone" => Some(131),
        "iron" => Some(250),
        "golden" => Some(32),
        "diamond" => Some(1561),
        "netherite" => Some(2031),
        _ => None,
    };
    if matches!(piece, "sword" | "pickaxe" | "axe" | "shovel" | "hoe") {
        return tool;
    }
    let base = match piece {
        "helmet" => 11,
        "chestplate" => 16,
        "leggings" => 15,
        "boots" => 13,
        _ => return None,
    };
    let multiplier = match material {
        "leather" => 5,
        "chainmail" | "iron" => 15,
        "golden" => 7,
        "diamond" => 33,
        "netherite" => 37,
        _ => return None,
    };
    Some(base * multiplier)
}

/// Whether `material` is the unit material that repairs `item` on an anvil.
pub(crate) fn repairs(item: &str, material: &str) -> bool {
    match repair_material(item) {
        Some("minecraft:planks") => material.ends_with("_planks"),
        Some(m) => m == material,
        None => false,
    }
}

fn repair_material(name: &str) -> Option<&'static str> {
    let item = name.strip_prefix("minecraft:").unwrap_or(name);
    Some(match item.split('_').next()? {
        "wooden" => "minecraft:planks",
        "stone" => "minecraft:cobblestone",
        "iron" | "chainmail" => "minecraft:iron_ingot",
        "golden" => "minecraft:gold_ingot",
        "diamond" => "minecraft:diamond",
        "netherite" => "minecraft:netherite_ingot",
        "leather" => "minecraft:leather",
        "turtle" => "minecraft:turtle_scute",
        "elytra" => "minecraft:phantom_membrane",
        _ => return None,
    })
}

/// Material units a repair of `damage` uses (each restores a quarter of `max`), at most `available`.
/// 0 when a quarter rounds to 0: BDS then makes no result.
pub(crate) fn repair_units(max: u16, damage: u16, available: u16) -> u16 {
    match max / 4 {
        0 => 0,
        per_unit => damage.div_ceil(per_unit).min(available),
    }
}

/// Damage left after combining two damaged items of the same kind at the anvil (the sum of what
/// is left of both plus a 12 % bonus).
pub(crate) fn combined_damage(max: u16, a: u16, b: u16) -> u16 {
    merged_damage(max, a, b, u32::from(max) * 12 / 100)
}

/// [`combined_damage`] at the grindstone, whose bonus is 5 % (`max / 20`).
pub(crate) fn grindstone_damage(max: u16, a: u16, b: u16) -> u16 {
    merged_damage(max, a, b, u32::from(max) / 20)
}

fn merged_damage(max: u16, a: u16, b: u16, bonus: u32) -> u16 {
    let left = u32::from(max - a.min(max)) + u32::from(max - b.min(max)) + bonus;
    max.saturating_sub(left.min(u32::from(max)) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durability_and_material() {
        assert_eq!(max_durability("minecraft:iron_pickaxe"), Some(250));
        assert_eq!(max_durability("minecraft:diamond_chestplate"), Some(528));
        assert_eq!(max_durability("minecraft:elytra"), Some(432));
        assert_eq!(max_durability("minecraft:stone"), None);
        assert!(repairs("minecraft:iron_pickaxe", "minecraft:iron_ingot"));
        assert!(repairs("minecraft:wooden_axe", "minecraft:birch_planks"));
        assert!(!repairs("minecraft:iron_pickaxe", "minecraft:gold_ingot"));
        assert!(repairs("minecraft:elytra", "minecraft:phantom_membrane"));
    }

    #[test]
    fn repair_uses_a_unit_per_quarter() {
        // Verified on BDS 1.26.52: an iron pickaxe with damage 200 took 4 ingots and came out at 0.
        assert_eq!(repair_units(250, 200, 4), 4);
        assert_eq!(repair_units(250, 200, 2), 2);
        assert_eq!(repair_units(250, 62, 8), 1);
        assert_eq!(repair_units(250, 0, 8), 0);
        assert_eq!(repair_units(3, 2, 8), 0, "a quarter of 3 is 0: no result on BDS");
        assert_eq!(combined_damage(250, 200, 200), 120);
        assert_eq!(combined_damage(250, 10, 10), 0);
        assert_eq!(grindstone_damage(250, 200, 200), 138, "200 + 200 - 250 - 12");
        assert_eq!(grindstone_damage(250, 100, 10), 0);
    }
}
