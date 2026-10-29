use acacia_api::{Block, BlockPos};
use acacia_world::World;

/// `None` when the chunk is not loaded.
pub(crate) fn block(world: &World, p: BlockPos) -> Option<Block> {
    let id = world.get(p.x >> 4, p.z >> 4)?.read().block(p.x, p.y, p.z);
    let state = world.registry().get(id)?;
    let states = state.properties.split(',').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.to_owned(), v.to_owned())).collect();
    Some(Block { name: state.name.to_owned(), states, state_id: id })
}
