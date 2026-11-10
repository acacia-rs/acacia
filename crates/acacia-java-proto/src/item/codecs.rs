//! Skippers for the `ByteBufCodecs` building blocks item components are composed of.

use crate::wire::{read_len, take};
use crate::{Error, Wire, nbt, read_varint};

pub(super) type Skipped = Result<(), Error>;

pub(super) fn varint(input: &mut &[u8]) -> Skipped {
    read_varint(input).map(drop)
}

pub(super) fn bytes<const N: usize>(input: &mut &[u8]) -> Skipped {
    take(input, N).map(drop)
}

pub(super) fn float(input: &mut &[u8]) -> Skipped {
    bytes::<4>(input)
}

pub(super) fn boolean(input: &mut &[u8]) -> Result<bool, Error> {
    bool::read(input)
}

/// `STRING_UTF8`, and every `Identifier`/`ResourceKey`, which go as their string.
pub(super) fn string(input: &mut &[u8]) -> Skipped {
    let len = read_len(input)?;
    take(input, len).map(drop)
}

/// `ComponentSerialization.STREAM_CODEC` and its trusted twin: a text component as an NBT root.
pub(super) fn text(input: &mut &[u8]) -> Skipped {
    nbt::skip_tag(input).map(drop)
}

/// `ByteBufCodecs.optional`: a presence flag.
pub(super) fn optional(input: &mut &[u8], value: impl FnOnce(&mut &[u8]) -> Skipped) -> Skipped {
    if boolean(input)? { value(input) } else { Ok(()) }
}

/// `ByteBufCodecs.either`: true for the left codec.
pub(super) fn either(input: &mut &[u8], left: impl FnOnce(&mut &[u8]) -> Skipped, right: impl FnOnce(&mut &[u8]) -> Skipped) -> Skipped {
    if boolean(input)? { left(input) } else { right(input) }
}

/// `ByteBufCodecs.list`/`collection`/`map`: a varint count, then each element.
pub(super) fn list(input: &mut &[u8], mut element: impl FnMut(&mut &[u8]) -> Skipped) -> Skipped {
    for _ in 0..read_len(input)? {
        element(input)?;
    }
    Ok(())
}

/// `ByteBufCodecs.holder`: a registry id plus one, or 0 and the value inline.
pub(super) fn holder(input: &mut &[u8], direct: impl FnOnce(&mut &[u8]) -> Skipped) -> Skipped {
    if read_varint(input)? == 0 { direct(input) } else { Ok(()) }
}

/// `ByteBufCodecs.holderSet`: 0 and a tag name, or the id count plus one and the ids.
pub(super) fn holder_set(input: &mut &[u8]) -> Skipped {
    match read_varint(input)? {
        0 => string(input),
        n if n < 0 => Err(Error::NegativeLength(n)),
        n => (1..n).try_for_each(|_| varint(input)),
    }
}

/// `SoundEvent.STREAM_CODEC`: a holder whose inline form is a location and an optional fixed range.
pub(super) fn sound(input: &mut &[u8]) -> Skipped {
    holder(input, |input| {
        string(input)?;
        optional(input, float)
    })
}

/// `MobEffectInstance.STREAM_CODEC`: the effect, then its recursive `Details`.
pub(super) fn mob_effect(input: &mut &[u8]) -> Skipped {
    varint(input)?;
    effect_details(input, 0)
}

fn effect_details(input: &mut &[u8], depth: u32) -> Skipped {
    if depth >= super::MAX_DEPTH {
        return Err(Error::TooDeep(super::MAX_DEPTH));
    }
    varint(input)?;
    varint(input)?;
    bytes::<3>(input)?;
    optional(input, |input| effect_details(input, depth + 1))
}

/// `ConsumeEffect.STREAM_CODEC`, dispatched on `consume_effect_type`.
pub(super) fn consume_effect(input: &mut &[u8]) -> Skipped {
    match read_varint(input)? {
        0 => {
            list(input, mob_effect)?;
            float(input)
        }
        1 => holder_set(input),
        2 => Ok(()),
        3 => bytes::<5>(input),
        4 => sound(input),
        _ => Err(Error::Invalid("consume effect type")),
    }
}

/// `Filterable.streamCodec`: the raw value and an optional filtered one.
pub(super) fn filterable(input: &mut &[u8], value: impl Fn(&mut &[u8]) -> Skipped) -> Skipped {
    value(input)?;
    optional(input, value)
}

/// `GAME_PROFILE_PROPERTIES`: name, value, nullable signature.
pub(super) fn profile_properties(input: &mut &[u8]) -> Skipped {
    list(input, |input| {
        string(input)?;
        string(input)?;
        optional(input, string)
    })
}

/// `ResolvableInt`/`ResolvableFloat`: a 4-byte constant, or a provider key.
pub(super) fn resolvable(input: &mut &[u8]) -> Skipped {
    either(input, bytes::<4>, string)
}
