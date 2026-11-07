use acacia_world::neutral::{self, Block, prop, value};
use acacia_world::{BlockRegistry, CustomBlock, NeutralFlags};

fn bedrock(name: &str, props: &str) -> u32 {
    BlockRegistry::vanilla().find(name, props).unwrap_or_else(|| panic!("{name}[{props}]"))
}

fn neutral_of(id: u32) -> (neutral::BlockState, NeutralFlags) {
    let s = BlockRegistry::vanilla().get(id).unwrap();
    (s.neutral.expect("vanilla state"), s.neutral_flags)
}

#[test]
fn every_vanilla_state_has_a_neutral_state() {
    let r = BlockRegistry::vanilla();
    for id in 0..r.len() as u32 {
        assert!(r.get(id).unwrap().neutral.is_some(), "{}", r.get(id).unwrap().name);
    }
}

#[test]
fn every_neutral_state_round_trips_to_bedrock() {
    let r = BlockRegistry::vanilla();
    for block in Block::all() {
        for s in block.states() {
            let id = r.runtime_id_of(s).unwrap_or_else(|| panic!("{s}"));
            let back = r.get(id).unwrap();
            // Bedrock merges some Java states; the trip back lands on the same Bedrock state at least.
            assert_eq!(r.runtime_id_of(back.neutral.unwrap()), Some(id), "{s} -> {}", back.name);
        }
    }
}

#[test]
fn renamed_and_merged_blocks() {
    assert_eq!(neutral_of(bedrock("minecraft:noteblock", "")).0.block(), Block::NoteBlock);
    let log = neutral_of(bedrock("minecraft:oak_log", "pillar_axis=x")).0;
    assert_eq!(log.get(prop::Axis), Some(value::Axis::X));
    let (stairs, flags) =
        neutral_of(bedrock("minecraft:oak_stairs", "minecraft:corner=none,upside_down_bit=0,weirdo_direction=0"));
    assert_eq!(stairs.block(), Block::OakStairs);
    assert_eq!(stairs.get(prop::Waterlogged), Some(false));
    assert!(flags.contains(NeutralFlags::WATERLOGGED));
    let slab = neutral_of(bedrock("minecraft:oak_double_slab", "minecraft:vertical_half=bottom")).0;
    assert_eq!(slab.get(prop::SlabType), Some(value::SlabType::Double));
    let bed = bedrock("minecraft:bed", "direction=0,head_piece_bit=0,occupied_bit=0");
    assert!(neutral_of(bed).1.contains(NeutralFlags::BLOCK_ENTITY));
    let hard = neutral_of(bedrock("minecraft:hard_glass", "")).0;
    assert_eq!(hard.block(), Block::HardGlass);
    assert_eq!(BlockRegistry::vanilla().runtime_id_of(hard), Some(bedrock("minecraft:hard_glass", "")));
}

#[test]
fn custom_blocks_shift_runtime_ids() {
    let blocks: Vec<_> = (0..40).map(|i| CustomBlock { name: format!("test:block_{i}"), state_count: 3 }).collect();
    let custom = BlockRegistry::vanilla().with_custom_blocks(&blocks);
    let vanilla = BlockRegistry::vanilla();
    let mut shifted = 0;
    for block in Block::all() {
        let s = block.default_state();
        let id = custom.runtime_id_of(s).unwrap();
        let v = vanilla.runtime_id_of(s).unwrap();
        assert_eq!(custom.get(id).unwrap().name, vanilla.get(v).unwrap().name, "{s}");
        assert_eq!(custom.get(id).unwrap().properties, vanilla.get(v).unwrap().properties, "{s}");
        shifted += usize::from(id != v);
    }
    assert!(shifted > 0);
}
