//! Configuration state. `FinishConfiguration` is an empty body in both directions ([`super::Empty`]; the
//! serverbound one is the acknowledgement). Keep-alive is [`super::KeepAlive`], `SelectKnownPacks` is
//! [`super::SelectKnownPacks`] (serverbound lists are capped at 64 by vanilla), and the configuration ping is
//! [`crate::packets::Ping`], answered by a `Pong` with the same body.

use crate::server::read_rest;
use crate::tag::Tag;
use crate::{Error, Wire};

pub use crate::ids::configuration::{clientbound, serverbound};

/// `PackedRegistryEntry`: an identifier, then an optional network NBT payload (`ByteBufCodecs.optional(TAG)`,
/// so a presence flag, not an end tag). `None` means the entry comes from a known pack.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RegistryEntry {
    pub id: String,
    pub data: Option<Tag>,
}

impl Wire for RegistryEntry {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let id = String::read(input)?;
        let data = if bool::read(input)? { Some(Tag::read_root(input)?.ok_or(Error::Invalid("registry entry tag"))?) } else { None };
        Ok(Self { id, data })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.id.write(out);
        self.data.is_some().write(out);
        if let Some(tag) = &self.data {
            tag.write_root(out);
        }
    }
}

/// `ClientboundRegistryDataPacket`: the registry key, then its entries.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RegistryData {
    pub registry: String,
    pub entries: Vec<RegistryEntry>,
}

impl Wire for RegistryData {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { registry: String::read(input)?, entries: Vec::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.registry.write(out);
        self.entries.write(out);
    }
}

/// `ClientboundDisconnectPacket` (configuration and play): the reason as a network NBT component.
#[derive(Debug, Clone, PartialEq)]
pub struct Disconnect {
    pub reason: Tag,
}

impl Disconnect {
    pub fn text(message: &str) -> Self {
        Self { reason: Tag::String(message.to_owned()) }
    }
}

impl Wire for Disconnect {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { reason: Tag::read_root(input)?.ok_or(Error::Invalid("disconnect reason"))? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.reason.write_root(out);
    }
}

/// Custom payload, either direction: a channel identifier, then the data to the end of the frame (limits:
/// 1 MiB clientbound, 32767 serverbound). `minecraft:brand` is a string in `data`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CustomPayload {
    pub channel: String,
    pub data: Vec<u8>,
}

impl Wire for CustomPayload {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { channel: String::read(input)?, data: read_rest(input) })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.channel.write(out);
        out.extend_from_slice(&self.data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{Empty, KeepAlive, KnownPack, SelectKnownPacks, round_trip};

    #[test]
    fn ids_match_the_26_3_reports() {
        assert_eq!(
            [clientbound::CUSTOM_PAYLOAD, clientbound::DISCONNECT, clientbound::FINISH_CONFIGURATION, clientbound::KEEP_ALIVE, clientbound::REGISTRY_DATA, clientbound::SELECT_KNOWN_PACKS],
            [1, 2, 3, 4, 7, 15]
        );
        assert_eq!([serverbound::CUSTOM_PAYLOAD, serverbound::FINISH_CONFIGURATION, serverbound::KEEP_ALIVE, serverbound::SELECT_KNOWN_PACKS], [2, 3, 4, 7]);
    }

    #[test]
    fn registry_data_round_trips_with_and_without_nbt() {
        let data = Tag::Compound(vec![("scale".into(), Tag::Float(1.0)), ("name".into(), Tag::String("x".into()))]);
        let packet = RegistryData {
            registry: "minecraft:test".into(),
            entries: vec![RegistryEntry { id: "minecraft:a".into(), data: Some(data) }, RegistryEntry { id: "minecraft:b".into(), data: None }],
        };
        let bytes = round_trip(packet);
        assert_eq!(bytes[0] as usize, "minecraft:test".len());
        assert_eq!(bytes[15], 2, "two entries");
    }

    #[test]
    fn registry_entry_rejects_a_flagged_end_tag() {
        let mut bytes = Vec::new();
        "a".to_string().write(&mut bytes);
        bytes.extend([1, 0]);
        assert!(RegistryEntry::read(&mut &bytes[..]).is_err());
    }

    #[test]
    fn disconnect_is_a_nameless_nbt_component() {
        assert_eq!(round_trip(Disconnect::text("bye")), [8, 0, 3, b'b', b'y', b'e']);
    }

    #[test]
    fn small_packets_round_trip() {
        assert!(round_trip(Empty {}).is_empty());
        assert_eq!(round_trip(KeepAlive { id: 1 }), [0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(round_trip(crate::packets::Ping { id: 5 }), [0, 0, 0, 5]);
        let packs = SelectKnownPacks { packs: vec![KnownPack { namespace: "minecraft".into(), id: "core".into(), version: "26.3".into() }] };
        assert_eq!(round_trip(packs)[0], 1);
        let brand = CustomPayload { channel: "minecraft:brand".into(), data: vec![7, b'a', b'c', b'a', b'c', b'i', b'a', b'!'] };
        assert!(round_trip(brand).ends_with(b"acacia!"));
    }
}
