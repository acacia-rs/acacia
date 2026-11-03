//! Reads of a bot's published state from any thread, without a call or a `done`. Each is `None` for an unknown
//! bot, one not spawned yet, or with `HostConfig::snapshots` off. Versioned parts (cold, entities) have a cheap
//! `*_version` read so a transport can skip converting what it already holds.

use acacia_api::{
    ApiError, Block, BlockPos, BotId, ColdSnapshot, Entity, EntityList, ErrorCode, FindBlocksParams, FindBlocksResult,
    HotSnapshot, ItemTable,
};

use crate::{convert, Host};

/// Numbers per entity in [`EntityFrame::numbers`]: feet x, y, z, velocity x, y, z, yaw, pitch, health (NaN when
/// the server sent none).
pub const ENTITY_NUMBERS: usize = 9;

/// The entity list split for readers that look every tick: what changes only when entities come or go, and the
/// numbers that change as they move, in the same order.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityFrame {
    /// As [`EntityList::version`]: moves on any change.
    pub version: u64,
    /// Moves when an entity comes or goes (or its ids, kind or name change).
    pub members_version: u64,
    /// The entities as of `members_version` (their numbers too); `None` when the reader said it holds them.
    pub members: Option<Vec<Entity>>,
    /// [`ENTITY_NUMBERS`] per member.
    pub numbers: Vec<f64>,
}

impl Host {
    /// The bot's state as of its last tick.
    pub fn hot(&self, bot: &BotId) -> Option<HotSnapshot> {
        Some(convert::hot(&self.reader(bot)?.hot()))
    }

    /// Inventory, effects and player list; `version` moves exactly when they change.
    pub fn cold(&self, bot: &BotId) -> Option<ColdSnapshot> {
        Some(convert::cold(&self.reader(bot)?.cold()))
    }

    pub fn cold_version(&self, bot: &BotId) -> Option<u64> {
        Some(self.reader(bot)?.cold().version)
    }

    /// Every tracked entity (empty unless the bot was created with `entities`); `version` moves when it changes.
    pub fn entities(&self, bot: &BotId) -> Option<EntityList> {
        Some(convert::entities(&self.reader(bot)?.entities()))
    }

    pub fn entities_version(&self, bot: &BotId) -> Option<u64> {
        Some(self.reader(bot)?.entities().version)
    }

    /// [`Host::entities`] as an [`EntityFrame`], leaving out the members when `known` is their version.
    pub fn entity_frame(&self, bot: &BotId, known: Option<u64>) -> Option<EntityFrame> {
        let list = self.reader(bot)?.entities();
        let members = (known != Some(list.members_version)).then(|| list.entities.iter().map(convert::entity).collect());
        let numbers = list
            .entities
            .iter()
            .flat_map(|e| {
                let ([x, y, z], [vx, vy, vz]) = (e.position, e.velocity);
                [x, y, z, vx, vy, vz, e.yaw, e.pitch, e.health.unwrap_or(f32::NAN)].map(f64::from)
            })
            .collect();
        Some(EntityFrame { version: list.version, members_version: list.members_version, members, numbers })
    }

    /// The block at a cell of the bot's current dimension; also `None` for a bot that tracks no terrain or a chunk
    /// it has not loaded.
    pub fn block_at(&self, bot: &BotId, pos: BlockPos) -> Option<Block> {
        let world = self.reader(bot)?.world()?;
        convert::block(&world, pos)
    }

    /// The server's item names (empty before its item table came; it comes once, at login).
    pub fn item_table(&self, bot: &BotId) -> Option<ItemTable> {
        Some(convert::item_table(&self.reader(bot)?.items()))
    }

    /// `bot.findBlocks` without a call (`params.bot` is ignored).
    pub fn find_blocks(&self, bot: &BotId, params: &FindBlocksParams) -> Result<FindBlocksResult, ApiError> {
        let reader = self.reader(bot).ok_or_else(|| ApiError::new(ErrorCode::UnknownBot, format!("no bot {}", bot.0)))?;
        convert::find_blocks(&reader, params)
    }
}
