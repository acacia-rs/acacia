//! Commands and chat, unsigned: what an offline-mode or chat-session-free server accepts.
// TODO: signed chat (a profile key and chat session) for online servers that enforce secure chat.

use std::time::{SystemTime, UNIX_EPOCH};

use acacia_java_proto::ids::play::serverbound as to;
use acacia_java_proto::packets::{ChatCommand, UnsignedChat};

use crate::frame::Error;
use crate::join::Client;

/// `ServerboundChatPacket`'s message limit, in characters.
const MAX_CHAT: usize = 256;

impl Client {
    /// Runs `command` (without the leading `/`).
    pub fn command(&self, command: &str) -> Result<(), Error> {
        self.send(to::CHAT_COMMAND, &ChatCommand { command: command.trim_start_matches('/').to_owned() })
    }

    /// Says `text`, cut to the protocol's 256 characters.
    pub fn chat(&self, text: &str) -> Result<(), Error> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
        let message = text.chars().take(MAX_CHAT).collect();
        self.send(to::CHAT, &UnsignedChat { message, timestamp_millis: now, salt: now.rotate_left(17) ^ 0x5DEE_CE66 })
    }
}
