use acacia_registry::{Block, BlockTag};

pub fn bounce(block: Block) -> f32 {
    if block == Block::SlimeBlock {
        1.0
    } else if block.is(BlockTag::Beds) {
        0.75
    } else {
        0.0
    }
}

pub fn friction(block: Block) -> f32 {
    match block {
        Block::Ice | Block::PackedIce | Block::FrostedIce => 0.98,
        Block::BlueIce => 0.989,
        Block::SlimeBlock => 0.8,
        _ => 0.6,
    }
}

pub fn speed_factor(block: Block) -> f32 {
    match block {
        Block::SoulSand | Block::HoneyBlock => 0.4,
        _ => 1.0,
    }
}

pub fn jump_factor(block: Block) -> f32 {
    match block {
        Block::HoneyBlock => 0.5,
        _ => 1.0,
    }
}
