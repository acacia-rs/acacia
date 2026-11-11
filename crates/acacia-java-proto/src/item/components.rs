//! Every `DataComponents` type's network codec, skipped. Ids are `data_component_type` registry order.

use super::codecs::{Skipped, boolean, bytes, consume_effect, filterable, float, holder, holder_set, list, mob_effect, optional, resolvable, sound, string, text, varint};
use super::composites::{
    attribute_modifier, block_predicate, blocks_attacks, custom_model_data, equippable, firework_explosion, item_template, kinetic_weapon, profile, tool,
    two_varints,
};
use crate::{Error, nbt};

/// The 26.3 `data_component_type` registry size.
pub const COUNT: i32 = 122;

/// Skips the value of component `id`; values the client never receives are an error, as in vanilla.
pub(super) fn skip(id: i32, input: &mut &[u8], depth: u32) -> Skipped {
    let template = |input: &mut &[u8]| item_template(input, depth);
    match id {
        0 => Err(Error::Invalid("synced component custom_data")),
        // A varint value, enum id or `holderRegistry` id: counts, rarity, dyes, map ids, and the mob variants.
        1..=3 | 8 | 12 | 19 | 31 | 42..=44 | 46 | 48 | 50 | 65 | 75 | 88..=108 | 110..=117 | 121 => varint(input),
        4 | 20 | 34 | 120 => Ok(()),
        5 => bytes::<6>(input),
        6 | 9 => text(input),
        7 | 54 => float(input),
        10 | 35 | 73 => string(input),
        11 => list(input, text),
        13 | 45 | 55 => list(input, two_varints),
        14 | 15 => list(input, |input| block_predicate(input, depth)),
        16 => list(input, attribute_modifier),
        17 => custom_model_data(input),
        18 => {
            boolean(input)?;
            list(input, varint)
        }
        21 => bytes::<1>(input),
        22 => Err(Error::Invalid("synced component intangible_projectile")),
        23 => {
            varint(input)?;
            bytes::<5>(input)
        }
        24 => {
            float(input)?;
            varint(input)?;
            sound(input)?;
            boolean(input)?;
            list(input, consume_effect)
        }
        25 | 80 => template(input),
        26 => {
            float(input)?;
            optional(input, string)
        }
        27 | 33 | 67 => holder_set(input),
        28 => tool(input),
        29 => {
            varint(input)?;
            float(input)
        }
        30 => bytes::<24>(input),
        32 => equippable(input),
        36 => list(input, consume_effect),
        37 => blocks_attacks(input),
        38 => {
            bytes::<2>(input)?;
            optional(input, sound)?;
            optional(input, sound)
        }
        39 => kinetic_weapon(input),
        40 | 41 => two_varints(input),
        47 => bytes::<4>(input),
        49 => Err(Error::Invalid("synced component map_decorations")),
        51 | 52 => list(input, template),
        53 => {
            optional(input, varint)?;
            optional(input, bytes::<4>)?;
            list(input, mob_effect)?;
            optional(input, string)
        }
        56 => list(input, |input| filterable(input, string)),
        57 => {
            filterable(input, string)?;
            string(input)?;
            varint(input)?;
            list(input, |input| filterable(input, text))?;
            bytes::<1>(input)
        }
        58 => {
            holder(input, |input| {
                string(input)?;
                text(input)
            })?;
            holder(input, |input| {
                string(input)?;
                text(input)?;
                bytes::<1>(input)
            })
        }
        59 => Err(Error::Invalid("synced component debug_stick_state")),
        60 | 62 => {
            varint(input)?;
            nbt::skip_compound(input)
        }
        61 => nbt::skip_compound(input),
        63 => holder(input, |input| {
            sound(input)?;
            bytes::<8>(input)?;
            varint(input)?;
            text(input)
        }),
        64 => holder(input, |input| {
            string(input)?;
            text(input)
        }),
        66 => holder(input, |input| {
            sound(input)?;
            text(input)?;
            float(input)?;
            varint(input)
        }),
        68 => Err(Error::Invalid("synced component recipes")),
        69 => {
            optional(input, |input| {
                string(input)?;
                bytes::<8>(input)
            })?;
            bytes::<1>(input)
        }
        70 => firework_explosion(input),
        71 => {
            varint(input)?;
            list(input, firework_explosion)
        }
        72 => profile(input),
        74 => list(input, |input| {
            holder(input, |input| {
                string(input)?;
                string(input)
            })?;
            varint(input)
        }),
        76 => (0..4).try_for_each(|_| optional(input, template)),
        77 => list(input, |input| optional(input, template)),
        78 => list(input, |input| {
            string(input)?;
            string(input)
        }),
        79 => list(input, |input| {
            varint(input)?;
            nbt::skip_compound(input)?;
            two_varints(input)
        }),
        81 => Err(Error::Invalid("synced component lock")),
        82 => Err(Error::Invalid("synced component container_loot")),
        83 => sound(input),
        84 => resolvable(input),
        85 | 86 => {
            resolvable(input)?;
            resolvable(input)
        }
        87 => {
            holder_set(input)?;
            float(input)
        }
        109 => holder(input, |input| {
            two_varints(input)?;
            string(input)?;
            optional(input, text)?;
            optional(input, text)
        }),
        118 | 119 => {
            (0..4).try_for_each(|_| text(input))?;
            optional(input, |input| (0..4).try_for_each(|_| text(input)))?;
            varint(input)?;
            bytes::<1>(input)
        }
        _ => Err(Error::Invalid("data component type")),
    }
}
