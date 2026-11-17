//! The player list: `ClientboundPlayerInfoUpdatePacket` and `ClientboundPlayerInfoRemovePacket`.

use std::collections::HashMap;

use acacia_java_proto::ids::play::clientbound as from;
use acacia_java_proto::{read_varint, Error, Wire};

use crate::messages::component;

const ADD_PLAYER: u8 = 1;
const INITIALIZE_CHAT: u8 = 1 << 1;
const UPDATE_GAME_MODE: u8 = 1 << 2;
const UPDATE_LISTED: u8 = 1 << 3;
const UPDATE_LATENCY: u8 = 1 << 4;
const UPDATE_DISPLAY_NAME: u8 = 1 << 5;
const UPDATE_LIST_ORDER: u8 = 1 << 6;
const UPDATE_HAT: u8 = 1 << 7;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerInfo {
    pub name: String,
    /// `GameType` id: 0 survival, 1 creative, 2 adventure, 3 spectator.
    pub game_mode: i32,
    pub latency: i32,
    pub listed: bool,
    pub display_name: Option<String>,
}

/// Everyone the server has listed, by profile UUID. A new level's `Login` starts a fresh list.
#[derive(Clone, Debug, Default)]
pub struct Players(HashMap<u128, PlayerInfo>);

impl Players {
    pub fn get(&self, uuid: u128) -> Option<&PlayerInfo> {
        self.0.get(&uuid)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&u128, &PlayerInfo)> {
        self.0.iter()
    }

    pub(crate) fn apply(&mut self, id: i32, mut input: &[u8]) -> Result<(), Error> {
        let input = &mut input;
        match id {
            from::LOGIN => self.0.clear(),
            from::PLAYER_INFO_REMOVE => {
                for _ in 0..read_varint(input)? {
                    self.0.remove(&u128::read(input)?);
                }
            }
            from::PLAYER_INFO_UPDATE => {
                let actions = u8::read(input)?;
                for _ in 0..read_varint(input)? {
                    let uuid = u128::read(input)?;
                    let entry = self.0.entry(uuid).or_default();
                    read_entry(entry, actions, input)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// One entry's fields, in `Action` order.
fn read_entry(entry: &mut PlayerInfo, actions: u8, input: &mut &[u8]) -> Result<(), Error> {
    if actions & ADD_PLAYER != 0 {
        entry.name = String::read(input)?;
        for _ in 0..read_varint(input)? {
            (String::read(input)?, String::read(input)?);
            if bool::read(input)? {
                String::read(input)?;
            }
        }
    }
    if actions & INITIALIZE_CHAT != 0 && bool::read(input)? {
        // `RemoteChatSession.Data`: session id, then the profile key's expiry, key and signature.
        (u128::read(input)?, i64::read(input)?, <Vec<u8>>::read(input)?, <Vec<u8>>::read(input)?);
    }
    if actions & UPDATE_GAME_MODE != 0 {
        entry.game_mode = read_varint(input)?;
    }
    if actions & UPDATE_LISTED != 0 {
        entry.listed = bool::read(input)?;
    }
    if actions & UPDATE_LATENCY != 0 {
        entry.latency = read_varint(input)?;
    }
    if actions & UPDATE_DISPLAY_NAME != 0 {
        entry.display_name = if bool::read(input)? { Some(component(input)?) } else { None };
    }
    if actions & UPDATE_LIST_ORDER != 0 {
        read_varint(input)?;
    }
    if actions & UPDATE_HAT != 0 {
        bool::read(input)?;
    }
    Ok(())
}
