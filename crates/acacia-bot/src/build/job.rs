//! What to build, as the caller saves and passes back after a restart. Progress is never stored: a builder
//! re-surveys the world against the structure when it starts.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::transform::Transform;
use crate::pathfind::DEFAULT_SCAFFOLD;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildJob {
    /// The `.mcstructure` file.
    pub structure: PathBuf,
    pub at: Transform,
    /// Containers to restock from (chests, barrels, shulker boxes, ender chests), by block position.
    #[serde(default)]
    pub stations: Vec<[i32; 3]>,
    /// Only place into empty cells; leave cells holding another block as they are (reported as kept).
    #[serde(default)]
    pub fill_only: bool,
    /// Temporary blocks (pillars, supports, liquid seals), in preference order.
    #[serde(default = "default_scaffold")]
    pub scaffold: Vec<acacia_world::neutral::Item>,
}

fn default_scaffold() -> Vec<acacia_world::neutral::Item> {
    DEFAULT_SCAFFOLD.to_vec()
}

impl BuildJob {
    pub fn new(structure: impl Into<PathBuf>, at: Transform) -> Self {
        Self { structure: structure.into(), at, stations: Vec::new(), fill_only: false, scaffold: default_scaffold() }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("plain data")
    }

    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}
