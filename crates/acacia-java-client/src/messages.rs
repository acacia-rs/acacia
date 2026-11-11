//! Incoming chat, system text and death messages, as plain text (`ClientboundPlayerChatPacket` and friends).

use acacia_java_proto::dialog::text;
use acacia_java_proto::tag::Tag;
use acacia_java_proto::{Error, Wire, ids, read_varint};

use ids::play::clientbound as from;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    /// A player's message (`PlayerChat`), signed or not.
    Player,
    /// A player's message the server sent as `DisguisedChat` (no sender profile).
    Disguised,
    System,
    /// System text above the hotbar.
    ActionBar,
    /// The local player's death message (`PlayerCombatKill`).
    Death,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub kind: MessageKind,
    /// The `chat_type` registry name of a player message (`minecraft:chat`, `minecraft:msg_command_incoming`, ...).
    pub chat_type: Option<String>,
    /// The sender's display name, for player messages.
    pub sender: Option<String>,
    pub sender_uuid: Option<u128>,
    /// The message itself, undecorated: no `<name>` prefix.
    pub text: String,
}

const SIGNATURE_BYTES: usize = 256;
const FILTER_PARTIAL: i32 = 2;

/// The message `id` carries, if it is one; death messages only for `player_id`.
pub(crate) fn read(id: i32, mut input: &[u8], chat_types: &[String], player_id: i32) -> Result<Option<Message>, Error> {
    let input = &mut input;
    let plain = |kind, text| Message { kind, chat_type: None, sender: None, sender_uuid: None, text };
    Ok(match id {
        from::SYSTEM_CHAT => {
            let content = component(input)?;
            Some(plain(if bool::read(input)? { MessageKind::ActionBar } else { MessageKind::System }, content))
        }
        from::PLAYER_COMBAT_KILL => (read_varint(input)? == player_id).then(|| component(input)).transpose()?.map(|t| plain(MessageKind::Death, t)),
        from::DISGUISED_CHAT => {
            let content = component(input)?;
            let (chat_type, sender) = bound(input, chat_types)?;
            Some(Message { kind: MessageKind::Disguised, chat_type, sender, sender_uuid: None, text: content })
        }
        from::PLAYER_CHAT => Some(player_chat(input, chat_types)?),
        _ => None,
    })
}

fn player_chat(input: &mut &[u8], chat_types: &[String]) -> Result<Message, Error> {
    read_varint(input)?;
    let uuid = u128::read(input)?;
    read_varint(input)?;
    if bool::read(input)? {
        skip(input, SIGNATURE_BYTES)?;
    }
    let content = String::read(input)?;
    i64::read(input)?;
    i64::read(input)?;
    for _ in 0..read_varint(input)? {
        if read_varint(input)? == 0 {
            skip(input, SIGNATURE_BYTES)?;
        }
    }
    let unsigned = if bool::read(input)? { Some(component(input)?) } else { None };
    if read_varint(input)? == FILTER_PARTIAL {
        for _ in 0..read_varint(input)? {
            i64::read(input)?;
        }
    }
    let (chat_type, sender) = bound(input, chat_types)?;
    Ok(Message { kind: MessageKind::Player, chat_type, sender, sender_uuid: Some(uuid), text: unsigned.unwrap_or(content) })
}

/// `ChatType.Bound`: the type and the sender's name. An inline type (holder id 0) leaves both unknown, since its
/// decoration can't be skipped without a full `ChatType` codec.
fn bound(input: &mut &[u8], chat_types: &[String]) -> Result<(Option<String>, Option<String>), Error> {
    let holder = read_varint(input)?;
    if holder == 0 {
        return Ok((None, None));
    }
    let chat_type = usize::try_from(holder - 1).ok().and_then(|i| chat_types.get(i)).cloned();
    Ok((chat_type, Some(component(input)?)))
}

fn component(input: &mut &[u8]) -> Result<String, Error> {
    Ok(Tag::read_root(input)?.as_ref().map(text).unwrap_or_default())
}

fn skip(input: &mut &[u8], n: usize) -> Result<(), Error> {
    *input = input.get(n..).ok_or(Error::Invalid("message signature"))?;
    Ok(())
}

#[cfg(test)]
#[path = "messages_tests.rs"]
mod tests;
