//! `Style` (colour and the five on/off flags) and `TextColor`. Click and hover events, insertion, font and shadow
//! colour are not modelled: nothing here sends them.

use crate::Error;
use crate::tag::Tag;

macro_rules! named_colors {
    ($($variant:ident $name:literal $code:literal $rgb:literal),* $(,)?) => {
        /// The sixteen `ChatFormatting` colours, serialised by name.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum NamedColor { $($variant),* }

        impl NamedColor {
            pub const ALL: [Self; 16] = [$(Self::$variant),*];

            pub fn name(self) -> &'static str {
                match self { $(Self::$variant => $name),* }
            }

            pub fn rgb(self) -> u32 {
                match self { $(Self::$variant => $rgb),* }
            }

            /// The `§` formatting code character.
            pub fn code(self) -> char {
                match self { $(Self::$variant => $code),* }
            }

            pub fn from_name(name: &str) -> Option<Self> {
                match name { $($name => Some(Self::$variant),)* _ => None }
            }

            pub fn from_code(code: char) -> Option<Self> {
                match code.to_ascii_lowercase() { $($code => Some(Self::$variant),)* _ => None }
            }
        }
    };
}

named_colors! {
    Black "black" '0' 0x000000,
    DarkBlue "dark_blue" '1' 0x0000AA,
    DarkGreen "dark_green" '2' 0x00AA00,
    DarkAqua "dark_aqua" '3' 0x00AAAA,
    DarkRed "dark_red" '4' 0xAA0000,
    DarkPurple "dark_purple" '5' 0xAA00AA,
    Gold "gold" '6' 0xFFAA00,
    Gray "gray" '7' 0xAAAAAA,
    DarkGray "dark_gray" '8' 0x555555,
    Blue "blue" '9' 0x5555FF,
    Green "green" 'a' 0x55FF55,
    Aqua "aqua" 'b' 0x55FFFF,
    Red "red" 'c' 0xFF5555,
    LightPurple "light_purple" 'd' 0xFF55FF,
    Yellow "yellow" 'e' 0xFFFF55,
    White "white" 'f' 0xFFFFFF,
}

/// `TextColor`: a named colour or `#RRGGBB`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Named(NamedColor),
    Rgb(u32),
}

impl Color {
    pub fn rgb(self) -> u32 {
        match self {
            Self::Named(n) => n.rgb(),
            Self::Rgb(v) => v & 0xFF_FFFF,
        }
    }

    pub fn serialize(self) -> String {
        match self {
            Self::Named(n) => n.name().to_owned(),
            Self::Rgb(v) => format!("#{:06X}", v & 0xFF_FFFF),
        }
    }

    pub fn parse(text: &str) -> Result<Self, Error> {
        if let Some(n) = NamedColor::from_name(text) {
            return Ok(Self::Named(n));
        }
        text.strip_prefix('#')
            .filter(|hex| hex.len() == 6)
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .map(Self::Rgb)
            .ok_or(Error::Invalid("text colour"))
    }
}

impl From<NamedColor> for Color {
    fn from(n: NamedColor) -> Self {
        Self::Named(n)
    }
}

/// Unset fields inherit from the parent component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub color: Option<Color>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underlined: Option<bool>,
    pub strikethrough: Option<bool>,
    pub obfuscated: Option<bool>,
}

impl Style {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// The `Style.Serializer` fields, in vanilla's order, for embedding in a component compound.
    pub(crate) fn entries(&self) -> Vec<(String, Tag)> {
        let flag = |name: &str, v: Option<bool>| v.map(|b| (name.to_owned(), Tag::Byte(i8::from(b))));
        let color = self.color.map(|c| ("color".to_owned(), Tag::String(c.serialize())));
        [
            color,
            flag("bold", self.bold),
            flag("italic", self.italic),
            flag("underlined", self.underlined),
            flag("strikethrough", self.strikethrough),
            flag("obfuscated", self.obfuscated),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    /// Reads the style fields of a compound, ignoring every other key.
    pub(crate) fn from_compound(tag: &Tag) -> Result<Self, Error> {
        let flag = |name: &str| tag.get(name).map(|t| t.as_bool().ok_or(Error::Invalid("style flag"))).transpose();
        let color = match tag.get("color") {
            Some(Tag::String(s)) => Some(Color::parse(s)?),
            Some(_) => return Err(Error::Invalid("text colour")),
            None => None,
        };
        Ok(Self {
            color,
            bold: flag("bold")?,
            italic: flag("italic")?,
            underlined: flag("underlined")?,
            strikethrough: flag("strikethrough")?,
            obfuscated: flag("obfuscated")?,
        })
    }
}
