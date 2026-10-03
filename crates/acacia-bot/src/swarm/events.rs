use serde::Serialize;

use super::spec::{BotId, Target};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SwarmEvent {
    Joining { id: BotId, target: Target, attempt: u32 },
    Spawned { id: BotId, display_name: String },
    Disconnected { id: BotId, reason: String },
    Reconnecting { id: BotId, after_ms: u64 },
    /// The bot stopped for good (needs a person, or the policy gave up); it stays listed until removed.
    Failed { id: BotId, error: String },
    /// Removed by the caller, or its task returned while connected.
    Removed { id: BotId },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum BotStatus {
    Waiting,
    Connecting,
    Online,
    Backoff,
    Failed { error: String },
}

#[derive(Clone, Debug, Serialize)]
pub struct BotInfo {
    pub id: BotId,
    pub target: Target,
    pub shard: usize,
    pub status: BotStatus,
}

/// What a coordinator needs for placement: every bot, and how loaded each shard is.
#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub bots: Vec<BotInfo>,
    pub shard_load: Vec<usize>,
    pub draining: bool,
}
