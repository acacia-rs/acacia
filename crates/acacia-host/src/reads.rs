//! Reads of a bot's published state from any thread, without a call or a `done`. Each is `None` for an unknown
//! bot, one not spawned yet, or with `HostConfig::snapshots` off. Versioned parts (cold, entities) have a cheap
//! `*_version` read so a transport can skip converting what it already holds.

use acacia_api::{
    ApiError, Block, BlockAtCursorParams, BlockAtCursorResult, BlockPos, BotId, ColdSnapshot, EntityList, ErrorCode,
    FindBlocksParams, FindBlocksResult, HotSnapshot, ItemTable,
};
use acacia_bot::snapshot::SnapshotReader;

use crate::{convert, EntityDelta, Host};

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

    /// [`Host::entities`] as changes since the reader's cursor, the version of the last delta it applied (`None`
    /// for a first read: a full one).
    pub fn entity_delta(&self, bot: &BotId, since: Option<u64>) -> Option<EntityDelta> {
        Some(EntityDelta(self.reader(bot)?.entity_delta(since)))
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
        convert::find_blocks(&self.known(bot)?, params)
    }

    /// `bot.blockAtCursor` without a call (`params.bot` is ignored).
    pub fn block_at_cursor(&self, bot: &BotId, params: &BlockAtCursorParams) -> Result<BlockAtCursorResult, ApiError> {
        convert::block_at_cursor(&self.known(bot)?, params)
    }

    fn known(&self, bot: &BotId) -> Result<SnapshotReader, ApiError> {
        self.reader(bot).ok_or_else(|| ApiError::new(ErrorCode::UnknownBot, format!("no bot {}", bot.0)))
    }
}
