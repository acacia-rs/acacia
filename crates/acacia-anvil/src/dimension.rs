//! Java 26.3's dimension types (`data/minecraft/dimension_type/*.json`) for the Bedrock dimensions they
//! arrive as: the nether is 256 blocks high in Java (Bedrock gets the bottom 128) and the End has sky light
//! in Java since 1.21.9 (not in Bedrock). Bedrock dimensions differ only in height here, which is what
//! tells them apart.

use acacia_world::Dimension;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JavaDimension {
    /// Folder under `dimensions/minecraft/`.
    pub name: &'static str,
    pub min_y: i32,
    pub height: i32,
    pub has_sky: bool,
}

pub const OVERWORLD: JavaDimension = JavaDimension {
    name: "overworld",
    min_y: -64,
    height: 384,
    has_sky: true,
};
pub const NETHER: JavaDimension = JavaDimension {
    name: "the_nether",
    min_y: 0,
    height: 256,
    has_sky: false,
};
pub const END: JavaDimension = JavaDimension {
    name: "the_end",
    min_y: 0,
    height: 256,
    has_sky: true,
};

pub fn java_dimension(dim: Dimension) -> JavaDimension {
    match (dim.min_y, dim.height) {
        (0, 128) => NETHER,
        (0, 256) => END,
        _ => OVERWORLD,
    }
}
