//! Block entity NBT to what the renderer's block models take from it.

use acacia_bot::proto::nbt::{Nbt, Value};
use acacia_bot::state::BlockEntities;
use acacia_render::block_models::BlockDataMap;
use acacia_render::blocks::model::BlockData;

pub fn snapshot(entities: &BlockEntities) -> BlockDataMap {
    entities.iter().filter_map(|(pos, nbt)| Some((*pos, data(nbt)?))).collect()
}

/// `None` when the block entity says nothing a model uses.
fn data(nbt: &Nbt) -> Option<BlockData> {
    let int = |key: &str| match nbt.value.get(key)? {
        Value::Int(v) => Some(*v),
        Value::Byte(v) => Some(i32::from(*v)),
        _ => None,
    };
    let rotation = match nbt.value.get("Rotation") {
        Some(Value::Float(degrees)) => Some(*degrees),
        _ => None,
    };
    let pair = int("pairx").zip(int("pairz")).map(|(x, z)| ([x, z], int("pairlead") == Some(1)));
    let data = BlockData { color: int("color").map(|c| c as u8), rotation, pair };
    (data != BlockData::default()).then_some(data)
}
