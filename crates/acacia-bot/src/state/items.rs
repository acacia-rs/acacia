use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

use acacia_client::proto::packets::ItemRegistry as ItemRegistryPacket;
use acacia_world::neutral::Item;
use acacia_client::proto::{DecodeError, Packet, RawPacket};
use bytes::Bytes;

/// Network item id <-> identifier (`minecraft:diamond`), from the server's `ItemRegistry`. Bots that
/// receive the same registry share one copy (~300 KB each).
#[derive(Debug, Default, Clone)]
pub struct ItemRegistry(Arc<Maps>);

#[derive(Debug, Default)]
struct Maps {
    names: HashMap<i32, Arc<str>>,
    ids: HashMap<Arc<str>, i32>,
    /// Network ids are Java item ids; names are the Bedrock spellings the rest of the bot compares.
    java: bool,
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

    /// Java's items, which every Java server shares: network id = Java item id.
    pub fn java() -> Self {
        static JAVA: std::sync::LazyLock<Arc<Maps>> = std::sync::LazyLock::new(|| {
            let mut maps = Maps { java: true, ..Maps::default() };
            for item in Item::all() {
                let (id, name): (i32, Arc<str>) = (i32::from(item.java_id()), item.bedrock().0.into());
                maps.ids.entry(name.clone()).or_insert(id);
                maps.names.insert(id, name);
            }
            Arc::new(maps)
        });
        Self(JAVA.clone())
    }

    pub fn name(&self, network_id: i32) -> Option<&str> {
        self.0.names.get(&network_id).map(|n| &**n)
    }

    pub fn id(&self, name: &str) -> Option<i32> {
        self.0.ids.get(name).copied()
    }

    /// The neutral item of a network id and its metadata (which tells bed and banner colours apart);
    /// `None` for items only Bedrock has.
    pub fn item(&self, network_id: i32, metadata: u32) -> Option<Item> {
        if self.0.java {
            return Item::from_java_id(u16::try_from(network_id).ok()?);
        }
        Item::from_bedrock(self.name(network_id)?, i16::try_from(metadata).unwrap_or(-1))
    }

    /// The network id and metadata this server uses for `item`.
    pub fn network_id(&self, item: Item) -> Option<(i32, u32)> {
        let (name, data) = item.bedrock();
        if self.0.java {
            return Some((i32::from(item.java_id()), data as u32));
        }
        Some((self.id(name)?, data as u32))
    }

    /// Whether both are the same table (one `ItemRegistry` packet), without comparing entries.
    pub fn same(&self, other: &ItemRegistry) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Every `(network id, name)`, in no order.
    pub fn iter(&self) -> impl Iterator<Item = (i32, &str)> {
        self.0.names.iter().map(|(id, name)| (*id, &**name))
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
    fn java_ids_name_bedrock_spellings() {
        let items = ItemRegistry::java();
        let bed = Item::from_java_id(Item::all().find(|i| i.name() == "minecraft:red_bed").unwrap().java_id()).unwrap();
        let (id, data) = items.network_id(bed).unwrap();
        assert_eq!(items.item(id, data), Some(bed));
        assert_eq!(items.name(id), Some("minecraft:bed"));
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
