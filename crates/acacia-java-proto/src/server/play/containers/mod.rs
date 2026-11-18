//! Play-state packets for containers, items, the HUD, chat, commands and scoreboards, in the server role: bodies the
//! server writes and the ones it reads. Every body is a [`Wire`], so both directions round-trip. Ids are in
//! [`crate::ids::play`]. Layouts come from `javap -p -c` on the 26.3 codec classes; where azalea's differed
//! (docs/PROTOCOL_AUDIT.md in the bridge) these follow the codec. Container, slot, merchant and item bodies that
//! already exist at the crate root are re-exported here so the bridge has one path for them.

/// A packet body with no `Default`, so text components can be fields.
macro_rules! body {
    ($(#[$meta:meta])* $name:ident { $($(#[$fmeta:meta])* $field:ident: $ty:ty),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name { $($(#[$fmeta])* pub $field: $ty),* }
        impl crate::Wire for $name {
            #[allow(unused_variables)]
            fn read(input: &mut &[u8]) -> Result<Self, crate::Error> {
                Ok(Self { $($field: <$ty as crate::Wire>::read(input)?),* })
            }
            #[allow(unused_variables)]
            fn write(&self, out: &mut Vec<u8>) {
                $(crate::Wire::write(&self.$field, out);)*
            }
        }
    };
}

mod boss;
mod chat;
mod commands;
mod hud;
mod scoreboard;
mod windows;

pub use crate::container::{ContainerClick, ContainerClose, ContainerInput, ContainerSetData, HashedStack, OpenScreen};
pub use crate::inventory::{ContainerSetContent, ContainerSetSlot, PLAYER_CONTAINER, SetCursorItem, SetHeldSlot, SetPlayerInventory};
pub use crate::merchant::{ItemCost, MerchantOffer, MerchantOffers};
pub use crate::packets::{ChatCommand, ContainerButtonClick, PlaceRecipe, RenameItem, SelectTrade, SetBeacon, SetHealth, UnsignedChat};
pub use boss::*;
pub use chat::*;
pub use commands::*;
pub use hud::*;
pub use scoreboard::*;
pub use windows::*;

use crate::text::Component;
use crate::wire::{read_len, take};
use crate::{Error, Wire};

/// A string the codec caps in UTF-16 units (`ByteBufCodecs.stringUtf8(max)`); a longer one fails the decode.
pub(crate) fn read_capped_string(input: &mut &[u8], max_units: usize) -> Result<String, Error> {
    let text = String::read(input)?;
    if text.encode_utf16().count() > max_units {
        return Err(Error::Invalid("string longer than its cap"));
    }
    Ok(text)
}

/// A list length the codec caps (`ByteBufCodecs.list(max)`).
pub(crate) fn read_capped_len(input: &mut &[u8], max: usize) -> Result<usize, Error> {
    let len = read_len(input)?;
    if len > max {
        return Err(Error::Invalid("list longer than its cap"));
    }
    Ok(len)
}

/// A length-prefixed byte array of at most `max` bytes.
pub(crate) fn read_capped_bytes(input: &mut &[u8], max: usize) -> Result<Vec<u8>, Error> {
    let len = read_capped_len(input, max)?;
    Ok(take(input, len)?.to_vec())
}

/// `ComponentSerialization.TRUSTED_OPTIONAL_STREAM_CODEC`: a presence flag, then the component.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OptionalText(pub Option<Component>);

impl Wire for OptionalText {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        if bool::read(input)? { Component::read(input).map(|c| Self(Some(c))) } else { Ok(Self(None)) }
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.0.is_some().write(out);
        if let Some(text) = &self.0 {
            text.write(out);
        }
    }
}

/// `FriendlyByteBuf.writeNullable` of a string.
impl Wire for NullableString {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        if bool::read(input)? { String::read(input).map(|s| Self(Some(s))) } else { Ok(Self(None)) }
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.0.is_some().write(out);
        if let Some(text) = &self.0 {
            text.write(out);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NullableString(pub Option<String>);

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_windows;
