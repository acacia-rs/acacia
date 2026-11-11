use std::fmt;
use std::str::FromStr;

use crate::block::Block;
pub use crate::generated::items::Item;
use crate::generated::items::{FROM_BEDROCK, ITEMS};

pub(crate) struct ItemInfo {
    pub item: Item,
    pub name: &'static str,
    pub stack: u8,
    pub block: Option<Block>,
    pub bedrock: (&'static str, i16),
}

impl Item {
    pub fn all() -> impl ExactSizeIterator<Item = Item> {
        ITEMS.iter().map(|i| i.item)
    }

    fn info(self) -> &'static ItemInfo {
        &ITEMS[self as usize]
    }

    /// Namespaced Java name, e.g. `minecraft:oak_log`.
    pub fn name(self) -> &'static str {
        self.info().name
    }

    pub fn max_stack(self) -> u8 {
        self.info().stack
    }

    /// The block it places, for block items.
    pub fn block(self) -> Option<Block> {
        self.info().block
    }

    /// Java's network id.
    pub fn java_id(self) -> u16 {
        self as u16
    }

    pub fn from_java_id(id: u16) -> Option<Item> {
        ITEMS.get(usize::from(id)).map(|i| i.item)
    }

    /// Bedrock's name and data value. Data is 0 but where Bedrock merges items under one name (beds and banners
    /// by colour).
    pub fn bedrock(self) -> (&'static str, i16) {
        self.info().bedrock
    }

    /// The item for a Bedrock name and data value; `None` for items only Bedrock has.
    pub fn from_bedrock(name: &str, data: i16) -> Option<Item> {
        let at = FROM_BEDROCK.partition_point(|(n, _, _)| *n < name);
        FROM_BEDROCK[at..].iter().take_while(|(n, _, _)| *n == name).find(|(_, d, _)| *d == -1 || *d == data).map(|e| e.2)
    }

    /// Accepts Java's name, or a Bedrock name that needs no data value, with or without `minecraft:`.
    pub fn from_name(name: &str) -> Option<Item> {
        let path = name.strip_prefix("minecraft:").unwrap_or(name);
        if let Some(info) = ITEMS.iter().find(|i| i.name[10..] == *path) {
            return Some(info.item);
        }
        let full = format!("minecraft:{path}");
        let at = FROM_BEDROCK.partition_point(|(n, _, _)| *n < full.as_str());
        FROM_BEDROCK.get(at).filter(|(n, d, _)| *n == full && *d == -1).map(|e| e.2)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseItemError(pub String);

impl fmt::Display for ParseItemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown item `{}`", self.0)
    }
}

impl std::error::Error for ParseItemError {}

impl FromStr for Item {
    type Err = ParseItemError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Item::from_name(s).ok_or_else(|| ParseItemError(s.to_owned()))
    }
}

impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Item {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.name())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Item {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let name = <std::borrow::Cow<'de, str>>::deserialize(d)?;
        name.parse().map_err(serde::de::Error::custom)
    }
}
