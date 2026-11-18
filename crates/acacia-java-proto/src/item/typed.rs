//! Typed values for the components a server writes into a stack's patch. Each layout was read off the 26.3
//! `DataComponents` registration and its `STREAM_CODEC` with `javap -p -c`; [`super::components::skip`] is the
//! independent check, since every value read back goes through it. Set with [`Components::set`], read with
//! [`Components::get`]. The multi-field ones (books, potions, profiles, attribute modifiers) are in `rich`.

use super::{Components, read_len};
use crate::text::Component;
use crate::{Error, Wire, read_varint, write_varint};

pub const ITEM_NAME: i32 = 9;
pub const LORE: i32 = 11;
pub const ATTRIBUTE_MODIFIERS: i32 = 16;
pub const ENCHANTMENT_GLINT_OVERRIDE: i32 = 21;
pub const STORED_ENCHANTMENTS: i32 = 45;
pub const DYED_COLOR: i32 = 47;
pub const POTION_CONTENTS: i32 = 53;
pub const SUSPICIOUS_STEW_EFFECTS: i32 = 55;
pub const WRITABLE_BOOK_CONTENT: i32 = 56;
pub const WRITTEN_BOOK_CONTENT: i32 = 57;
pub const TRIM: i32 = 58;
pub const PROFILE: i32 = 72;
pub const BANNER_PATTERNS: i32 = 74;

/// The ids kept as raw values when a patch is read.
pub(super) const CAPTURED: [i32; 13] = [
    ITEM_NAME,
    LORE,
    ATTRIBUTE_MODIFIERS,
    ENCHANTMENT_GLINT_OVERRIDE,
    STORED_ENCHANTMENTS,
    DYED_COLOR,
    POTION_CONTENTS,
    SUSPICIOUS_STEW_EFFECTS,
    WRITABLE_BOOK_CONTENT,
    WRITTEN_BOOK_CONTENT,
    TRIM,
    PROFILE,
    BANNER_PATTERNS,
];

/// `ItemLore.STREAM_CODEC` is `list(256)`: a longer lore fails the client's decode.
pub const MAX_LORE_LINES: usize = 256;

/// A component value with its `data_component_type` id.
pub trait ComponentValue: Sized {
    const ID: i32;
    fn read_value(input: &mut &[u8]) -> Result<Self, Error>;
    fn write_value(&self, out: &mut Vec<u8>);
}

impl Components {
    pub fn set<C: ComponentValue>(&mut self, value: &C) {
        let mut raw = Vec::new();
        value.write_value(&mut raw);
        self.put_raw(C::ID, raw);
    }

    /// `extra` stays sorted by id, so equal patches compare equal whatever order they were built in.
    fn put_raw(&mut self, id: i32, raw: Vec<u8>) {
        self.extra.retain(|(have, _)| *have != id);
        self.extra.push((id, raw));
        self.extra.sort_by_key(|&(id, _)| id);
    }

    pub fn with<C: ComponentValue>(mut self, value: &C) -> Self {
        self.set(value);
        self
    }

    /// The value of `C` if the patch carries it.
    pub fn get<C: ComponentValue>(&self) -> Result<Option<C>, Error> {
        let Some((_, raw)) = self.extra.iter().find(|(id, _)| *id == C::ID) else { return Ok(None) };
        let mut input = &raw[..];
        let value = C::read_value(&mut input)?;
        if input.is_empty() { Ok(Some(value)) } else { Err(Error::Invalid("component value length")) }
    }

    pub fn set_custom_name(&mut self, name: &Component) {
        let mut raw = Vec::new();
        name.write(&mut raw);
        self.custom_name = Some(raw);
    }
}

/// Reads `id`'s value off `input` by skipping it, and keeps its bytes.
pub(super) fn capture(id: i32, input: &mut &[u8], depth: u32, components: &mut Components) -> Result<(), Error> {
    let start = *input;
    super::components::skip(id, input, depth)?;
    components.put_raw(id, start[..start.len() - input.len()].to_vec());
    Ok(())
}

macro_rules! component_value {
    ($ty:ty, $id:expr, |$input:ident| $read:expr, |$this:ident, $out:ident| $write:expr) => {
        impl ComponentValue for $ty {
            const ID: i32 = $id;
            fn read_value($input: &mut &[u8]) -> Result<Self, Error> {
                $read
            }
            fn write_value(&self, $out: &mut Vec<u8>) {
                let $this = self;
                $write
            }
        }
    };
}

mod rich;
pub use rich::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ItemName(pub Component);
component_value!(ItemName, ITEM_NAME, |input| Component::read(input).map(Self), |v, out| v.0.write(out));

/// Lines past [`MAX_LORE_LINES`] are dropped on write.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Lore(pub Vec<Component>);
component_value!(Lore, LORE, |input| Vec::read(input).map(Self), |v, out| {
    write_varint(v.0.len().min(MAX_LORE_LINES) as i32, out);
    v.0.iter().take(MAX_LORE_LINES).for_each(|line| line.write(out));
});

/// `DyedItemColor`: an RGB int.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DyedColor(pub i32);
component_value!(DyedColor, DYED_COLOR, |input| i32::read(input).map(Self), |v, out| v.0.write(out));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlintOverride(pub bool);
component_value!(GlintOverride, ENCHANTMENT_GLINT_OVERRIDE, |input| bool::read(input).map(Self), |v, out| v.0.write(out));

/// `ItemEnchantments.STREAM_CODEC` as `(enchantment registry id, level)`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StoredEnchantments(pub Vec<(i32, i32)>);
component_value!(StoredEnchantments, STORED_ENCHANTMENTS, |input| read_pairs(input).map(Self), |v, out| write_pairs(&v.0, out));

/// `(mob_effect registry id, duration in ticks)` for each effect.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SuspiciousStewEffects(pub Vec<(i32, i32)>);
component_value!(SuspiciousStewEffects, SUSPICIOUS_STEW_EFFECTS, |input| read_pairs(input).map(Self), |v, out| write_pairs(&v.0, out));

fn read_pairs(input: &mut &[u8]) -> Result<Vec<(i32, i32)>, Error> {
    (0..read_len(input)?).map(|_| Ok((read_varint(input)?, read_varint(input)?))).collect()
}

fn write_pairs(pairs: &[(i32, i32)], out: &mut Vec<u8>) {
    write_varint(pairs.len() as i32, out);
    pairs.iter().for_each(|&(a, b)| [a, b].iter().for_each(|&v| write_varint(v, out)));
}

/// `ArmorTrim`: two registry holders by registry id (the wire holds id + 1; 0 would start an inline value).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trim {
    pub material: i32,
    pub pattern: i32,
}
component_value!(
    Trim,
    TRIM,
    |input| Ok(Self { material: read_holder(input)?, pattern: read_holder(input)? }),
    |v, out| [v.material, v.pattern].iter().for_each(|&id| write_varint(id + 1, out))
);

fn read_holder(input: &mut &[u8]) -> Result<i32, Error> {
    match read_varint(input)? {
        0 => Err(Error::Invalid("inline registry holder")),
        n => Ok(n - 1),
    }
}

/// One layer: a `banner_pattern` registry id (written as a holder, id + 1) and a dye colour id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BannerLayer {
    pub pattern: i32,
    pub color: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BannerPatterns(pub Vec<BannerLayer>);
component_value!(
    BannerPatterns,
    BANNER_PATTERNS,
    |input| (0..read_len(input)?).map(|_| Ok(BannerLayer { pattern: read_holder(input)?, color: read_varint(input)? })).collect::<Result<_, _>>().map(Self),
    |v, out| {
        write_varint(v.0.len() as i32, out);
        for layer in &v.0 {
            write_varint(layer.pattern + 1, out);
            write_varint(layer.color, out);
        }
    }
);
