//! Skippers for the multi-field component records in [`super::components`].

use super::codecs::{Skipped, boolean, bytes, either, float, holder_set, list, optional, profile_properties, sound, string, text, varint};
use super::{Components, MAX_DEPTH};
use crate::{Error, nbt};

pub(super) fn two_varints(input: &mut &[u8]) -> Skipped {
    varint(input)?;
    varint(input)
}

/// `ItemStackTemplate.STREAM_CODEC`: item, count, then a patch; unlike `ItemStack`, the count comes second.
pub(super) fn item_template(input: &mut &[u8], depth: u32) -> Skipped {
    varint(input)?;
    varint(input)?;
    Components::read_nested(input, depth + 1).map(drop)
}

/// `advancements.predicates.BlockPredicate.STREAM_CODEC`.
pub(super) fn block_predicate(input: &mut &[u8], depth: u32) -> Skipped {
    optional(input, holder_set)?;
    optional(input, |input| {
        list(input, |input| {
            string(input)?;
            either(input, string, |input| {
                optional(input, string)?;
                optional(input, string)
            })
        })
    })?;
    optional(input, nbt::skip_compound)?;
    // DataComponentMatchers: exact typed components, then partial predicates as NBT.
    list(input, |input| typed_component(input, depth))?;
    list(input, |input| {
        boolean(input)?;
        varint(input)?;
        text(input)
    })
}

/// `TypedDataComponent.STREAM_CODEC`: a type id and that type's value.
fn typed_component(input: &mut &[u8], depth: u32) -> Skipped {
    if depth >= MAX_DEPTH {
        return Err(Error::TooDeep(MAX_DEPTH));
    }
    let id = crate::read_varint(input)?;
    super::read_component(id, input, depth + 1, &mut Components::default())
}

pub(super) fn attribute_modifier(input: &mut &[u8]) -> Skipped {
    varint(input)?;
    string(input)?;
    bytes::<8>(input)?;
    varint(input)?;
    varint(input)?;
    // Display type: default, hidden, or override text.
    match crate::read_varint(input)? {
        0 | 1 => Ok(()),
        2 => text(input),
        _ => Err(Error::Invalid("attribute modifier display")),
    }
}

pub(super) fn custom_model_data(input: &mut &[u8]) -> Skipped {
    list(input, float)?;
    list(input, bytes::<1>)?;
    list(input, string)?;
    list(input, bytes::<4>)
}

pub(super) fn tool(input: &mut &[u8]) -> Skipped {
    list(input, |input| {
        holder_set(input)?;
        optional(input, float)?;
        optional(input, bytes::<1>)
    })?;
    float(input)?;
    varint(input)?;
    bytes::<1>(input)
}

pub(super) fn equippable(input: &mut &[u8]) -> Skipped {
    varint(input)?;
    sound(input)?;
    optional(input, string)?;
    optional(input, string)?;
    optional(input, holder_set)?;
    bytes::<5>(input)?;
    sound(input)
}

pub(super) fn blocks_attacks(input: &mut &[u8]) -> Skipped {
    bytes::<8>(input)?;
    list(input, |input| {
        float(input)?;
        optional(input, holder_set)?;
        bytes::<8>(input)
    })?;
    bytes::<12>(input)?;
    optional(input, holder_set)?;
    optional(input, sound)?;
    optional(input, sound)
}

pub(super) fn kinetic_weapon(input: &mut &[u8]) -> Skipped {
    varint(input)?;
    varint(input)?;
    let condition = |input: &mut &[u8]| {
        varint(input)?;
        bytes::<8>(input)
    };
    (0..3).try_for_each(|_| optional(input, condition))?;
    bytes::<8>(input)?;
    optional(input, sound)?;
    optional(input, sound)
}

pub(super) fn firework_explosion(input: &mut &[u8]) -> Skipped {
    varint(input)?;
    list(input, bytes::<4>)?;
    list(input, bytes::<4>)?;
    bytes::<2>(input)
}

/// `ResolvableProfile.STREAM_CODEC`: a full or partial profile, then the skin patch.
pub(super) fn profile(input: &mut &[u8]) -> Skipped {
    either(
        input,
        |input| {
            bytes::<16>(input)?;
            string(input)?;
            profile_properties(input)
        },
        |input| {
            optional(input, string)?;
            optional(input, bytes::<16>)?;
            profile_properties(input)
        },
    )?;
    (0..3).try_for_each(|_| optional(input, string))?;
    optional(input, bytes::<1>)
}
