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

    /// Int value of `key` (variant, mark variant, skin id, trade tier...), 0 when unset.
    pub fn int(&self, key: Key) -> i32 {
        match self.plain(key) {
            Some(Plain::Int(v)) => *v,
            _ => 0,
        }
    }

    /// Dye colour index (sheep wool, collars, shulkers).
    pub fn color(&self) -> i8 {
        match self.plain(Key::Color) {
            Some(Plain::Byte(v)) => *v,
            _ => 0,
        }
    }

    pub fn scale(&self) -> f32 {
        match self.plain(Key::Scale) {
            Some(Plain::Float(v)) => *v,
            _ => 1.0,
        }
    }

    /// Collision box width and height in blocks, when the server sent them (every mob gets them at spawn).
    pub fn bounding_box(&self) -> Option<(f32, f32)> {
        match (self.plain(Key::BoundingboxWidth)?, self.plain(Key::BoundingboxHeight)?) {
            (Plain::Float(w), Plain::Float(h)) => Some((*w, *h)),
            _ => None,
        }
    }

    /// The rider's wire position against its vehicle, in the vehicle's frame (`RiderSeatPosition`).
    pub fn seat_position(&self) -> Option<[f32; 3]> {
        match self.plain(Key::RiderSeatPosition)? {
            Plain::Vec3f(v) => Some([v.x, v.y, v.z]),
            _ => None,
        }
    }

    /// Degrees a seat that locks its rider's rotation (`RiderRotationLocked`) turns the rider from the
    /// vehicle's yaw (`RiderSeatRotationOffset`); `None` on a seat that leaves the rider free.
    pub fn seat_turn(&self) -> Option<f32> {
        let locked = matches!(self.plain(Key::RiderRotationLocked), Some(Plain::Byte(b)) if *b != 0);
        match self.plain(Key::RiderSeatRotationOffset) {
            Some(Plain::Float(v)) if locked => Some(*v),
            _ => locked.then_some(0.0),
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
