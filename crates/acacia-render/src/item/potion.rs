//! What a potion's contents do to its item: the tint Java 26.3's `potion` tint source gives
//! (`PotionContents.getColor`: the effects' colours averaged by amplifier, or the base colour),
//! and which Java potion a Bedrock potion's aux is. Checked by the parity harness `item_models`.

/// Items whose model tints by their potion contents.
pub fn holds_potion(item: &str) -> bool {
    matches!(item.trim_start_matches("minecraft:"), "potion" | "splash_potion" | "lingering_potion" | "tipped_arrow" | "arrow")
}

/// Bedrock potion ids by aux, as Java potions; `None` where Java has no such potion (decay).
const BEDROCK: [Option<&str>; 47] = [
    Some("water"),
    Some("mundane"),
    Some("mundane"),
    Some("thick"),
    Some("awkward"),
    Some("night_vision"),
    Some("long_night_vision"),
    Some("invisibility"),
    Some("long_invisibility"),
    Some("leaping"),
    Some("long_leaping"),
    Some("strong_leaping"),
    Some("fire_resistance"),
    Some("long_fire_resistance"),
    Some("swiftness"),
    Some("long_swiftness"),
    Some("strong_swiftness"),
    Some("slowness"),
    Some("long_slowness"),
    Some("water_breathing"),
    Some("long_water_breathing"),
    Some("healing"),
    Some("strong_healing"),
    Some("harming"),
    Some("strong_harming"),
    Some("poison"),
    Some("long_poison"),
    Some("strong_poison"),
    Some("regeneration"),
    Some("long_regeneration"),
    Some("strong_regeneration"),
    Some("strength"),
    Some("long_strength"),
    Some("strong_strength"),
    Some("weakness"),
    Some("long_weakness"),
    None,
    Some("turtle_master"),
    Some("long_turtle_master"),
    Some("strong_turtle_master"),
    Some("slow_falling"),
    Some("long_slow_falling"),
    Some("strong_slowness"),
    Some("wind_charged"),
    Some("weaving"),
    Some("oozing"),
    Some("infested"),
];

/// The Java potion (`minecraft:swiftness`) a Bedrock potion item's `aux` holds. A tipped arrow's
/// aux is the potion's plus one (0 is a plain arrow).
pub fn java_potion(item: &str, aux: u32) -> Option<String> {
    let index = if item.trim_start_matches("minecraft:") == "arrow" { aux.checked_sub(1)? } else { aux };
    Some(format!("minecraft:{}", BEDROCK.get(index as usize).copied().flatten()?))
}

/// `PotionContents.BASE_POTION_COLOR`: no potion, or one without effects.
pub const BASE: u32 = 0xFF38_5DC6;

/// The ARGB a potion tints with, by Java potion; [`BASE`] for any other.
pub fn color(potion: &str) -> u32 {
    COLORS.iter().find(|(id, _)| *id == potion.trim_start_matches("minecraft:")).map_or(BASE, |&(_, argb)| argb)
}

/// `getColorOptional` of each potion's effects (`MobEffects`' colours, weighted by amplifier).
const COLORS: &[(&str, u32)] = &[
    ("night_vision", 0xFFC2FF66),
    ("long_night_vision", 0xFFC2FF66),
    ("invisibility", 0xFFF6F6F6),
    ("long_invisibility", 0xFFF6F6F6),
    ("leaping", 0xFFFDFF84),
    ("long_leaping", 0xFFFDFF84),
    ("strong_leaping", 0xFFFDFF84),
    ("fire_resistance", 0xFFFF9900),
    ("long_fire_resistance", 0xFFFF9900),
    ("swiftness", 0xFF33EBFF),
    ("long_swiftness", 0xFF33EBFF),
    ("strong_swiftness", 0xFF33EBFF),
    ("slowness", 0xFF8BAFE0),
    ("long_slowness", 0xFF8BAFE0),
    ("strong_slowness", 0xFF8BAFE0),
    ("turtle_master", 0xFF8D82E6),
    ("long_turtle_master", 0xFF8D82E6),
    ("strong_turtle_master", 0xFF8D85E6),
    ("water_breathing", 0xFF98DAC0),
    ("long_water_breathing", 0xFF98DAC0),
    ("healing", 0xFFF82423),
    ("strong_healing", 0xFFF82423),
    ("harming", 0xFFA9656A),
    ("strong_harming", 0xFFA9656A),
    ("poison", 0xFF87A363),
    ("long_poison", 0xFF87A363),
    ("strong_poison", 0xFF87A363),
    ("regeneration", 0xFFCD5CAB),
    ("long_regeneration", 0xFFCD5CAB),
    ("strong_regeneration", 0xFFCD5CAB),
    ("strength", 0xFFFFC700),
    ("long_strength", 0xFFFFC700),
    ("strong_strength", 0xFFFFC700),
    ("weakness", 0xFF484D48),
    ("long_weakness", 0xFF484D48),
    ("luck", 0xFF59C106),
    ("slow_falling", 0xFFF3CFB9),
    ("long_slow_falling", 0xFFF3CFB9),
    ("wind_charged", 0xFFBDC9FF),
    ("weaving", 0xFF78695A),
    ("oozing", 0xFF99FFA3),
    ("infested", 0xFF8C9B8C),
];
