//! Component <-> NBT. A plain unstyled string collapses to a bare string tag, anything else is a compound.

use super::{Component, Content, Style};
use crate::Error;
use crate::tag::Tag;

const MAX_DEPTH: u32 = 64;

impl Component {
    pub fn to_tag(&self) -> Tag {
        if let Some(text) = self.plain_text() {
            return Tag::String(text.to_owned());
        }
        let mut entries = match &self.content {
            Content::Text(text) => vec![("text".to_owned(), Tag::String(text.clone()))],
            Content::Translatable { key, fallback, args } => {
                let mut entries = vec![("translate".to_owned(), Tag::String(key.clone()))];
                entries.extend(fallback.iter().map(|f| ("fallback".to_owned(), Tag::String(f.clone()))));
                if !args.is_empty() {
                    entries.push(("with".to_owned(), Tag::List(args.iter().map(Component::to_tag).collect())));
                }
                entries
            }
        };
        entries.extend(self.style.entries());
        if !self.extra.is_empty() {
            entries.push(("extra".to_owned(), Tag::List(self.extra.iter().map(Component::to_tag).collect())));
        }
        Tag::Compound(entries)
    }

    pub fn from_tag(tag: &Tag) -> Result<Self, Error> {
        Self::from_tag_at(tag, 0)
    }

    fn from_tag_at(tag: &Tag, depth: u32) -> Result<Self, Error> {
        if depth >= MAX_DEPTH {
            return Err(Error::TooDeep(MAX_DEPTH));
        }
        match tag {
            Tag::String(s) => Ok(Self::text(s.as_str())),
            Tag::Byte(_) | Tag::Short(_) | Tag::Int(_) | Tag::Long(_) | Tag::Float(_) | Tag::Double(_) => {
                Ok(Self::text(number_text(tag)))
            }
            Tag::List(items) => {
                let mut parts = items.iter().map(|t| Self::from_tag_at(t, depth + 1));
                let mut first = parts.next().ok_or(Error::Invalid("empty component list"))??;
                for sibling in parts {
                    first.extra.push(sibling?);
                }
                Ok(first)
            }
            Tag::Compound(_) => Self::from_compound(tag, depth),
            _ => Err(Error::Invalid("text component")),
        }
    }

    fn from_compound(tag: &Tag, depth: u32) -> Result<Self, Error> {
        let components = |name: &str| -> Result<Vec<Component>, Error> {
            match tag.get(name) {
                Some(Tag::List(items)) => items.iter().map(|t| Self::from_tag_at(t, depth + 1)).collect(),
                Some(_) => Err(Error::Invalid("component list")),
                None => Ok(Vec::new()),
            }
        };
        let content = match (tag.get("text"), tag.get("translate")) {
            (Some(Tag::String(text)), _) => Content::Text(text.clone()),
            (_, Some(Tag::String(key))) => Content::Translatable {
                key: key.clone(),
                fallback: tag.get("fallback").and_then(Tag::as_str).map(str::to_owned),
                args: components("with")?,
            },
            _ => return Err(Error::Invalid("component content")),
        };
        Ok(Self { content, style: Style::from_compound(tag)?, extra: components("extra")? })
    }
}

fn number_text(tag: &Tag) -> String {
    match tag {
        Tag::Float(v) => v.to_string(),
        Tag::Double(v) => v.to_string(),
        Tag::Byte(v) => v.to_string(),
        Tag::Short(v) => v.to_string(),
        Tag::Int(v) => v.to_string(),
        Tag::Long(v) => v.to_string(),
        _ => String::new(),
    }
}
