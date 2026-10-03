use std::collections::HashMap;

use acacia_client::proto::manual::Uuid;
use acacia_client::proto::packets::PlayerList as PlayerListPacket;
use acacia_client::proto::types::{PlayerRecordContent, PlayerRecordContentAdd};
use acacia_client::proto::{DecodeError, Packet, RawPacket};

/// One player in the tab list. Skins are not kept.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerListEntry {
    pub uuid: Uuid,
    pub username: String,
    pub xuid: String,
    pub entity_unique_id: i64,
    pub platform_chat_id: String,
    pub build_platform: i32,
}

impl From<PlayerRecordContentAdd> for PlayerListEntry {
    fn from(r: PlayerRecordContentAdd) -> Self {
        Self {
            uuid: r.uuid,
            username: r.username,
            xuid: r.xbox_user_id,
            entity_unique_id: r.entity_unique_id,
            platform_chat_id: r.platform_chat_id,
            build_platform: r.build_platform,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PlayerChange {
    Joined(PlayerListEntry),
    Left(PlayerListEntry),
}

/// Online players by UUID.
#[derive(Debug, Default)]
pub struct PlayerList {
    players: HashMap<Uuid, PlayerListEntry>,
    /// Joins and leaves since the bot last drained them (an Add for a known UUID is an update).
    pub(crate) changes: Vec<PlayerChange>,
}

impl PlayerList {
    pub const PACKETS: &'static [u32] = &[PlayerListPacket::ID];

    pub fn get(&self, uuid: &Uuid) -> Option<&PlayerListEntry> {
        self.players.get(uuid)
    }

    /// Case-insensitive, like Bedrock usernames.
    pub fn by_name(&self, name: &str) -> Option<&PlayerListEntry> {
        self.players.values().find(|p| p.username.eq_ignore_ascii_case(name))
    }

    pub fn len(&self) -> usize {
        self.players.len()
    }

    pub fn is_empty(&self) -> bool {
        self.players.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &PlayerListEntry> {
        self.players.values()
    }

    pub fn apply(&mut self, packet: &RawPacket) -> Result<(), DecodeError> {
        let p: PlayerListPacket = packet.decode()?;
        for record in p.records {
            match record.content {
                PlayerRecordContent::Add(add) => {
                    let entry = PlayerListEntry::from(add);
                    if self.players.insert(entry.uuid, entry.clone()).is_none() {
                        self.changes.push(PlayerChange::Joined(entry));
                    }
                }
                PlayerRecordContent::Remove(r) => {
                    if let Some(entry) = self.players.remove(&r.uuid) {
                        self.changes.push(PlayerChange::Left(entry));
                    }
                }
                PlayerRecordContent::Default => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
