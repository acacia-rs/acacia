use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A point (feet position, velocity).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// A block cell.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// One block of a bot's world, read without a call (`blockAt`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    /// Namespaced, e.g. `minecraft:stone`.
    pub name: String,
    /// The state's properties as Bedrock names them; booleans are `"0"` / `"1"`.
    pub states: BTreeMap<String, String>,
    /// The runtime id in this connection's block palette (custom blocks shift it, so never compare across servers).
    pub state_id: u32,
}

impl From<[f32; 3]> for Vec3 {
    fn from([x, y, z]: [f32; 3]) -> Self {
        Self { x, y, z }
    }
}

impl From<[i32; 3]> for BlockPos {
    fn from([x, y, z]: [i32; 3]) -> Self {
        Self { x, y, z }
    }
}

impl From<BlockPos> for [i32; 3] {
    fn from(p: BlockPos) -> Self {
        [p.x, p.y, p.z]
    }
}
