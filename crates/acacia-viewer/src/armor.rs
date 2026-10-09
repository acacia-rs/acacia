//! Armour points for the HUD's armour bar: the client adds them up from what is worn (the server
//! sends no armour attribute). Java's values per piece.

/// Points `item` (`minecraft:iron_chestplate`) gives; 0 for anything that is not armour.
pub fn points(item: &str) -> u32 {
    let name = item.strip_prefix("minecraft:").unwrap_or(item);
    if name == "turtle_helmet" {
        return 2;
    }
    let Some((material, piece)) = name.rsplit_once('_') else { return 0 };
    // Helmet, chestplate, leggings, boots.
    let by_piece: [u32; 4] = match material {
        "leather" => [1, 3, 2, 1],
        "copper" => [2, 4, 3, 1],
        "golden" => [2, 5, 3, 1],
        "chainmail" => [2, 5, 4, 1],
        "iron" => [2, 6, 5, 2],
        "diamond" | "netherite" => [3, 8, 6, 3],
        _ => return 0,
    };
    match piece {
        "helmet" => by_piece[0],
        "chestplate" => by_piece[1],
        "leggings" => by_piece[2],
        "boots" => by_piece[3],
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_sets_add_up_as_in_java() {
        let set = |m: &str| ["helmet", "chestplate", "leggings", "boots"].iter().map(|p| points(&format!("minecraft:{m}_{p}"))).sum::<u32>();
        assert_eq!((set("leather"), set("golden"), set("chainmail"), set("iron"), set("diamond"), set("netherite")), (7, 11, 12, 15, 20, 20));
        assert_eq!((points("minecraft:turtle_helmet"), points("minecraft:diamond_sword"), points("minecraft:stick")), (2, 0, 0));
    }
}
