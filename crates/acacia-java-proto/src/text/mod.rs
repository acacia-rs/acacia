//! Text components: plain and translatable text with a colour and the five style flags, plus siblings. Network form
//! is a nameless NBT root (`ComponentSerialization.TRUSTED_STREAM_CODEC`), see [`nbt`]; JSON form (status
//! descriptions, login disconnects) is [`Component::to_json`]. Layouts were checked with `javap -p -c` on
//! `ComponentSerialization`, `Style$Serializer`, `TranslatableContents` and `ListTag`.

mod json;
mod legacy;
mod nbt;
mod number_format;
pub mod style;

use crate::tag::Tag;
use crate::{Error, Wire};

pub use number_format::NumberFormat;
pub use style::{Color, NamedColor, Style};

#[derive(Debug, Clone, PartialEq)]
pub enum Content {
    Text(String),
    Translatable { key: String, fallback: Option<String>, args: Vec<Component> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    pub content: Content,
    pub style: Style,
    pub extra: Vec<Component>,
}

impl Component {
    pub fn text(text: impl Into<String>) -> Self {
        Self { content: Content::Text(text.into()), style: Style::default(), extra: Vec::new() }
    }

    pub fn empty() -> Self {
        Self::text("")
    }

    pub fn translate(key: impl Into<String>, args: Vec<Component>) -> Self {
        Self { content: Content::Translatable { key: key.into(), fallback: None, args }, style: Style::default(), extra: Vec::new() }
    }

    /// Shown instead of the key when the client has no translation. No-op on plain text.
    pub fn fallback(mut self, text: impl Into<String>) -> Self {
        if let Content::Translatable { fallback, .. } = &mut self.content {
            *fallback = Some(text.into());
        }
        self
    }

    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.style.color = Some(color.into());
        self
    }

    pub fn bold(mut self, on: bool) -> Self {
        self.style.bold = Some(on);
        self
    }

    pub fn italic(mut self, on: bool) -> Self {
        self.style.italic = Some(on);
        self
    }

    pub fn underlined(mut self, on: bool) -> Self {
        self.style.underlined = Some(on);
        self
    }

    pub fn strikethrough(mut self, on: bool) -> Self {
        self.style.strikethrough = Some(on);
        self
    }

    pub fn obfuscated(mut self, on: bool) -> Self {
        self.style.obfuscated = Some(on);
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn append(mut self, sibling: Component) -> Self {
        self.extra.push(sibling);
        self
    }

    /// `Component.tryCollapseToString`: the text of a component that is nothing but unstyled literal text.
    pub(crate) fn plain_text(&self) -> Option<&str> {
        match &self.content {
            Content::Text(s) if self.style.is_empty() && self.extra.is_empty() => Some(s),
            _ => None,
        }
    }

    /// Legacy `§` codes (colours `0-f`, `k l m n o` flags, `r` reset) as a styled component.
    pub fn from_legacy(text: &str) -> Self {
        legacy::parse(text)
    }
}

impl From<&str> for Component {
    fn from(text: &str) -> Self {
        Self::text(text)
    }
}

impl From<String> for Component {
    fn from(text: String) -> Self {
        Self::text(text)
    }
}

impl Wire for Component {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Self::from_tag(&Tag::read_root(input)?.ok_or(Error::Invalid("text component"))?)
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.to_tag().write_root(out);
    }
}

#[cfg(test)]
mod tests;
