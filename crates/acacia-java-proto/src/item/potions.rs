//! The 26.3 `potion` registry, a built-in one: ids are `Potions`' registration order.

use crate::wire::read_len;
use crate::{Error, Wire, read_varint, write_varint};

use super::codecs::{mob_effect, optional, string};

pub const POTIONS: [&str; 46] = [
    "water", "mundane", "thick", "awkward", "night_vision", "long_night_vision", "invisibility", "long_invisibility",
    "leaping", "long_leaping", "strong_leaping", "fire_resistance", "long_fire_resistance", "swiftness",
    "long_swiftness", "strong_swiftness", "slowness", "long_slowness", "strong_slowness", "turtle_master",
    "long_turtle_master", "strong_turtle_master", "water_breathing", "long_water_breathing", "healing",
    "strong_healing", "harming", "strong_harming", "poison", "long_poison", "strong_poison", "regeneration",
    "long_regeneration", "strong_regeneration", "strength", "long_strength", "strong_strength", "weakness",
    "long_weakness", "luck", "slow_falling", "long_slow_falling", "wind_charged", "weaving", "oozing", "infested",
];

/// The namespaced name of potion `id`.
pub fn potion_name(id: i32) -> Option<String> {
    POTIONS.get(usize::try_from(id).ok()?).map(|name| format!("minecraft:{name}"))
}

/// `PotionContents.STREAM_CODEC`: the potion id of a plain one (no custom colour, effects or name), else `None`.
pub(super) fn read_plain(input: &mut &[u8]) -> Result<Option<i32>, Error> {
    let potion = if bool::read(input)? { Some(read_varint(input)?) } else { None };
    let coloured = bool::read(input)?;
    if coloured {
        i32::read(input)?;
    }
    let effects = read_len(input)?;
    for _ in 0..effects {
        mob_effect(input)?;
    }
    let named = matches!(input.first(), Some(1));
    optional(input, string)?;
    Ok(potion.filter(|_| !coloured && effects == 0 && !named))
}

pub(super) fn write_plain(id: i32, out: &mut Vec<u8>) {
    true.write(out);
    write_varint(id, out);
    false.write(out);
    write_varint(0, out);
    false.write(out);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_potions_round_trip_and_custom_ones_are_dropped() {
        let mut out = Vec::new();
        write_plain(3, &mut out);
        assert_eq!(read_plain(&mut &out[..]).unwrap(), Some(3));
        assert_eq!(potion_name(3).as_deref(), Some("minecraft:awkward"));
        // Water with a custom colour: kept out, so its hash misses as before.
        let coloured = [1, 0, 1, 0, 0, 0, 7, 0, 0];
        let mut input = &coloured[..];
        assert_eq!(read_plain(&mut input).unwrap(), None);
        assert!(input.is_empty());
    }
}
