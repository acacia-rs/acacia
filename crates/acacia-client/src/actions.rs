//! Convenience senders for common player actions, encoded the way the vanilla client does.

use acacia_session::proto::manual::Uuid;
use acacia_session::proto::packets::{CommandRequest, Text, TextCategory, TextContent, TextContentChat, TextType};
use acacia_session::proto::types::CommandOrigin;

use crate::driver::Command;
use crate::Client;

impl Client {
    /// Sends a chat message as this player. Returns false if the connection has closed.
    pub fn chat(&self, message: impl Into<String>) -> bool {
        self.send(&Text {
            needs_translation: false,
            category: TextCategory::Authored,
            r#type: TextType::Chat,
            content: TextContent::Chat(TextContentChat { source_name: self.display_name().to_owned(), message: message.into() }),
            xuid: self.xuid().to_owned(),
            platform_chat_id: String::new(),
            has_filtered_message: false,
            filtered_message: None,
        })
    }

    /// Runs a command as this player; the leading `/` is optional. Like vanilla, each request has a
    /// fresh random origin UUID (capture 2026-10-02: version 4, new per command).
    pub fn command(&self, command: &str) -> bool {
        let command = if command.starts_with('/') { command.to_owned() } else { format!("/{command}") };
        self.send(&CommandRequest {
            command,
            origin: CommandOrigin { r#type: "player".into(), uuid: random_uuid(), request_id: String::new(), player_entity_id: 0 },
            internal: false,
            // BDS 1.26.52 disconnects on anything but "latest" (verified against "1", "" and "1.26.51").
            version: "latest".into(),
        })
    }

    /// Sends a private message (`/tell`); names with spaces are quoted.
    pub fn whisper(&self, to: &str, message: &str) -> bool {
        let to = if to.contains(' ') { format!("\"{to}\"") } else { to.to_owned() };
        self.command(&format!("tell {to} {message}"))
    }

    /// Requests a respawn after death (see `acacia_session::Session::respawn`). Harmless while alive.
    pub fn respawn(&self) -> bool {
        self.command_raw(Command::Respawn)
    }

    /// Sends what vanilla sends once it is back in the world after a respawn (see
    /// `acacia_session::Session::send_respawn_done`); [`Client::respawn`] sends it on its own.
    pub fn respawn_done(&self) -> bool {
        self.command_raw(Command::RespawnDone)
    }
}

/// A random version-4 UUID in wire order (`Uuid`: each 8-byte half reversed, so the canonical version
/// byte 6 is wire byte 1 and the variant byte 8 is wire byte 15).
fn random_uuid() -> Uuid {
    use rand_core::{OsRng, RngCore};
    let mut b = [0u8; 16];
    OsRng.fill_bytes(&mut b);
    b[1] = (b[1] & 0x0f) | 0x40;
    b[15] = (b[15] & 0x3f) | 0x80;
    Uuid(b)
}

#[cfg(test)]
mod tests {
    #[test]
    fn command_uuids_are_fresh_version_4() {
        let (a, b) = (super::random_uuid(), super::random_uuid());
        assert_ne!(a, b);
        for s in [a.to_string(), b.to_string()] {
            assert_eq!(&s[14..15], "4", "{s}");
            assert!("89ab".contains(&s[19..20]), "{s}");
        }
    }
}
