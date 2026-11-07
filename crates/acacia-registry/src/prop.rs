//! Typed block properties: one marker type per property, `state.get(prop::Axis)`.

pub use crate::generated::props::*;

pub(crate) struct PropInfo {
    pub key: &'static str,
    pub values: &'static [&'static str],
}

/// A block property. `VALUES` is in state order, so a value's index is its digit in the state id.
pub trait Property: Copy {
    type Value: Copy + PartialEq + 'static;
    const ID: PropId;
    const VALUES: &'static [Self::Value];
}

impl PropId {
    /// The key as each edition's own state string spells it (Java's for Java properties).
    pub fn key(self) -> &'static str {
        PROPS[self as usize].key
    }

    /// Serialized values in state order.
    pub fn values(self) -> &'static [&'static str] {
        PROPS[self as usize].values
    }

    pub(crate) fn count(self) -> u32 {
        PROPS[self as usize].values.len() as u32
    }
}
