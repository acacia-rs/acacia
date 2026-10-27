//! What an enchanting table's option says: the level it needs and the enchantment it hints at
//! (the game shows the hint in the option's tooltip, under galactic text).

/// Bedrock's enchantment ids, in order.
const NAMES: [&str; 42] = [
    "Protection", "Fire Protection", "Feather Falling", "Blast Protection", "Projectile Protection", "Thorns", "Respiration",
    "Depth Strider", "Aqua Affinity", "Sharpness", "Smite", "Bane of Arthropods", "Knockback", "Fire Aspect", "Looting",
    "Efficiency", "Silk Touch", "Unbreaking", "Fortune", "Power", "Punch", "Flame", "Infinity", "Luck of the Sea", "Lure",
    "Frost Walker", "Mending", "Curse of Binding", "Curse of Vanishing", "Impaling", "Riptide", "Loyalty", "Channeling",
    "Multishot", "Piercing", "Quick Charge", "Soul Speed", "Swift Sneak", "Wind Burst", "Density", "Breach", "Lunge",
];
const ROMAN: [&str; 5] = ["I", "II", "III", "IV", "V"];
/// Letters of a name that fit the option's button beside the level.
const FIT: usize = 13;

/// `level` is what the option needs; `hint` is an enchantment's id and level.
pub fn option(level: u8, hint: Option<(u8, u8)>) -> String {
    let Some((name, rank)) = hint.and_then(|(id, rank)| Some((*NAMES.get(usize::from(id))?, rank))) else { return format!("Level {level}") };
    let short: String = name.chars().take(FIT).collect();
    let cut = if short.len() < name.len() { "." } else { "" };
    format!("{level}: {short}{cut} {}...", ROMAN.get(usize::from(rank).saturating_sub(1)).copied().unwrap_or("?"))
}

/// Java's id and maximum level of each of Bedrock's enchantments, in Bedrock's order.
pub const JAVA: [(&str, i32); 42] = [
    ("protection", 4), ("fire_protection", 4), ("feather_falling", 4), ("blast_protection", 4), ("projectile_protection", 4),
    ("thorns", 3), ("respiration", 3), ("depth_strider", 3), ("aqua_affinity", 1), ("sharpness", 5), ("smite", 5),
    ("bane_of_arthropods", 5), ("knockback", 2), ("fire_aspect", 2), ("looting", 3), ("efficiency", 5), ("silk_touch", 1),
    ("unbreaking", 3), ("fortune", 3), ("power", 5), ("punch", 2), ("flame", 1), ("infinity", 1), ("luck_of_the_sea", 3),
    ("lure", 3), ("frost_walker", 2), ("mending", 1), ("binding_curse", 1), ("vanishing_curse", 1), ("impaling", 5),
    ("riptide", 3), ("loyalty", 3), ("channeling", 1), ("multishot", 1), ("piercing", 4), ("quick_charge", 3),
    ("soul_speed", 3), ("swift_sneak", 3), ("wind_burst", 3), ("density", 5), ("breach", 4), ("lunge", 3),
];

/// A tooltip's lines under a stack's name: each enchantment in grey (curses red), then its lore.
pub fn notes(stack: &crate::control::Stack) -> Vec<String> {
    let enchantment = |&(id, rank): &(i16, i16)| {
        let name = usize::try_from(id).ok().and_then(|id| NAMES.get(id))?;
        let rank = usize::try_from(rank - 1).ok().and_then(|r| ROMAN.get(r)).map_or_else(|| rank.to_string(), |r| (*r).to_owned());
        Some(format!("§{}{name} {rank}", if name.starts_with("Curse") { 'c' } else { '7' }))
    };
    let lore = stack.lore.iter().map(|line| format!("§5§o{line}"));
    stack.enchantments.iter().filter_map(enchantment).chain(lore).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_option_names_its_level_and_its_hint() {
        assert_eq!(option(30, Some((9, 4))), "30: Sharpness IV...");
        assert_eq!(option(8, Some((4, 1))), "8: Projectile Pr. I...");
        assert_eq!((option(3, None), option(3, Some((200, 1)))), ("Level 3".to_owned(), "Level 3".to_owned()));
    }
}
