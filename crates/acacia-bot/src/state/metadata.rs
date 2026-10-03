//! Entity metadata from spawn packets and `SetEntityData`; each update replaces only the keys it carries.

use std::collections::HashMap;

use acacia_client::proto::types::{
    MetadataDictionaryItem, MetadataDictionaryItemKey as Key, MetadataDictionaryItemValue as Value,
    MetadataDictionaryItemValueDefault as Plain, MetadataFlags1, MetadataFlags2,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Metadata(HashMap<Key, Value>);

impl Metadata {
    pub fn get(&self, key: Key) -> Option<&Value> {
        self.0.get(&key)
    }

    /// Plain (non-flag) value of `key`.
    pub fn plain(&self, key: Key) -> Option<&Plain> {
        match self.get(key)? {
            Value::Default(v) => Some(v),
            _ => None,
        }
    }

    pub fn flags(&self) -> MetadataFlags1 {
        match self.get(Key::Flags) {
            Some(Value::Flags(f)) => *f,
            _ => MetadataFlags1::default(),
        }
    }

    pub fn flags_extended(&self) -> MetadataFlags2 {
        match self.get(Key::FlagsExtended) {
            Some(Value::FlagsExtended(f)) => *f,
            _ => MetadataFlags2::default(),
        }
    }

    pub fn name_tag(&self) -> Option<&str> {
        match self.plain(Key::Nametag)? {
            Plain::String(s) => Some(s),
            _ => None,
        }
    }

    pub(crate) fn merge(&mut self, items: Vec<MetadataDictionaryItem>) {
        self.0.extend(items.into_iter().map(|i| (i.key, i.value)));
    }
}

impl From<Vec<MetadataDictionaryItem>> for Metadata {
    fn from(items: Vec<MetadataDictionaryItem>) -> Self {
        let mut m = Metadata::default();
        m.merge(items);
        m
    }
}
