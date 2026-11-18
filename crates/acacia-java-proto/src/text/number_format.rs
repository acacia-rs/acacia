//! `NumberFormatTypes.STREAM_CODEC`: a `number_format_type` registry id, then the body. Scoreboard score formats.

use super::{Component, Style};
use crate::registry::NUMBER_FORMAT_TYPE;
use crate::tag::Tag;
use crate::{Error, VarInt, Wire};

#[derive(Debug, Clone, PartialEq)]
pub enum NumberFormat {
    Blank,
    /// The score as digits in this style.
    Styled(Style),
    /// This text instead of the score.
    Fixed(Component),
}

impl NumberFormat {
    fn type_id(&self) -> i32 {
        let name = match self {
            Self::Blank => "blank",
            Self::Styled(_) => "styled",
            Self::Fixed(_) => "fixed",
        };
        NUMBER_FORMAT_TYPE.id_lenient(name).expect("generated registry has the number format types")
    }
}

impl Wire for NumberFormat {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let id = VarInt::read(input)?.0;
        match NUMBER_FORMAT_TYPE.name(id) {
            Some("minecraft:blank") => Ok(Self::Blank),
            Some("minecraft:styled") => {
                let tag = Tag::read_root(input)?.ok_or(Error::Invalid("style"))?;
                Style::from_compound(&tag).map(Self::Styled)
            }
            Some("minecraft:fixed") => Component::read(input).map(Self::Fixed),
            _ => Err(Error::Invalid("number format")),
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        VarInt(self.type_id()).write(out);
        match self {
            Self::Blank => {}
            Self::Styled(style) => Tag::Compound(style.entries()).write_root(out),
            Self::Fixed(text) => text.write(out),
        }
    }
}

/// `ByteBufCodecs.optional`: a presence flag, then the value.
impl Wire for Option<NumberFormat> {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        if bool::read(input)? { NumberFormat::read(input).map(Some) } else { Ok(None) }
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.is_some().write(out);
        if let Some(format) = self {
            format.write(out);
        }
    }
}
