use std::collections::HashMap;

use acacia_client::proto::packets::ItemRegistry as ItemRegistryPacket;
use acacia_client::proto::{DecodeError, Packet, RawPacket};

/// Network item id <-> identifier (`minecraft:diamond`), from the server's `ItemRegistry`.
#[derive(Debug, Default)]
pub struct ItemRegistry {
    names: HashMap<i32, String>,
    ids: HashMap<String, i32>,
}

impl ItemRegistry {
    pub const PACKETS: &'static [u32] = &[ItemRegistryPacket::ID];

    pub fn apply(&mut self, packet: &RawPacket) -> Result<(), DecodeError> {
        let registry: ItemRegistryPacket = packet.decode()?;
        self.names.clear();
        self.ids.clear();
        for item in registry.itemstates {
            let id = i32::from(item.runtime_id);
            self.ids.insert(item.name.clone(), id);
            self.names.insert(id, item.name);
        }
        Ok(())
    }

    pub fn name(&self, network_id: i32) -> Option<&str> {
        self.names.get(&network_id).map(String::as_str)
    }

    pub fn id(&self, name: &str) -> Option<i32> {
        self.ids.get(name).copied()
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use acacia_client::proto::nbt::Raw;
    use acacia_client::proto::types::{ItemstatesItem, ItemstatesItemVersion};

    use super::*;
    use crate::state::queries::test_support::{fixtures, raw};

    fn entry(name: &str, runtime_id: i16) -> ItemstatesItem {
        ItemstatesItem {
            name: name.into(),
            runtime_id,
            component_based: false,
            version: ItemstatesItemVersion::Legacy,
            nbt: Raw::default(),
        }
    }

    #[test]
    fn maps_both_directions_and_replaces_on_resend() {
        let mut items = ItemRegistry::default();
        let packet = ItemRegistryPacket { itemstates: vec![entry("minecraft:stone", 1), entry("minecraft:diamond", 304)] };
        items.apply(&raw(&packet)).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items.name(304), Some("minecraft:diamond"));
        assert_eq!(items.id("minecraft:stone"), Some(1));
        assert_eq!(items.name(5), None);

        items.apply(&raw(&ItemRegistryPacket { itemstates: vec![entry("minecraft:apple", 257)] })).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items.id("minecraft:stone"), None);
    }

    #[test]
    fn decodes_fixture() {
        for packet in fixtures::<ItemRegistryPacket>() {
            let expected: ItemRegistryPacket = packet.decode().unwrap();
            let mut items = ItemRegistry::default();
            items.apply(&packet).unwrap();
            for item in &expected.itemstates {
                assert!(items.id(&item.name).is_some());
            }
        }
    }
}
