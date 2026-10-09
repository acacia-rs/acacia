//! Block entity NBT to what the renderer's block models take from it.

use std::sync::Arc;

use acacia_bot::proto::nbt::{Nbt, Value};
use acacia_bot::state::BlockEntities;
use acacia_render::banner::Banner;
use acacia_render::block_models::BlockDataMap;
use acacia_render::blocks::model::BlockData;

pub fn snapshot(entities: &BlockEntities) -> BlockDataMap {
    entities.iter().filter_map(|(pos, nbt)| Some((*pos, data(nbt)?))).collect()
}

fn int(nbt: &Value, key: &str) -> Option<i32> {
    match nbt.get(key)? {
        Value::Int(v) => Some(*v),
        Value::Byte(v) => Some(i32::from(*v)),
        _ => None,
    }
}

/// `None` when the block entity says nothing a model uses.
fn data(nbt: &Nbt) -> Option<BlockData> {
    let int = |key: &str| int(&nbt.value, key);
    let rotation = match nbt.value.get("Rotation") {
        Some(Value::Float(degrees)) => Some(*degrees),
        _ => None,
    };
    let pair = int("pairx").zip(int("pairz")).map(|(x, z)| ([x, z], int("pairlead") == Some(1)));
    let data = BlockData { banner: banner(&nbt.value).map(Arc::new), color: int("color").map(|c| c as u8), rotation, pair };
    (data != BlockData::default()).then_some(data)
}

/// A banner's `Base` dye, `Patterns` (each a `Pattern` code and `Color`) and `Type` (1: ominous).
fn banner(nbt: &Value) -> Option<Banner> {
    let patterns = match nbt.get("Patterns") {
        Some(Value::List(patterns)) => &patterns.items[..],
        _ => &[],
    };
    fn layer(pattern: &Value) -> Option<(&str, i32)> {
        match pattern.get("Pattern")? {
            Value::String(code) => Some((&**code, int(pattern, "Color")?)),
            _ => None,
        }
    }
    Some(Banner::from_bedrock(int(nbt, "Base")?, patterns.iter().filter_map(layer), int(nbt, "Type").unwrap_or(0)))
}
