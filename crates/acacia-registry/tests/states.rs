use acacia_registry::prop;
use acacia_registry::value::{Direction, Half, SlabType, StairsShape};
use acacia_registry::{Block, BlockState};

fn state(s: &str) -> BlockState {
    s.parse().unwrap_or_else(|e| panic!("{s}: {e}"))
}

// Ids from the Java 26.3 data-generator report.
#[test]
fn java_ids_match_the_report() {
    assert_eq!(Block::Stone.default_state().java_id(), Some(1));
    assert_eq!(Block::OakStairs.default_state().java_id(), Some(5474));
    assert_eq!(state("oak_stairs[facing=east,half=bottom,shape=outer_right,waterlogged=false]").id(), 5542);
    assert_eq!(Block::NoteBlock.default_state().id(), 681);
    assert_eq!(state("note_block[instrument=custom_head,note=24,powered=false]").id(), 2029);
    assert_eq!(Block::RedBed.default_state().id(), 2257);
    assert_eq!(state("redstone_wire[power=15]").id(), 6862);
    assert_eq!(state("oak_slab[type=double,waterlogged=false]").id(), 15182);
}

#[test]
fn every_state_round_trips_through_its_string() {
    let mut count = 0;
    for block in Block::all() {
        for s in block.states() {
            assert_eq!(s.block(), block);
            assert_eq!(state(&s.to_string()), s, "{s}");
            count += 1;
        }
    }
    assert_eq!(BlockState::from_id(count - 1).map(|s| s.id()), Some(count - 1));
    assert_eq!(BlockState::from_id(count), None);
}

#[test]
fn typed_properties() {
    let s = Block::OakStairs.default_state();
    assert_eq!(s.get(prop::HorizontalFacing), Some(Direction::North));
    assert_eq!(s.get(prop::Half), Some(Half::Bottom));
    assert_eq!(s.get(prop::StairsShape), Some(StairsShape::Straight));
    assert_eq!(s.get(prop::Waterlogged), Some(false));
    assert_eq!(s.get(prop::Axis), None);
    let t = s.with(prop::HorizontalFacing, Direction::East).unwrap().with(prop::Half, Half::Top).unwrap();
    assert_eq!(t, state("oak_stairs[facing=east,half=top]"));
    assert_eq!(s.with(prop::HorizontalFacing, Direction::Up), None);
    let slab = Block::OakSlab.default_state().with(prop::SlabType, SlabType::Double).unwrap();
    assert_eq!(slab.id(), 15182);
    assert_eq!(state("redstone_wire").with(prop::Power, 15).unwrap().id(), 6862);
}

#[test]
fn names_parse_in_either_edition() {
    assert_eq!("noteblock".parse::<Block>(), Ok(Block::NoteBlock));
    assert_eq!("minecraft:note_block".parse::<Block>(), Ok(Block::NoteBlock));
    assert_eq!("azalea_leaves_flowered".parse::<Block>(), Ok(Block::FloweringAzaleaLeaves));
    assert_eq!("minecraft:hard_glass".parse::<Block>(), Ok(Block::HardGlass));
    assert_eq!("minecraft:stonecutter".parse::<Block>(), Ok(Block::Stonecutter));
    assert!(!Block::HardGlass.is_java());
    assert!(Block::Stonecutter.is_java());
    assert!("minecraft:no_such_block".parse::<Block>().is_err());
}

#[test]
fn tags_resolve_nested_tags() {
    use acacia_registry::BlockTag;
    assert!(Block::OakFence.is(BlockTag::Fences));
    assert!(Block::NetherBrickFence.is(BlockTag::Fences));
    assert!(!Block::OakFenceGate.is(BlockTag::Fences));
    assert!(Block::Stone.is(BlockTag::MineablePickaxe));
    assert_eq!(BlockTag::MineablePickaxe.name(), "minecraft:mineable/pickaxe");
    assert!(!Block::HardGlass.is(BlockTag::Impermeable));
}

#[test]
fn bedrock_only_states_follow_java() {
    let tnt = state("underwater_tnt[explode_bit=1]");
    assert_eq!(tnt.get(prop::BedrockExplodeBit), Some(true));
    assert!(tnt.java_id().is_none());
    assert!(tnt.id() >= 35723);
}
