use std::path::PathBuf;

use acacia_client::PacketFilter;

use crate::events::{ChatPattern, Events};
use crate::state::Trackers;
use crate::survival::AutoEat;
use crate::world::SharedWorlds;

#[derive(Clone)]
pub struct BotConfig {
    /// Which trackers run. Entity tracking is off by default: on busy servers it is most of the
    /// decode work (a busy Geyser server sends ~300 entity moves/s), and AFK bots rarely need it.
    /// Skins are off too: they are only worth their memory to something that draws players.
    pub trackers: Trackers,
    /// Typed events [`crate::Bot::next`] returns (default: none).
    pub events: Events,
    /// Emit [`crate::BotEvent::ChatMatch`] when one of these matches a chat line.
    pub chat_patterns: Vec<ChatPattern>,
    /// Packets delivered to the caller as [`crate::BotEvent::Packet`] (default: none).
    pub subscribe: PacketFilter,
    /// Track terrain and simulate movement (needed to walk). Off by default: the bot still sends a
    /// vanilla standing-still `PlayerAuthInput` every tick ([`crate::movement::Idle`]).
    pub physics: bool,
    /// Share terrain between bots on the same server (pass the same value to each bot).
    pub shared_worlds: SharedWorlds,
    /// Respawn as soon as the server shows the death screen (default on). Off: call [`crate::Bot::respawn`].
    pub auto_respawn: bool,
    /// Record a movement trace here (physics bots only; see [`crate::trace`]).
    pub record: Option<PathBuf>,
    /// Eat when hungry (default off), moving food into the hotbar if needed; also [`crate::Bot::set_auto_eat`].
    pub auto_eat: Option<AutoEat>,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            trackers: Trackers::default(),
            events: Events::NONE,
            chat_patterns: Vec::new(),
            subscribe: PacketFilter::none(),
            physics: false,
            shared_worlds: SharedWorlds::new(),
            auto_respawn: true,
            record: None,
            auto_eat: None,
        }
    }
}
