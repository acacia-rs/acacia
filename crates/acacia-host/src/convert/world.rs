use acacia_api::{ApiError, Block, BlockPos, ErrorCode, FindBlocksParams, FindBlocksResult, ItemName, ItemTable};
use acacia_bot::snapshot::SnapshotReader;
use acacia_bot::state::ItemRegistry;
use acacia_world::World;

/// `bot.findBlocks`' caps, so a sync read stays bounded (a 64-block sphere is under 1.1 M cells, palette-skipped).
const MAX_DISTANCE: u32 = 64;
const MAX_COUNT: u32 = 1024;

/// `None` when the chunk is not loaded.
pub(crate) fn block(world: &World, p: BlockPos) -> Option<Block> {
    let id = world.get(p.x >> 4, p.z >> 4)?.read().block(p.x, p.y, p.z);
    let state = world.registry().get(id)?;
    let states = state.properties.split(',').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.to_owned(), v.to_owned())).collect();
    Some(Block { name: state.name.to_owned(), states, state_id: id })
}

/// `bot.findBlocks` around the bot's feet, from its published terrain.
pub(crate) fn find_blocks(reader: &SnapshotReader, p: &FindBlocksParams) -> Result<FindBlocksResult, ApiError> {
    let world = reader.world().ok_or_else(|| ApiError::new(ErrorCode::NotPossible, "the bot tracks no terrain yet"))?;
    let ids = acacia_world::block_ids(world.registry(), &p.matching);
    if ids.is_empty() {
        return Err(ApiError::new(ErrorCode::BadParams, format!("no block is named any of {:?}", p.matching)));
    }
    let feet = reader.hot().position.map(|v| v.floor() as i32);
    let (distance, count) = (p.max_distance.unwrap_or(16).min(MAX_DISTANCE), p.count.unwrap_or(1).min(MAX_COUNT));
    let found = acacia_world::find_blocks(&world, feet, &ids, distance, count as usize);
    Ok(FindBlocksResult { blocks: found.into_iter().map(|[x, y, z]| BlockPos { x, y, z }).collect() })
}

pub(crate) fn item_table(items: &ItemRegistry) -> ItemTable {
    let mut items: Vec<_> = items.iter().map(|(id, name)| ItemName { id, name: name.to_owned() }).collect();
    items.sort_unstable_by_key(|i| i.id);
    ItemTable { items }
}
