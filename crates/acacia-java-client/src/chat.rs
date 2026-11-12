//! Chat and commands. On an online-mode server, with the account's profile key, they are signed as vanilla signs
//! them (secure chat); otherwise they go out unsigned, as offline-mode and session-free servers take them.
// TODO: refresh the profile key after its `refreshed_after` (`ProfileKeyPairManager.shouldRefreshKeyPair`).

use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use acacia_auth::{Account, JavaProfile};
use acacia_java_proto::chat::{ArgumentSignature, Chat, ChatAck, LastSeenUpdate, MessageSignature, PackedSignature, PlayerChatSignatures, SignedChatCommand};
use acacia_java_proto::ids::play::{clientbound as from, serverbound as to};
use acacia_java_proto::packets::ChatCommand;
use acacia_java_proto::{VarInt, Wire};

use crate::command_tree::CommandTree;
use crate::frame::Error;
use crate::join::Client;
use crate::last_seen::{SignatureCache, Tracker};
use crate::signing::{Body, ChatKey, Session, salt};

/// `ServerboundChatPacket`'s message limit, in characters.
const MAX_CHAT: usize = 256;
/// `markMessageAsProcessed` acknowledges once more than this many messages are unacknowledged.
const ACK_AFTER: i32 = 64;

/// The client's secure chat state (`ClientPacketListener`'s chat session, tracker and cache).
pub(crate) struct ChatState {
    /// The profile key, or why there is none.
    key: Result<ChatKey, String>,
    session: Option<Session>,
    tracker: Tracker,
    cache: SignatureCache,
    commands: Option<CommandTree>,
}

impl Default for ChatState {
    fn default() -> Self {
        Self { key: Err("joined offline".into()), session: None, tracker: Tracker::default(), cache: SignatureCache::default(), commands: None }
    }
}

impl ChatState {
    /// The account's profile key for an online join; a failure leaves chat unsigned, as in vanilla.
    pub(crate) async fn fetch(account: &Account, player: &JavaProfile) -> Mutex<Self> {
        let key = match account.java_profile_key(player).await {
            Ok(key) => ChatKey::new(key).map_err(|e| e.to_string()),
            Err(e) => Err(format!("profile key: {e}")),
        };
        Mutex::new(Self { key, ..Self::default() })
    }

    /// Signs `body` with the session's next link; `None` without a session.
    fn sign(&mut self, body: &Body) -> Option<MessageSignature> {
        let key = self.key.as_ref().ok()?;
        self.session.as_mut()?.sign(key, body)
    }
}

fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

impl Client {
    fn chat_state(&self) -> MutexGuard<'_, ChatState> {
        self.chat_state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Whether chat goes out signed: a chat session is announced.
    pub fn signs_chat(&self) -> bool {
        self.chat_state().session.is_some()
    }

    /// Why an online join has no profile key (chat then goes out unsigned).
    pub fn profile_key_error(&self) -> Option<String> {
        self.chat_state().key.as_ref().err().cloned()
    }

    /// Runs `command` (without the leading `/`). With a chat session, its message arguments are signed
    /// (`sendCommand`); see `command_tree.rs` for how they are found.
    pub fn command(&self, command: &str) -> Result<(), Error> {
        let command = command.trim_start_matches('/');
        let mut chat = self.chat_state();
        let signable = match (&chat.session, &chat.commands) {
            (Some(_), Some(tree)) => tree.signable(command),
            _ => Vec::new(),
        };
        if signable.is_empty() {
            return self.send(to::CHAT_COMMAND, &ChatCommand { command: command.to_owned() });
        }
        let (timestamp_millis, salt) = (now_millis(), salt());
        let (seen, last_seen) = chat.tracker.update();
        let arguments = signable
            .into_iter()
            .filter_map(|(name, value)| {
                let signature = chat.sign(&Body { content: &value, timestamp_millis, salt, last_seen: &seen })?;
                Some(ArgumentSignature { name, signature })
            })
            .collect();
        self.send(to::CHAT_COMMAND_SIGNED, &SignedChatCommand { command: command.to_owned(), timestamp_millis, salt, arguments, last_seen })
    }

    /// Says `text`, cut to the protocol's 256 characters; signed with a chat session (`sendChat`).
    pub fn chat(&self, text: &str) -> Result<(), Error> {
        let message: String = text.chars().take(MAX_CHAT).collect();
        let (timestamp_millis, salt) = (now_millis(), salt());
        let mut chat = self.chat_state();
        let (signature, last_seen) = if chat.session.is_some() {
            let (seen, update) = chat.tracker.update();
            (chat.sign(&Body { content: &message, timestamp_millis, salt, last_seen: &seen }), update)
        } else {
            (None, LastSeenUpdate::default())
        };
        self.send(to::CHAT, &Chat { message, timestamp_millis, salt, signature, last_seen })
    }

    /// Keeps the chat session, acknowledgements and command tree in step with the server.
    pub(crate) fn apply_chat(&self, id: i32, body: &[u8]) -> Result<(), Error> {
        let mut chat = self.chat_state();
        match id {
            // `handleLogin`: a fresh session for an online-mode server, whose flag is the packet's second-last byte.
            from::LOGIN => {
                let online = body.len() >= 2 && body[body.len() - 2] != 0;
                chat.tracker = Tracker::default();
                chat.cache = SignatureCache::default();
                chat.session = None;
                if let (true, Ok(key)) = (online, &chat.key) {
                    let session = Session::new(self.uuid);
                    self.send(to::CHAT_SESSION_UPDATE, &session.update(key))?;
                    chat.session = Some(session);
                }
            }
            from::COMMANDS => chat.commands = CommandTree::read(&mut &body[..]).ok(),
            from::PLAYER_CHAT => {
                let Ok(head) = PlayerChatSignatures::read(&mut &body[..]) else { return Ok(()) };
                // Vanilla disconnects on a signature id it doesn't hold; we only stop tracking.
                let Some(seen) = head.last_seen.iter().map(|p| chat.cache.unpack(p)).collect::<Option<Vec<_>>>() else { return Ok(()) };
                chat.cache.push(seen, head.signature.clone());
                if let Some(signature) = head.signature
                    && chat.tracker.add_pending(signature, true)
                    && chat.tracker.offset() > ACK_AFTER
                {
                    self.send(to::CHAT_ACK, &ChatAck { offset: VarInt(chat.tracker.take_offset()) })?;
                }
            }
            from::DELETE_CHAT => {
                if let Some(signature) = PackedSignature::read(&mut &body[..]).ok().and_then(|p| chat.cache.unpack(&p)) {
                    chat.tracker.ignore_pending(&signature);
                }
            }
            // Vanilla's acknowledgement on `StartConfiguration` is skipped: the link has already left play, and the
            // server's next play listener starts a fresh `LastSeenMessagesValidator`.
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "chat_tests.rs"]
mod tests;
