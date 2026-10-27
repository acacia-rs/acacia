use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

use acacia_client::proto::packets::ItemRegistry as ItemRegistryPacket;
use acacia_client::proto::{DecodeError, Packet, RawPacket};
use bytes::Bytes;

/// Network item id <-> identifier (`minecraft:diamond`), from the server's `ItemRegistry`. Bots that
/// receive the same registry share one copy (~300 KB each).
#[derive(Debug, Default)]
pub struct ItemRegistry(Arc<Maps>);

#[derive(Debug, Default)]
struct Maps {
    names: HashMap<i32, Arc<str>>,
    ids: HashMap<Arc<str>, i32>,
}

impl ItemRegistry {
    pub const PACKETS: &'static [u32] = &[ItemRegistryPacket::ID];

    pub fn apply(&mut self, packet: &RawPacket) -> Result<(), DecodeError> {
        static LAST: Mutex<Option<(Bytes, Weak<Maps>)>> = Mutex::new(None);
        let body = super::kept::keep(packet).body;
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        // keep() hands identical bodies the same buffer.
        if let Some((seen, maps)) = &*last
            && seen.as_ptr() == body.as_ptr()
            && let Some(maps) = maps.upgrade()
        {
            self.0 = maps;
            return Ok(());
        }
        let registry: ItemRegistryPacket = packet.decode()?;
        let mut maps = Maps::default();
        for item in registry.itemstates {
            let id = i32::from(item.runtime_id);
            let name: Arc<str> = item.name.into();
            maps.ids.insert(name.clone(), id);
            maps.names.insert(id, name);
        }
        self.0 = Arc::new(maps);
        *last = Some((body, Arc::downgrade(&self.0)));
        Ok(())
    }

    pub fn name(&self, network_id: i32) -> Option<&str> {
        self.0.names.get(&network_id).map(|n| &**n)
    }

    pub fn id(&self, name: &str) -> Option<i32> {
        self.0.ids.get(name).copied()
    }

    pub fn len(&self) -> usize {
        self.0.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.names.is_empty()
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
